# Handoff — ripwire-broker

Estado em 2026-10-05, até o
[D-152](spec/changelog.md#d-152--leitor-do-workspace-por-openat).
Para quem pega o projeto agora: o que existe, o que está no meio, o que falta e onde já se tropeçou.

## O que é

Um servidor MCP em Rust entre um agente de código (Claude Code, Codex) e o
[ripwire](https://github.com/redhat-et/ripwire). Expõe três tools (`context_for_task`,
`context_after_edit`, `context_before_finish`) e um resource de status, e devolve contexto estrutural
dentro de um orçamento, sem repetir o que a sessão já recebeu e sem nunca escrever no workspace.
Também funciona por hooks (o contexto entra sozinho no prompt) e tem um modo opcional `--online`
que consulta um classificador remoto (Jev). O PRD vigente é
[`spec/ripwire-broker-mcp.md`](spec/ripwire-broker-mcp.md).

## Estado

| fase | estado |
| --- | --- |
| 0–1 · spike e MVP | feitas |
| 2 · hooks, contexto incremental, `install`, `doctor` | feita |
| 3 · notas por modelo local | feita, com cache só em memória (o de disco espera a medição do §21.3) |
| 4–5 · `--online` | feitas, atrás da feature Cargo `online`; **experimental** até o A/B |
| barra de status do Claude Code (§24) | feita (D-123); validada à mão numa sessão real do Claude Code 2.1.285, com fixture de payload real (D-128); as seis divergências da validação fechadas (D-129 a D-131) |
| 6 · times e CI (HTTP autenticado, multi-workspace, políticas) | **não começada** |
| auditoria de 2026-10-04 (D-143, D-144) | os cinco defeitos mais graves e os achados médios corrigidos em TDD; ficam os baixos, o código morto e as simplificações (lista abaixo) |
| achados baixos, área da CLI e dos hooks (D-147) | corrigidos em TDD, com o código morto e as simplificações da área; as outras áreas seguem |
| achados baixos, núcleo MCP (D-148) | corrigidos em TDD, com as simplificações da área; seguem online, memória e eval |
| achados baixos, modo online (D-149) | corrigidos em TDD, com as simplificações da área; o `openat` do leitor veio no D-152 |
| achados baixos, memória, primeira parte (D-150) | sete corrigidos em TDD, entre eles o worker esperando o store no pool bloqueante (grupo 3) |
| achados baixos, memória (segunda parte) e eval (D-151) | corrigidos em TDD, com o pânico do worker, o `is_error` de infraestrutura e o prompt que segurava o timeout (grupo 3); o `connected` conferido num init real |
| leitor do workspace por `openat` (D-152) | cada componente aberto relativo ao anterior, sem seguir link, pela crate `rustix`: um diretório trocado por link depois das conferências não leva mais para fora do workspace |
| revisão de 2026-10-04 (D-146) | os cinco achados de maior impacto em produção corrigidos em TDD (leitura do online fora da thread assíncrona, TOCTOU do leitor, chave fora dos processos filhos, prazo no `ripwire --version`, erros do worker de memória no stderr); os demais ficam na lista abaixo |
| plugin do Claude Code ([plano](spec/plan/mod-plan.md), §25 do PRD) | Etapas 1 e 2 implementadas (D-158, D-159): `integrations/claude-code/` é o plugin e o mod (`hooks/register.ts`, testes em `integrations/claude-code/tests/`, `claude plugin test integrations/claude-code`, Claude Code 2.1.292), a raiz é o marketplace `aquental`. Release `v0.1.0` publicado pelo `release.yml` e fixado no `checksums.txt` (D-160). O que falta é do mantenedor, listado no §5a do plano: verificações numa sessão real, instalar pelo marketplace, medir |
| `--memory` · memória persistente ([PRD](docs/jev-mem-prd.md), [plano](spec/plan/jev-mem-plan.md)) | Fases 0 a 5 feitas (D-136 a D-142): store, coleta pelas tools e pelos hooks, comandos locais, worker de enriquecimento no `serve --memory`, `memory drain --online`, a leitura em `context_for_task` (`memories[]`, `provenance.memory`, seção legível no texto MCP), a consolidação (cadência de 20 enriquecimentos ou 24 h, decisões e ligações por par, nota derivada pelo `--summarizer-cmd` só com o gate de 0,85) e o instrumento da avaliação (`--memory-selection deterministic`, braços `broker-memory` e `broker-memory-deterministic`, sequências no corpus, custo da memória no relatório). A T2.0 (Choice no modelo pinado) rodou com a chave real. Pendente: a T3.11, validar num Claude Code e num Codex reais que os hosts usam as memórias; os hooks não as trazem na v1 (não fazem HTTP e não há cache de decisões). Pendente também a T5.3, a rodada da avaliação (≥ 30 tarefas em sequências, fora do repositório; o plano lista o que o instrumento ainda não faz). Fase 6 (fechamento) não começada; **experimental** |

O código não tem `TODO`/`FIXME`. As pendências moram no PRD (§19, §21, §23.17) e no
[changelog de decisões](spec/changelog.md), que é a fonte da verdade sobre o porquê de cada coisa.

## Como verificar

```sh
cargo test --all-targets                    # 755 testes, 5 ignorados (opt-in)
cargo test --all-targets --features online  # 773 testes, 8 ignorados
cargo clippy --all-targets -- -D warnings   # também com --features online
cargo fmt --check
```

- **CI** (`.github/workflows/rust.yml`): dois jobs, `default` e `online`, obrigatórios no `master`.
  Também cobre a porta do CA-10 (nenhum crate de rede no build padrão), a guarda de fixtures
  sintéticas e as propriedades.
- **`cargo-deny`:** agendado às segundas no `master` (`supply-chain.yml`), nunca em PR (D-108).
- **Ignorados:** precisam de algo externo — modelo local (`RIPWIRE_BROKER_TEST_MODEL`), chave da Jev
  (`RIPWIRE_BROKER_JEV_API_KEY`) ou um benchmark. O SLO do `hook --memory` também é medição manual em
  release (`the_hook_overhead_meets_the_slo`).

## Onde está o quê

- **`src/broker.rs`:** o núcleo (roteamento, orçamento, deduplicação, sessão).
- **`src/mcp.rs`:** a fachada MCP.
- **`src/upstream.rs` e `src/supervise.rs`:** o cliente do ripwire e o limite de memória.
- **`src/hook.rs` e `src/state.rs`:** os hooks e o estado de sessão em disco.
- **`src/worktree.rs`:** a impressão digital do `git status` que diz ao hook se um comando do shell
  mudou arquivos (D-129). Com prazo, teto de entradas e desligamento pela sessão; o hook a dispensa
  quando o próprio Claude Code manda a lista de arquivos (`bashEditDiff`, D-131).
- **`src/online/`:** o adaptador `--online`.
- **`src/memory/`:** a memória do `--memory` (registro, identidade, admissão, store, coleta, comandos
  `memory …`, e a leitura: `index.rs`, `retrieve.rs`, `recall.rs`). O controlador (`controller.rs`, `runtime.rs`) também compila no build padrão; só o
  `JevClient` exige a feature `online`.
- **`src/eval/` e `src/bin/ripwire-eval.rs`:** o instrumento do A/B, um segundo binário que o broker
  nunca chama.
- **`src/usage.rs`:** o `hook-stats`.
- **`src/statusline.rs` e `src/statusline_state.rs`:** a barra de status do Claude Code (§24). O primeiro
  lê o JSON do host, monta e ajusta a linha; o segundo é a projeção que os hooks publicam por sessão
  e workspace (`statusline/<hash>.json` no state-dir) e que o comando só lê. `src/hook.rs` publica,
  `src/install.rs` registra com `--statusline`.
- **`tests/`:** um arquivo por costura pública (`broker`, `mcp_surface`, `hooks`, `cli`, `statusline`,
  `notes`, `summarizer`, `worktree`, `upstream_ripwire`, `online*`, `memory_*`, `props*`, `eval`). As fixtures do ripwire e dos hosts são gravações reais.
- **`spec/`:**
  - `ripwire-broker-mcp.md`: o PRD;
  - `changelog.md`: D-001 a D-152, a tabela de índice no topo;
  - `plan/`: os planos de cada fase;
  - `diagrams/`: arquitetura, mantida à mão.
- **`integrations/`:** configuração e skill para Claude Code e Codex.

## Em andamento: as duas medições (D-116, D-117)

O plano está em [`spec/plan/plano-ab-e-session-hits.md`](spec/plan/plano-ab-e-session-hits.md).
Os instrumentos estão prontos; as medições, não.

### 1. A/B (PRD §16.2, §17, §23.15)

- **Instrumento:** `ripwire-eval` roda um corpus em seis braços (`none`, `ripwire`, `broker`,
  `broker-online` e, para a memória, `broker-memory` e `broker-memory-deterministic`, D-141) com
  Claude Code headless isolado, reduz cada transcript a contagens, pontua
  contra o commit de referência e julga as barras. Uso no [README](README.md#ab-evaluation).
- **Corpus:** 32 tarefas de commits reais em três repositórios (dois privados, A e B, e este). Mora
  **fora deste repositório**, em `~/projects/ai/CECI/ab-eval/` **noutra máquina** do mantenedor,
  com o próprio `README.md` e uma `SPEC.md` de como as tarefas foram escritas. Nunca versionar aqui:
  descreve código privado. Nesta máquina há só o corpus de diagnóstico do D-121, em
  `~/projects/ai/CECI/ab-eval-diag/`, que não serve ao A/B.
- **Validação:** 30 tarefas têm `check` validado (falha no base, passa no fix); 2 só medem custo.
- **Falta, nesta ordem:**
  1. o mantenedor revisar os enunciados (ressalvas no D-117: alguns nomeiam interfaces, um entrega
     o diagnóstico);
  2. dar a cada tarefa do repositório A uma porta própria e o relógio congelado no `env` (D-121), e
     revalidar (`ripwire-eval validate`). A porta vale para todas: a configuração de teste do A sobe
     o servidor em todo `mix test`. O relógio só importa nas tarefas cujos testes leem a data, mas
     congelá-lo nas outras é inofensivo e evita decidir tarefa por tarefa;
  3. **piloto pago**: 3 tarefas × 3 braços, teto de US$ 3 por execução, para medir custo real e
     gravar um transcript real (a fixture de `stream-json` dos testes é sintética);
  4. a rodada: 32 tarefas × 3 braços × 3 repetições. O braço `broker-online` só com a chave e
     consentimento para enviar trechos dos três repositórios ao provedor.
- **Se as barras passarem:** o `--online` deixa de ser experimental; atualizar o §23 do PRD.

### 2. `session_hits` em uso real (PRD §21.3)

- **O que existe:** `ripwire-broker hook-stats` soma as sessões salvas pelos hooks e mede a
  repetição dentro de cada sessão e **entre** sessões. A segunda é o que um cache persistente de
  notas (S3.15) acrescentaria.
- **Falta:** usar os hooks (`ripwire-broker install <host> --workspace … --hooks --write`) em
  trabalho real por alguns dias, ≥ 20 sessões, e rodar `hook-stats` (que ignora a sessão sem evento e sem fingerprint, só de falhas de launch). Na máquina do mantenedor,
  em 2026-10-01, não havia nenhuma sessão.
- **Regra proposta (o mantenedor decide):** repetição entre sessões < 15% recusa o S3.15;
  ≥ 30% o põe no plano.

## Pendências conhecidas, fora das medições

- **Barra de status: o campo `agent` (§24.8) nunca foi visto num payload real.** A validação do
  D-128 rodou sem `--agent`, e nenhum dos 121 payloads o trouxe. Uma sessão com `--agent`, gravada
  pelo `capture.sh` do roteiro (`~/projects/ai/CECI/statusline-manual/`, pasta local do mantenedor,
  não versionada), fecha isso.
- **Fase 6:** inteira. A política de falhar em CI com `strict=true` (§21.4) depende dela.
- **`sha2` preso abaixo de 0.11** no `dependabot.yml` (D-119). Quem mover o `rust-mcp-sdk` revê
  essa linha na mesma decisão.
- **Auditoria de 2026-10-04, o que ficou (D-143, D-144):** os achados baixos de todas as áreas
  foram corrigidos (D-147 a D-151). Ficam:
  - código que nunca executa: `Role::{Config, Test, Risk}`, que são reservados no schema v1, entre
    outros;
  - cerca de 25 itens de API pública usados só por testes;
  - as simplificações maiores listadas no D-143 (`read_with`, `commit_*`, batches de controller e
    consolidate, `admit`/`admit_note`).
- **Revisão de 2026-10-04, o que ficou (D-146; D-147 a D-152 fecharam o resto):** `unwrap` de
  mutex dentro de `Drop` (`broker.rs`, mantido pelo D-094, ver D-148).
- **Memória, registrado nas Fases 2 a 5 (D-138 a D-142):** o cache de decisões (sem ele os hooks
  não entregam memória e `--memory-read-request-limit 0` não serve nada), as notas derivadas fora
  dos 5 s da rodada, a cadência fora do `memory status`, e os demais itens dos D-140 e D-142.
- **Diagrama:** `spec/diagrams/` não se atualiza sozinho. Quem mudar a topologia edita o JSON e roda
  `deliver` do archify de novo (D-115, D-132, D-151). O archify é um skill, em `~/.agents/skills/archify/`,
  não um comando no `PATH`.

## Armadilhas já pisadas

Cada uma custou uma conclusão errada antes de ser achada. O changelog conta seis "notas de método".

- **Um pipe devolve o status do último comando.** `cargo test | tail` e `… && echo ok` passam com o
  build quebrado. Verificar com `if cargo …; then`, sem pipe.
- **No zsh, `$var` sem aspas não vira várias palavras.** `"--features online"` num laço chega ao
  cargo como um argumento só. Numa rodada de mutações, o cargo recusa o argumento, sai com erro, e
  a mutação aparece "morta" sem teste nenhum ter rodado (D-140): conferir no log que a falha foi
  um `panicked` do teste esperado.
- **Restaurar um arquivo com mtime antigo engana o cargo.** Ele não recompila e o teste roda o
  binário velho. `touch` nos fontes depois de mutações.
- **Teste que depende de tempo passa pelo motivo errado sob carga.** Com 100 ms de `sleep` no `git`
  falso, o teste das duas impressões lentas seguidas (limite de 50 ms) passava em paralelo porque o
  prazo de 500 ms estourava sob carga, não pela regra testada. Com 60 ms ele só passa pela regra
  (D-129). Folga pequena acima do limite, e mutação para confirmar.
- **O CI não tem identidade git.** Teste que faz commit passa `-c user.email=… -c user.name=…`, como
  o `common::sample_repo`.
- **O `master` é protegido** (checks obrigatórios, `enforce_admins`). Tudo entra por PR, e os PRs
  entram por squash. Um push num branch de PR já mesclado não volta ao `master`: foi o que gerou o
  #29.
- **O repositório é público.** Detalhes dos repositórios privados do corpus não entram em commit,
  changelog nem PR.
- **A suíte do repositório A abre uma porta fixa, e parte dos testes lê a data.** A porta é da
  configuração de teste, então vale para toda execução: duas suítes ao mesmo tempo, e a segunda morre
  com `eaddrinuse` e o `check` conta como falha. A data é de alguns testes: num `fix` antigo, eles
  envelhecem com o calendário. Medido no corpus de diagnóstico (D-121).
- **O corpus usa um Postgres compartilhado.** As migrações do repositório A têm efeitos globais no
  cluster: rodar uma validação ou rodada por vez, e sem outra sessão trabalhando no A.

## Convenções

- **TDD por costura pública:** teste vermelho antes, nos arquivos de `tests/`; nada de testar
  função privada.
- **Mutação para provar que um teste morde.** Já pegou meia dúzia de testes que não testavam nada.
- **Toda decisão vira uma entrada `D-NNN` em `spec/changelog.md`,** em português: linha no índice
  do topo (a ordem do bloco recente é decrescente) e seção no fim.
- **Commits e PRs em inglês;** código e comentários também. Docs de `spec/` em português.
- **Nada de dependência nova sem motivo:** o grafo do build padrão é uma promessa (CA-10).
