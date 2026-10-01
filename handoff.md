# Handoff — ripwire-broker

Estado em 2026-10-01, até o
[D-119](spec/changelog.md#d-119--pendências-do-handoff-sha2-no-dependabot-e-links-dos-planos). Para
quem pega o projeto agora: o que existe, o que está no meio, o que falta e onde já se tropeçou.

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
| 6 · times e CI (HTTP autenticado, multi-workspace, políticas) | **não começada** |

O código não tem `TODO`/`FIXME`. As pendências moram no PRD (§19, §21, §23.17) e no
[changelog de decisões](spec/changelog.md), que é a fonte da verdade sobre o porquê de cada coisa.

## Como verificar

```sh
cargo test --all-targets                    # 323 testes, 2 ignorados (opt-in)
cargo test --all-targets --features online  # 336 testes, 4 ignorados
cargo clippy --all-targets -- -D warnings   # também com --features online
cargo fmt --check
```

- **CI** (`.github/workflows/rust.yml`): dois jobs, `default` e `online`, obrigatórios no `master`.
  Também cobre a porta do CA-10 (nenhum crate de rede no build padrão), a guarda de fixtures
  sintéticas e as propriedades.
- **`cargo-deny`:** agendado às segundas no `master` (`supply-chain.yml`), nunca em PR (D-108).
- **Ignorados:** precisam de algo externo — modelo local (`RIPWIRE_BROKER_TEST_MODEL`), chave da Jev
  (`RIPWIRE_BROKER_JEV_API_KEY`) ou um benchmark.

## Onde está o quê

- **`src/broker.rs`:** o núcleo (roteamento, orçamento, deduplicação, sessão).
- **`src/mcp.rs`:** a fachada MCP.
- **`src/upstream.rs` e `src/supervise.rs`:** o cliente do ripwire e o limite de memória.
- **`src/hook.rs` e `src/state.rs`:** os hooks e o estado de sessão em disco.
- **`src/online/`:** o adaptador `--online`.
- **`src/eval/` e `src/bin/ripwire-eval.rs`:** o instrumento do A/B, um segundo binário que o broker
  nunca chama.
- **`src/usage.rs`:** o `hook-stats`.
- **`tests/`:** um arquivo por costura pública (`broker`, `mcp_surface`, `hooks`, `cli`, `online*`,
  `props*`, `eval`). As fixtures do ripwire e dos hosts são gravações reais.
- **`spec/`:**
  - `ripwire-broker-mcp.md`: o PRD;
  - `changelog.md`: D-001 a D-119, a tabela de índice no topo;
  - `plan/`: os planos de cada fase;
  - `diagrams/`: arquitetura, mantida à mão.
- **`integrations/`:** configuração e skill para Claude Code e Codex.

## Em andamento: as duas medições (D-116, D-117)

O plano está em [`spec/plan/plano-ab-e-session-hits.md`](spec/plan/plano-ab-e-session-hits.md).
Os instrumentos estão prontos; as medições, não.

### 1. A/B (PRD §16.2, §17, §23.15)

- **Instrumento:** `ripwire-eval` roda um corpus em quatro braços (`none`, `ripwire`, `broker`,
  `broker-online`) com Claude Code headless isolado, reduz cada transcript a contagens, pontua
  contra o commit de referência e julga as barras. Uso no [README](README.md#ab-evaluation).
- **Corpus:** 32 tarefas de commits reais em três repositórios (dois privados, A e B, e este). Mora
  **fora deste repositório**, em `~/projects/ai/CECI/ab-eval/` na máquina do mantenedor, com o
  próprio `README.md` e uma `SPEC.md` de como as tarefas foram escritas. Nunca versionar aqui:
  descreve código privado.
- **Validação:** 30 tarefas têm `check` validado (falha no base, passa no fix); 2 só medem custo.
- **Falta, nesta ordem:**
  1. o mantenedor revisar os enunciados (ressalvas no D-117: alguns nomeiam interfaces, um entrega
     o diagnóstico);
  2. revalidar no dia (`ripwire-eval validate`; os testes do repositório A dependem da data);
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
  trabalho real por alguns dias, ≥ 20 sessões, e rodar `hook-stats`. Na máquina do mantenedor,
  em 2026-10-01, não havia nenhuma sessão.
- **Regra proposta (o mantenedor decide):** repetição entre sessões < 15% recusa o S3.15;
  ≥ 30% o põe no plano.

## Pendências conhecidas, fora das medições

- **Fase 6:** inteira. A política de falhar em CI com `strict=true` (§21.4) depende dela.
- **Cancelamento sob HTTP/2** (reset de stream) não tem teste: os fixtures falam HTTP/1.1
  (`tests/online_protocol.rs`).
- **`sha2` preso abaixo de 0.11** no `dependabot.yml` (D-119). Quem mover o `rust-mcp-sdk` revê
  essa linha na mesma decisão.
- **Diagrama:** `spec/diagrams/` não se atualiza sozinho. Quem mudar a topologia edita o JSON e roda
  `deliver` de novo (D-115).
- **Uma validação do corpus falhou sem causa provada:** 9 tarefas do repositório A falharam no fix
  e passaram de novo com a máquina parada (D-117). Suspeitas: carga, ou outra sessão no mesmo
  Postgres. Hoje o `validate` grava logs; se repetir, eles dizem por quê.

## Armadilhas já pisadas

Cada uma custou uma conclusão errada antes de ser achada. O changelog conta seis "notas de método".

- **Um pipe devolve o status do último comando.** `cargo test | tail` e `… && echo ok` passam com o
  build quebrado. Verificar com `if cargo …; then`, sem pipe.
- **No zsh, `$var` sem aspas não vira várias palavras.** `"--features online"` num laço chega ao
  cargo como um argumento só.
- **Restaurar um arquivo com mtime antigo engana o cargo.** Ele não recompila e o teste roda o
  binário velho. `touch` nos fontes depois de mutações.
- **O CI não tem identidade git.** Teste que faz commit passa `-c user.email=… -c user.name=…`, como
  o `common::sample_repo`.
- **O `master` é protegido** (checks obrigatórios, `enforce_admins`). Tudo entra por PR, e os PRs
  entram por squash. Um push num branch de PR já mesclado não volta ao `master`: foi o que gerou o
  #29.
- **O repositório é público.** Detalhes dos repositórios privados do corpus não entram em commit,
  changelog nem PR.
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
