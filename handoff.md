# Handoff — ripwire-broker

Estado em 2026-10-10, até o
[D-169](spec/changelog.md#d-169--roteiro-dos-testes-pendentes-e-três-medições).
O passo a passo de cada teste que falta está em
[`spec/plan/roteiro-testes-pendentes.md`](spec/plan/roteiro-testes-pendentes.md).
Para quem pega o projeto agora: o que existe, o que está no meio, o que falta e onde já se tropeçou.

## O que é

Um servidor MCP em Rust entre um agente de código (Claude Code, Codex) e o
[ripwire](https://github.com/redhat-et/ripwire). Expõe três tools (`context_for_task`,
`context_after_edit`, `context_before_finish`) e um resource de status, e devolve contexto estrutural
dentro de um orçamento, sem repetir o que a sessão já recebeu e sem nunca escrever no workspace.
Também funciona por hooks (o contexto entra sozinho no prompt) e tem um modo opcional `--online`
que consulta um classificador remoto (o Jev da TypeSafe ou, com `--jev-provider cloudflare`, o Clef
da Cloudflare). O PRD vigente é
[`spec/ripwire-broker-mcp.md`](spec/ripwire-broker-mcp.md).

## Estado

| fase | estado |
| --- | --- |
| 0–1 · spike e MVP | feitas |
| 2 · hooks, contexto incremental, `install`, `doctor` | feita |
| 3 · notas por modelo local | feita; o cache é em memória e, com `--summarizer-cache`, também em disco, no state dir (S3.15, D-168, [plano](spec/plan/xtd/plan-s3-15-cache-de-notas.md)) |
| 4–5 · `--online` | feitas, atrás da feature Cargo `online`; **experimental** até o A/B |
| `--jev-provider cloudflare` (D-166, #89) | feito em TDD e rodado contra a Cloudflare real: o Clef (Workers AI) pelo mesmo protocolo do Jev. O provider decide a URL (nenhuma vem da configuração), abre o envelope `{success, errors, messages, result}` e limita o pedido a 64 perguntas; `--jev-account-id` é obrigatório com ele e redigido no `jev.log`; a credencial vem da mesma `RIPWIRE_BROKER_JEV_API_KEY`. O padrão segue `typesafe`, e **o provider em uso é o Jev**. Com Cloudflare o modelo padrão é `clef-flash`, mantido pelo mantenedor depois da medição: com os limiares do Jev ele rejeita o arquivo de teste do corpus sintético, que o `clef` admite (6,3 vezes o preço, `--jev-model clef`). A Cloudflare não será o provider (D-167), então não há limiares por provider nem planos de tê-los |
| barra de status do Claude Code (§24) | feita (D-123); validada à mão numa sessão real do Claude Code 2.1.285, com fixture de payload real (D-128); as seis divergências da validação fechadas (D-129 a D-131); `hooks sem sessão` quando a entrada não traz `session_id` (D-163, #82) |
| 6 · times e CI (HTTP autenticado, multi-workspace, políticas) | **não começada** |
| auditoria de 2026-10-04 (D-143, D-144) | os cinco defeitos mais graves e os achados médios corrigidos em TDD; ficam os baixos, o código morto e as simplificações (lista abaixo) |
| achados baixos, área da CLI e dos hooks (D-147) | corrigidos em TDD, com o código morto e as simplificações da área; as outras áreas seguem |
| achados baixos, núcleo MCP (D-148) | corrigidos em TDD, com as simplificações da área; seguem online, memória e eval |
| achados baixos, modo online (D-149) | corrigidos em TDD, com as simplificações da área; o `openat` do leitor veio no D-152 |
| achados baixos, memória, primeira parte (D-150) | sete corrigidos em TDD, entre eles o worker esperando o store no pool bloqueante (grupo 3) |
| achados baixos, memória (segunda parte) e eval (D-151) | corrigidos em TDD, com o pânico do worker, o `is_error` de infraestrutura e o prompt que segurava o timeout (grupo 3); o `connected` conferido num init real |
| leitor do workspace por `openat` (D-152) | cada componente aberto relativo ao anterior, sem seguir link, pela crate `rustix`: um diretório trocado por link depois das conferências não leva mais para fora do workspace |
| revisão de 2026-10-04 (D-146) | os cinco achados de maior impacto em produção corrigidos em TDD (leitura do online fora da thread assíncrona, TOCTOU do leitor, chave fora dos processos filhos, prazo no `ripwire --version`, erros do worker de memória no stderr); os demais ficam na lista abaixo |
| plugin do Claude Code ([plano](spec/plan/mod-plan.md), §25 do PRD) | Etapas 1 e 2 implementadas (D-158, D-159): `integrations/claude-code/` é o plugin e o mod (`hooks/register.ts`, testes em `integrations/claude-code/tests/`, `claude plugin test integrations/claude-code`, Claude Code 2.1.292), a raiz é o marketplace `aquental`. Release `v0.1.0` publicado pelo `release.yml` e fixado no `checksums.txt` (D-160). Mods sem variável de early access desde o 2.1.292 (D-161); o `register.ts` e o kit tipados com as declarações do host, e `MultiEdit` como `/^MultiEdit$/` no matcher, porque o literal quebrava a inferência nos tipos do 2.1.295 (#83); o braço `broker-plugin` do eval é trabalho futuro (D-162). O que falta é do mantenedor, listado no §5a do plano: verificações numa sessão real (P1, P2, P6, P7), instalar pelo marketplace (P3), medir (P8); depois a T3.4 e a T6.1 |
| `--memory` · memória persistente ([PRD](docs/jev-mem-prd.md), [plano](spec/plan/jev-mem-plan.md)) | Fases 0 a 5 feitas (D-136 a D-142): store, coleta pelas tools e pelos hooks, comandos locais, worker de enriquecimento no `serve --memory`, `memory drain --online`, a leitura em `context_for_task` (`memories[]`, `provenance.memory`, seção legível no texto MCP), a consolidação (cadência de 20 enriquecimentos ou 24 h, decisões e ligações por par, nota derivada pelo `--summarizer-cmd` só com o gate de 0,85) e o instrumento da avaliação (`--memory-selection deterministic`, braços `broker-memory` e `broker-memory-deterministic`, sequências no corpus, custo da memória no relatório). A T2.0 (Choice no modelo pinado) rodou com a chave real. Pendente: a T3.12, o texto enriquecido das observações (`memory-observation/v2`, PRD v0.4 §5.2, D-171), sem a qual nenhuma observação automática é entregue; a T3.11, validar num Claude Code e num Codex reais que os hosts usam as memórias; os hooks não as trazem na v1 (não fazem HTTP e não há cache de decisões). Pendente também a T5.3, a rodada da avaliação (≥ 30 tarefas em sequências, fora do repositório; o plano lista o que o instrumento ainda não faz). Fase 6 (fechamento) não começada; **experimental**. Para acompanhar ao vivo: `--memory-debug-log` (D-164), uma linha por evento em `debug.log` ao lado do store, escrita pelo servidor, pelos hooks e pelos comandos, para `tail -F`. A leitura faz o routing junto com o primeiro scoring e não pergunta se deve parar sem evidência (D-165): 1–2 requests, cabendo nos 850 ms. Num A/B da leitura com store sintético a seleção não mudou e a mediana caiu de 782 para 524 ms; o efeito na tarefa do agente fica para a T5.3 |

O código não tem `TODO`/`FIXME`. As pendências moram no PRD (§19, §21, §23.17) e no
[changelog de decisões](spec/changelog.md), que é a fonte da verdade sobre o porquê de cada coisa.

## Como verificar

```sh
cargo test --all-targets                    # 840 testes, 5 ignorados (opt-in)
cargo test --all-targets --features online  # 877 testes, 8 ignorados
cargo clippy --all-targets -- -D warnings   # também com --features online
cargo fmt --check
claude plugin validate --strict integrations/claude-code   # o plugin
claude plugin validate --strict .                          # o marketplace
claude plugin test integrations/claude-code                # o mod: 46 testes, sem sessão nem rede
```

- **CI** (`.github/workflows/rust.yml`): dois jobs, `default` e `online`, obrigatórios no `master`.
  Também cobre a porta do CA-10 (nenhum crate de rede no build padrão), a guarda de fixtures
  sintéticas e as propriedades. Um terceiro, `plugin`, não obrigatório, instala o Claude Code
  2.1.292 pelo npm e roda as três linhas `claude plugin` acima.
- **Release** (`.github/workflows/release.yml`, D-160): só numa tag `v*` igual à versão do
  `Cargo.toml`. Chama o `rust.yml`, compila os quatro alvos e publica tarballs e `SHA256SUMS`. A
  ordem de publicar uma versão está no README do plugin ("Publishing a version"): os hashes entram
  no `scripts/checksums.txt` depois do release, num PR.
- **`cargo-deny`:** agendado às segundas no `master` (`supply-chain.yml`), nunca em PR (D-108).
- **Ignorados:** precisam de algo externo — modelo local (`RIPWIRE_BROKER_TEST_MODEL`), chave do
  provider (`RIPWIRE_BROKER_JEV_API_KEY`) ou um benchmark. O SLO do `hook --memory` também é medição manual em
  release (`the_hook_overhead_meets_the_slo`). Ele lê o tempo que o hook mede em si mesmo, nas linhas
  `collect` do `debug.log` (D-170).
- **Ao vivo** (`tests/online_live.rs`, com `--ignored`): perguntam ao Jev, ou à Cloudflare com
  `RIPWIRE_BROKER_LIVE_PROVIDER=cloudflare` e `RIPWIRE_BROKER_LIVE_ACCOUNT_ID`. Com o padrão
  `clef-flash`, `a_real_provider_classifies_the_synthetic_corpus` reprova (D-166); passa com
  `RIPWIRE_BROKER_LIVE_MODEL=clef`.

## Onde está o quê

- **`src/broker.rs`:** o núcleo (roteamento, orçamento, deduplicação, sessão).
- **`src/mcp.rs`:** a fachada MCP.
- **`src/upstream.rs` e `src/supervise.rs`:** o cliente do ripwire e o limite de memória.
- **`src/hook.rs` e `src/state.rs`:** os hooks e o estado de sessão em disco.
- **`src/worktree.rs`:** a impressão digital do `git status` que diz ao hook se um comando do shell
  mudou arquivos (D-129). Com prazo, teto de entradas e desligamento pela sessão; o hook a dispensa
  quando o próprio Claude Code manda a lista de arquivos (`bashEditDiff`, D-131).
- **`src/online/`:** o adaptador `--online`. O `JevProvider` (`mod.rs`) é o único lugar que sabe
  host, caminho, modelo padrão e teto de perguntas de cada provider; o `JevClient` (`jev.rs`) abre o
  envelope da Cloudflare.
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
  `notes`, `summarizer`, `worktree`, `upstream_ripwire`, `online*`, `memory_*`, `props*`, `eval`,
  `plugin`). As fixtures do ripwire e dos hosts são gravações reais.
- **`spec/`:**
  - `ripwire-broker-mcp.md`: o PRD;
  - `changelog.md`: D-001 a D-169, a tabela de índice no topo;
  - `plan/`: os planos de cada fase;
  - `diagrams/`: arquitetura, mantida à mão.
- **`integrations/`:** configuração e skill para Claude Code e Codex.
- **`integrations/claude-code/`:** também é o plugin do Claude Code (§25 do PRD): manifesto em
  `.claude-plugin/plugin.json`, servidor em `.mcp.json`, hooks em `hooks/hooks.json`, o mod em
  `hooks/register.ts` (testes em `tests/*.test.ts`, contrato do `$.state` em `types/index.d.ts`),
  o resolvedor `scripts/broker`, `scripts/install-binary.sh` e `scripts/checksums.txt` (o release
  fixado). A raiz do repositório é o marketplace `aquental` (`.claude-plugin/marketplace.json`).
  `tests/plugin.rs` testa os arquivos e scripts; `tests/hooks.rs` escreve o golden
  `integrations/claude-code/tests/parity.ts` com `RIPWIRE_BROKER_WRITE_PARITY=1`.

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
  trabalho real por alguns dias, ≥ 20 sessões, e rodar `hook-stats` (que ignora a sessão sem evento e sem fingerprint, só de falhas de launch). Nesta máquina,
  em 2026-10-10: 17 sessões, 852 eventos, 85,2% de acerto dentro da sessão e **44,6% de repetição
  entre sessões** (160 de 359 fingerprints). Faltam 3 sessões para as 20 da regra; o número já
  está acima dos 30%.
- **Decidido (D-168):** com 44,6% o mantenedor mandou implementar o S3.15 sem esperar as 20
  sessões. O cache persistente de notas existe, opt-in (`--summarizer-cache`). Ele só serve ao
  `serve`: os hooks continuam sem notas, porque um hook não roda modelo.

## Pendências conhecidas, fora das medições

- **Plugin do Claude Code:** as pendências do mantenedor no §5a do
  [plano](spec/plan/mod-plan.md#5a-pendências-do-mantenedor). Numa sessão `claude --plugin-dir
  integrations/claude-code`: o workspace que o servidor recebe (P1), os hooks clássicos (P2), o
  interruptor mod ↔ clássico, a ordem `tool.call` ↔ `PostToolUse`, o `$.state` depois de um reload e
  o `bashEditDiff` (P6), a faixa e o `/ripwire-status` (P7). Depois, instalar pelo marketplace (P3) e
  medir (P8); com as medições saem a T3.4 e a T6.1.

- **Barra de status: o campo `agent` (§24.8) nunca foi visto num payload real.** A validação do
  D-128 rodou sem `--agent`, e nenhum dos 121 payloads o trouxe. Uma sessão com `--agent`, gravada
  pelo `capture.sh` do roteiro (`~/projects/ai/CECI/statusline-manual/`, pasta local do mantenedor,
  não versionada), fecha isso.
- **T3.11, primeira tentativa (D-168):** três sessões reais do Claude Code 2.1.296 num repositório
  sintético, com `serve --online --memory`. Coleta, ingestão e enriquecimento funcionaram (4
  memórias), e a leitura visitou as 4 e perguntou ao Jev (22 perguntas, 1 request), mas **não
  manteve nenhuma** (`stop=empty`). Sem memória entregue, o consumo pelo host segue sem validar; o
  que falta saber primeiro é por que a leitura rejeita memórias do mesmo arquivo da tarefa.
- **Por que a leitura rejeita (D-171):** reproduzido com `--log`. O Jev deu 0,20 e 0,16 de
  relevância às duas observações automáticas, contra a barra de 0,60 (`ENTRY`), porque o texto diz
  que uma análise rodou sobre o arquivo e não diz sobre o quê. Uma nota explícita no mesmo store,
  para a mesma tarefa, teve 0,91 e foi entregue: a leitura funciona. O PRD v0.4 §5.2 libera o texto
  enriquecido (`memory-observation/v2`: nomes de símbolos, dependentes e testes, e os `kind` dos
  achados; nunca assinatura, corpo, valor ou diff). **A implementar: T3.12**, antes da T3.11 e da
  T5.3. Que os nomes bastem é hipótese; o aceite é o bloco B do roteiro com `kept≥1`.
- **SLO do `hook --memory` (D-170):** medido pelo cronômetro do próprio hook, p95 de 9,5 a 9,9 ms
  contra 10. A folga é de meio milissegundo, e quase todo o custo é a observação (admissão e
  gravação durável); a T3.12 mexe nesse caminho e tem de medir de novo.
- **A chave:** `RIPWIRE_BROKER_JEV_API_KEY` guarda a credencial de um provider por vez: confira
  qual está lá antes de uma chamada real.
- **Fase 6:** inteira. A política de falhar em CI com `strict=true` (§21.4) depende dela.
- **`sha2` preso abaixo de 0.11** no `dependabot.yml` (D-119). Revisto no D-167: o `rust-mcp-sdk`
  mais novo ainda é o 2.0.0 pinado, com `sha2` 0.10. Quem mover o SDK revê essa linha na mesma decisão.
- **Auditoria de 2026-10-04, o que ficou:** o código sem uso saiu no D-167, que lista o que ficou e
  por quê (costuras de teste, `Role::{Config, Test, Risk}` reservados no schema v1,
  `Outcome::Failed`). Seguem as simplificações maiores do D-143 (`read_with`, `commit_*`, batches de
  controller e consolidate, `admit`/`admit_note`).
- **Memória, registrado nas Fases 2 a 5 (D-138 a D-142):** o cache de decisões (sem ele os hooks
  não entregam memória e `--memory-read-request-limit 0` não serve nada), as notas derivadas fora
  dos 5 s da rodada, a cadência fora do `memory status`, e os demais itens dos D-140 e D-142.
- **Diagrama:** `spec/diagrams/` não se atualiza sozinho (refeito no D-167, com a Cloudflare). Quem
  mudar a topologia edita o JSON e roda o `deliver` do archify de novo, com `--repo-root` apontando
  para este checkout e o `meta.repository.revision` no commit conferido (D-115, D-132, D-151). O
  archify é um skill, em `~/.agents/skills/archify/`, não um comando no `PATH`.

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
- **O `master` é protegido** (checks `default` e `online` obrigatórios, `enforce_admins`, e o branch
  precisa estar em dia com a base). Tudo entra por PR, e os PRs entram por merge commit ("Merge pull
  request #N", assim desde pelo menos o #68). Dois PRs abertos ao mesmo tempo: depois de mesclar o
  primeiro, o segundo é recusado ("not up to date"); `gh pr update-branch N` traz o `master`, o CI
  roda de novo e aí ele entra (#83). Um push num branch de PR já mesclado não volta ao `master`: foi
  o que gerou o #29.
- **O canal `stable` do Claude Code pode estar atrás.** Em 2026-10-06 o `stable` era o 2.1.285 e
  o `latest` o 2.1.292; `claude update` não saía do 2.1.285, e o updater voltava a ele. A saída foi
  `autoUpdatesChannel: "latest"` e `minimumVersion` no `~/.claude/settings.json` e `claude install
  <versão>` com as sessões antigas fechadas: uma sessão aberta reaponta o `~/.local/bin/claude`
  (D-161). Conferir com `readlink ~/.local/bin/claude`.
- **O kit de testes do mod não carimba `origin`.** Um `$.prompt.submit` do próprio mod volta ao
  `prompt.submit` dele no kit; o mod se reconhece pelo texto que guardou no `$.state` (D-159). E o
  `claude plugin validate` recusa `export {}` no `types/index.d.ts`.
- **Dois testes de memória falham sob carga e passam isolados**
  (`memory_controller::auth_failures_suspend_the_worker_until_reauthorized`,
  `memory_retrieval::hashes_and_generation_are_revalidated_right_before_delivery`, `Err(Locked)`).
  Já existiam antes do plugin (D-158). Rodar de novo, isolado, antes de concluir que algo quebrou.
- **A barra de status testada à mão precisa de `session_id`.** O retrato é achado por `(host,
  session_id, raiz)`; um JSON de teste sem ele nunca acha nada. Antes do D-163 isso aparecia como
  `hooks sem dados` e mandava investigar hooks que funcionavam; agora aparece `hooks sem sessão`.
  O `session_id` de uma sessão do Claude Code é o nome da pasta dela no scratchpad.
- **O repositório saiu de `~/projects/ai/CECI/ripwire-broker` para `~/projects/ai/ripwire-broker`.**
  O que guardava o caminho absoluto quebrou em silêncio: o `statusLine` do
  `.claude/settings.local.json` (a barra some, sem erro) e o registro do `cargo install`. O local agora
  usa `"${CLAUDE_PROJECT_DIR:-.}/.claude/statusline.sh"`, e o script não passa `--workspace` (a barra
  o lê do stdin). Os `--workspace` do `.claude/settings.json` e do `.mcp.json`, que o `install`
  escreve, continuam absolutos: mudar o repositório de lugar pede rodar o `install` de novo.
- **`cargo install` não lembra as features.** O binário em `~/.cargo/bin` tem `online`; reinstalar
  sem `--features online` tira o cliente HTTP em silêncio. `cargo install --path . --features online
  --locked`; o `~/.cargo/.crates2.json` diz com que features foi instalado.
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
  do topo e seção no fim. A ordem do índice não é uma só: o bloco do D-155 em diante é crescente
  (linha nova depois da última), o que vem antes dele é decrescente.
- **Commits e PRs em inglês;** código e comentários também. Docs de `spec/` em português.
- **Nada de dependência nova sem motivo:** o grafo do build padrão é uma promessa (CA-10).
