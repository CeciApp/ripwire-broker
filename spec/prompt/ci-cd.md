# Testes de propriedade e CI/CD — ripwire-broker

> Este prompt foi escrito originalmente **sem acesso ao `src/`** e marcava suas lacunas como
> `inferred`, `UNKNOWN` e `SPECULATING`. Todas foram preenchidas pela leitura do código. Os fatos
> abaixo foram verificados contra a árvore — as constantes por `grep`, e cada caminho dito
> "alcançável" por uma sonda que compila `use`/referência de fora do crate. Não são inferências
> do `Cargo.toml`.

## Regra

Escreva testes de propriedade **contra as assinaturas e os invariantes desta página**. Não
invente módulos, tipos, nomes de ferramenta ou regras de PRD que não estejam aqui. Se precisar
de algo que não está descrito, diga qual arquivo/símbolo falta em vez de supor.

## Contexto

- **Crate:** `ripwire-broker` 0.1.0, `edition = "2024"`, MIT. Pacote único, sem workspace.
- **Toolchain:** **fixada em `rust-toolchain.toml` (canal `1.98.1`, com `clippy` e `rustfmt`).**
  Não é preciso propor MSRV — ele já existe e o CI deve usá-lo.
- **Alvos de build (confirmados via `cargo metadata`):**
  - `lib` → `src/lib.rs` (nome do crate: `ripwire_broker`)
  - `bin` → `src/main.rs` (nome: `ripwire-broker`)
  - `example` → `examples/jev_record.rs` (imprime o corpus `prompts/v1`, uma requisição JSON por
    linha, para gravação ao vivo sem crate de rede no build) e `examples/spike.rs` (medições de
    tamanho e latência)
  - 12 alvos de teste de integração em `tests/` (listados adiante)
- **Dependências (autoritativas, do `Cargo.toml`):**
  `rust-mcp-sdk = "=2.0.0"` (`default-features = false`; features `server`, `client`, `macros`,
  `stdio`), `async-trait` 0.1, `serde` 1 + derive, `serde_json` 1, `sha2` 0.10, `ignore` 0.4,
  `tokio-util` 0.7, `tokio` 1 (`rt-multi-thread`, `macros`, `time`, `sync`, `process`, `io-util`).
  Opcionais atrás da feature `online`: `secrecy` 0.10, `reqwest` 0.12 (`rustls-tls`, `http2`,
  `default-features = false`).
- **Features:** `default = []` (**tem de continuar sem pilha de rede — CA-10**);
  `online = ["dep:secrecy", "dep:reqwest"]`.
- **Dev-dependencies hoje:** `tokio` (`test-util`), `tempfile` 3. **Sem `proptest`.**
- **Runtime:** tokio, assíncrono.
- **Host de CI:** GitHub Actions (`.github/workflows/rust.yml`).

## O que o sistema faz

Servidor MCP local que reduz a superfície do [Ripwire](https://github.com/redhat-et/ripwire)
(33 verbos) a **três ferramentas** com orçamento de tokens, deduplicação, procedência e
limitações preservadas. Local, offline e somente-leitura por padrão.

```text
agente ⇄ stdio ⇄ ripwire-broker ⇄ stdio ⇄ ripwire <workspace> --mcp
```

- **Ferramentas publicadas:** `context_for_task`, `context_after_edit`, `context_before_finish`.
  Um recurso: `ripwire-broker://status` (só dados operacionais — contagens, durações e
  categorias de erro; nunca prompt, código ou caminho).
- **Envelope:** `schema_version = "ripwire-broker.context/v1"` (`src/model.rs`), com `items`,
  `tests`, `risks`, `limitations`, `notes`, `provenance` e `budget`. Todo texto de repositório
  vive em `content.untrusted_repository_data` — é dado, nunca instrução.
- **Fluxo de uma chamada:** `mcp` (fachada do SDK) → `broker` (núcleo) → `router` (intenção
  determinística) → `upstream` (cliente MCP do processo ripwire) → `normalize` (payload → entradas)
  → `dedup` → `budget` (corte por orçamento) → envelope.
- **Subsistemas opcionais:** `notes` (modelo local por subprocesso, para notas arquiteturais) e
  `online` (classificador semântico remoto, só com `--online`).
- **Comandos de um disparo além do `serve`:** `hook`, `hook-log`, `prompt`, `doctor`, `install`,
  e os internos `__supervise` / `__watch` (limite de memória do ripwire).

## Visibilidade dos módulos — a restrição que decide o plano

Isto era `UNKNOWN` no prompt original e é o fato mais importante para o trabalho: **os módulos
com as funções puras mais atraentes para PBT são privados** e não são alcançáveis de `tests/`.

| Módulo | Visibilidade | Consequência para PBT |
| --- | --- | --- |
| `workspace`, `session`, `notes`, `model`, `metrics`, `state`, `cli`, `broker`, `mcp`, `upstream`, `online`, `summarizer`, `supervise`, `doctor`, `install`, `local`, `hook` | `pub mod` | alcançável de `tests/` |
| **`normalize`, `router`, `markup`, `budget`, `dedup`** | **`mod` (privado)** | **não alcançável de `tests/`** |
| `online::coordinator` | `mod` privado (reexporta `OnlineConfig`, `OnlineEngine`, `OnlineTotals`) | só pelos tipos reexportados |
| `online::merge` | `pub(crate)` | não alcançável |
| `online::credential`, `online::jev` | `pub mod` com `#[cfg(feature = "online")]` | só no build `online` |
| `session::{item,test,risk,note}_fingerprint`, `session::reference`, `SessionMemory::{has,remember}` | `pub(crate)` | não alcançável |
| `Snapshot.text` | campo privado | `units()` exige um `Snapshot` real, via `WorkspaceReader::snapshot` num tempdir |

**Decida e declare explicitamente** qual caminho tomar para os privados, porque a escolha tem
custo em cada direção:

1. promover a `pub` (ou `pub(crate)` + reexport) o que o PBT precisa — muda a superfície pública;
2. testar através das costuras públicas (`Broker` com um `FakeUpstream`) — propriedade mais fraca
   e mais lenta, mas sem mexer na API;
3. `#[cfg(test)] mod tests` dentro de `src/` — **o repositório não tem nenhum, por convenção
   deliberada**, e um teto no `Inflight` já foi revertido justamente para preservá-la
   (`D-093` → `D-094`). Não escolha esta sem dizer que está quebrando a convenção.

**`markup::parse` é o caso mais urgente e é privado.** Ele lê a saída XML-ish do ripwire, ou seja,
entrada de fora do processo, e tinha **dois panics** por fatiamento fora de limite e em fronteira
de caractere (`D-091`). É o alvo número um de `proptest` sobre `&str` arbitrário, e hoje só é
alcançável indiretamente, alimentando um payload por um `FakeUpstream`.

## Invariantes reais, com as constantes do código

Números do código, não do PRD. Use-os nas propriedades.

**Orçamento e envelope** (`src/broker.rs`, `src/budget.rs`)
- `MIN_BUDGET_TOKENS = 256`; `MIN_ONLINE_BUDGET_TOKENS = 512` (piso do `context_for_task` num
  processo com `--online`, D-072).
- Estimativa de tokens = `len(JSON) / 4`, arredondando para cima.
- `max_item_tokens` padrão 800 (teto por item, `× 4` em bytes).
- Padrões: `context_for_task` 2500, `context_after_edit` 1500, `context_before_finish` 1800.
- `GATE_RISKS = ["cochange_missing", "contract_change"]` — nunca suprimidos, mesmo em sessão
  incremental (CA-05).
- `MIN_RIPWIRE_VERSION = (0, 6, 4)`; uma versão ilegível **passa**.
- `REQUIRED_VERBS`: 10 verbos somente-leitura, que são também a allowlist.
  Os verbos de escrita e `quality_baseline` são deliberadamente ausentes.

**Limiares do classificador** (`src/online/decision.rs`) — **estritos**
- `ADMISSION = 0.25` (admite com `p > 0.25`), `SELECTION = 0.50` (seleciona com `p > 0.50`).
- Igual ao limiar **não** passa. `None` é `Unknown`, **nunca** zero e nunca `false`.
- `file_decision`: um arquivo em fragmentos fica com a maior nota conhecida, e só é rejeitado se
  **todos** os fragmentos foram avaliados.

**Limites de requisição** (`src/online/request.rs`)
- `MAX_QUESTIONS = 128`, `MAX_REQUEST_BYTES = 38_000`,
  `MAX_EVIDENCE_UNITS = 8`, `EVIDENCE_BATCH_BYTES = 14 KiB`.

**Leitura do workspace** (`src/online/reader.rs`)
- `PREVIEW_BYTES = 16 KiB`, `LOOKAHEAD_PREVIEW_BYTES = 4 KiB`, `CHUNK_BYTES = 3 KiB`,
  `MAX_UNIT_BYTES = 24 KiB`, `LOCATION_ONLY_BYTES = 1 MiB`, `MAX_READ_BYTES = 8 MiB`.
- Motivos de inelegibilidade (enum `Ineligible`, 12 variantes): `outside`, `sensitive_name`,
  `hidden`, `dependency_or_build`, `symlink`, `not_regular`, `unreadable`, `ignored`, `too_large`,
  `binary`, `not_utf8`, `private_key`.

**Notas** (`src/notes.rs`)
- `PROMPT_VERSION = "notes/v1"`, `MAX_GROUPS = 3`, `MAX_EVIDENCE_CHARS = 2000`,
  `MAX_NOTE_CHARS = 600`.

**Versões que entram em chave de cache**
- `online::prompt::VERSION = "v1"`, `online::cache::POLICY_VERSION = "policy/v1"`.

**Métricas** (`src/metrics.rs`): `RECENT_REQUESTS = 32`.

## Superfície P0 — confirmada, com assinaturas

Não é mais especulação. Cada item traz onde está e o que provar.

**P0.1 — O guarda do workspace nunca deixa escapar da raiz** *(alcançável)*
`ripwire_broker::workspace::Workspace::{relative, check_symbol}` (`src/workspace.rs`).
`relative(&self, path: &str) -> Result<String, String>` resolve `.`/`..` lexicalmente, recusa
absolutos de fora e symlinks que saem da raiz, e trata caminhos inexistentes pelo ancestral
existente mais próximo. Propriedade: para **qualquer** string, ou é `Err`, ou o `Ok(rel)`
concatenado à raiz permanece dentro da raiz canônica. `check_symbol` aplica a mesma regra à parte
de arquivo de um seed `@ARQUIVO:LINHA`.
*Oráculo:* comparar com `Path::canonicalize` quando o caminho existe.

**P0.2 — Política de elegibilidade e o crate `ignore`** *(alcançável, precisa de tempdir)*
`ripwire_broker::online::reader::WorkspaceReader::{snapshot, files_in, is_fresh}`.
`snapshot` checa cada componente do caminho: nome sensível, oculto, diretório de
dependência/build, symlink (em **todo** componente), regular, tamanho, ignorado, binário, UTF-8 e
marcador de chave privada. Propriedades: nenhum arquivo inelegível é lido; um arquivo ignorado por
`.gitignore`/`.ignore` nunca vira `Snapshot`; `is_fresh` é falso após qualquer mudança de bytes.
*Atenção:* o `.ignore` da raiz do repositório reabre `graft/` para busca — não confunda com
política do broker.

**P0.3 — Unidades de evidência cobrem o texto e respeitam os limites** *(alcançável via tempdir)*
`ripwire_broker::online::reader::units(&Snapshot, &[u64]) -> Vec<Unit>`.
Propriedades: as faixas de bytes são **contíguas, não se sobrepõem e cobrem todo o texto** (exceto
`location_only`, que não gera unidade); cada unidade é alinhada a linha; nenhuma passa de
`MAX_UNIT_BYTES`; `start_line`/`end_line` são 1-based inclusivos e consistentes com os bytes; uma
linha maior que `MAX_UNIT_BYTES` é cortada em fronteira de caractere; uma unidade começa em cada
linha de símbolo informada.

**P0.4 — Lotes de requisição respeitam todos os limites e preservam a ordem** *(alcançável, puro)*
`ripwire_broker::online::request::{build, request_bytes, batches}`.
Propriedades: para qualquer `Vec<StateItem>`, todo lote tem `≤ MAX_QUESTIONS` perguntas e
`≤ MAX_REQUEST_BYTES` de JSON; num lote de `source_selection`, `≤ MAX_EVIDENCE_UNITS` unidades e
`≤ EVIDENCE_BATCH_BYTES` de texto; a **concatenação dos lotes mais os `too_large` é exatamente a
entrada, na ordem**; `request_bytes` é **igual** ao `len` do JSON de `build` (já existe teste de
exemplo — a versão de propriedade é a que vale).

**P0.5 — Validação de resposta do classificador** *(alcançável, puro)*
`ripwire_broker::online::response::parse_answers(model, ids, body) -> Result<Vec<Option<f64>>, InvalidResponse>`.
Propriedades sobre bytes/JSON arbitrários: nunca entra em panic; probabilidade ausente, não-finita
ou fora de `[0,1]` vira `None` — **nunca é truncada para dentro da faixa nem lida como `false`**;
modelo diferente do pinado → `WrongModel`; id não perguntado → `UnknownQuestion`, invalidando a
resposta **inteira**; o vetor de saída tem o mesmo tamanho e ordem de `ids`.

**P0.6 — Limiares estritos** *(alcançável, puro)*
`ripwire_broker::online::decision::{admit, select, file_decision}`. Propriedades: monotonicidade em
`p`; igualdade ao limiar não passa; `None` nunca se torna `0.0`; `file_decision` devolve `Unknown`,
não `Rejected`, se algum fragmento não foi avaliado.

**P0.7 — Redação e limite de texto remoto** *(alcançável, puro)*
`ripwire_broker::online::redact::remote_text(text, secret, max) -> String`.
Propriedades: saída só com ASCII imprimível e espaço; `len ≤ max`; **nunca contém o segredo**,
para qualquer `text` e qualquer `secret` não vazio; o fatiamento nunca quebra fronteira de
caractere.

**P0.8 — Chaves de cache mudam se e somente se a decisão muda** *(alcançável, puro)*
`ripwire_broker::online::cache::key(&KeyParts)` e `ripwire_broker::notes::key(model_id, scope, evidence)`.
Ambas usam sha2 com prefixo de comprimento por parte. Propriedades: mudar **qualquer** parte muda a
chave; partes diferentes não colidem por concatenação (o prefixo de comprimento existe para isso —
prove com pares como `("ab","c")` vs `("a","bc")`); a chave é estável entre execuções.

**P0.9 — Saneamento de nota** *(alcançável, puro)*
`ripwire_broker::notes::sanitize(&str) -> String`. Propriedades: sem caracteres de controle exceto
`\n`; sem sequências de escape de terminal (CSI e OSC); **contagem de caracteres** `≤ MAX_NOTE_CHARS`
(não bytes); idempotente; nunca entra em panic com UTF-8 arbitrário.
`ripwire_broker::notes::scope(path)` e `groups(&[Item])`: no máximo `MAX_GROUPS` grupos, na ordem do
primeiro item.

**P0.10 — O parser da linha de comando é puro e total** *(alcançável)*
`ripwire_broker::cli::parse(Vec<String>) -> Result<Command, String>`. O módulo declara "parsing é
puro; nada aqui toca o disco". Propriedades: nunca entra em panic com `argv` arbitrário; toda
mensagem de erro contém o `USAGE`; ida-e-volta dos argumentos de `serve`; flags `--jev-*` sem
`--online` são erro; `--online` fora do `serve` é erro.

**P0.11 — Invariantes do orçamento** *(só pela costura pública)*
`budget` é privado; exercite por `Broker::context_for_task` com `FakeUpstream`.
Propriedades: `budget.shown + budget.omitted` é igual ao total de entradas não-limitação;
`estimated_tokens ≤ requested_tokens`, **a menos que** só restem limitações (que nunca são
cortadas); `truncated` é verdadeiro se e somente se `omitted > 0`; `next_step` existe se e somente
se `truncated`; limitações nunca são descartadas.

**P0.12 — O leitor tolerante nunca entra em panic** *(hoje só indireto — ver acima)*
`markup::parse(&str) -> Option<Node>`. Propriedade: para **qualquer** `&str`, devolve `Some`/`None`
sem panic e sem laço infinito. Historicamente falso (`D-091`). Exige decidir a visibilidade.

## Superfície P1

**P1.1 — Retry-After** `ripwire_broker::online::retry_after::parse(value, now)`: nunca entra em
panic; só segundos e IMF-fixdate são aceitos; data no passado dá `Duration::ZERO`; datas inválidas
(`31 Feb`, hora 24) dão `None`.

**P1.2 — Ordenação dos candidatos** `ripwire_broker::online::ranked_paths(...)`: documentos são
excluídos; a ordem é `(melhor prioridade, ordem do ripwire)`; caminhos são distintos; `lines`
ordenadas e sem repetição.

**P1.3 — Escalonador** `ripwire_broker::online::scheduler::Scheduler` com um `FakeClassifier`
(já existe em `tests/common/classifier.rs`): nunca mais de `max_in_flight` em voo; `request_limit`
nunca é excedido contando tentativas; ids sobrevivem a respostas fora de ordem; nunca há deadlock
ao dividir um lote.

**P1.4 — Isolamento da feature** Mais compilação que PBT: o build `default` não pode referenciar
`reqwest`/`secrecy`. **Já existe** o teste `the_build_has_no_network_stack` em
`tests/mcp_surface.rs`. Não duplique — no máximo estenda.

## Rejeitar por padrão

- Conformidade de protocolo do `rust-mcp-sdk` (não reimplementamos JSON-RPC).
- Sessão MCP stdio ao vivo como PBT (lenta, e testa o SDK).
- Comportamento de `reqwest`, TLS, DNS. **Nenhum teste de propriedade toca a rede.**
- Internos de `serde`, `sha2`, `ignore`.
- "não entra em panic" como **única** asserção — aceitável apenas em P0.12 e P0.5, onde a
  totalidade *é* a propriedade, e ainda assim acompanhada de asserções de forma.
- Reproduzir o corpus congelado de prompts como PBT: `prompts_v1_are_frozen_for_*`,
  `the_live_recording_still_matches_prompts_v1` e `the_recorded_live_answers_parse_in_question_order`
  (em `tests/online_units.rs`) são testes-golden **de propósito**. Mudar o texto tem de quebrá-los.

## Cobertura atual — o PBT precisa somar, não repetir

`cargo test --all-targets`: **234 passam, 2 ignorados.** Com `--features online`: **247 passam,
4 ignorados.** Nenhum teste inline em `src/`.

| Alvo | default | `--features online` | Costura |
| --- | --- | --- | --- |
| `tests/broker.rs` | 52 | 52 | núcleo `Broker` com payloads gravados |
| `tests/cli.rs` | 37 | 37 | linha de comando; `parse` puro e execuções e2e do binário |
| `tests/hooks.rs` | 17 | 17 | eventos de host → broker → saída |
| `tests/mcp_surface.rs` | 11 | 13 | o binário como servidor MCP por stdio |
| `tests/notes.rs` | 19 | 19 | notas com um `FakeSummarizer` |
| `tests/online.rs` | 42 (+1 ign.) | 42 (+1 ign.) | adaptador online ponta a ponta, sem rede |
| `tests/online_units.rs` | 24 | 24 | partes puras do online |
| `tests/online_scheduler.rs` | 18 | 18 | escalonador com classificador falso |
| `tests/online_protocol.rs` | — | 11 | protocolo HTTP em loopback |
| `tests/online_live.rs` | — | 0 (+2 ign.) | provider real |
| `tests/summarizer.rs` | 5 (+1 ign.) | 5 (+1 ign.) | modelo local por subprocesso |
| `tests/upstream_ripwire.rs` | 9 | 9 | processo ripwire real |

**Ignorados, e como habilitar** (não os ligue no CI de PR):
- `tests/online_live.rs` (2) — `#[ignore]`, exigem `RIPWIRE_BROKER_JEV_API_KEY` e a feature
  `online`: `cargo test --features online --test online_live -- --ignored`.
- `tests/summarizer.rs` (1) — exige `RIPWIRE_BROKER_TEST_MODEL` (modelo local real).
- `tests/online.rs::overhead_of_batching_and_merge` (1) — medição, roda com
  `--release ... -- --ignored --nocapture`.

**Auxiliares já disponíveis em `tests/common/`** — reaproveite em vez de recriar:
`FakeUpstream` (ripwire roteirizado, com fixtures gravadas, sequências, travas e falhas),
`FakeClassifier` + `Counters`, `FakeSummarizer` (com trava, sem tempo real),
`sample_repo()`, `slow_ripwire()`, `flaky_ripwire()`, `write_executable()`, `fixture()`,
`jev_corpus::requests()`.

`write_executable()` existe por um motivo que importa no CI: no Linux, escrever um script
enquanto outra thread faz `fork` faz o filho herdar o descritor e o `exec` falhar com
**ETXTBSY**. Isso já derrubou uma execução de CI (`D-088`). Se você gerar executáveis num teste de
propriedade, use esse auxiliar.

## Segurança — o que aqui é propriedade de segurança, não higiene

Este crate tem um modelo de ameaça explícito: ele lê texto de repositório e saída de processo
alheio, e promete não vazar conteúdo nem executar o que lê. Quatro dos P0 **são controles de
segurança**, e devem ser tratados como tal ao priorizar:

- **P0.1 e P0.2 fuzzam uma fronteira de segurança.** O guarda de workspace (RF-02, CA-08) e a
  política de elegibilidade são o que impede path traversal e o envio de arquivo sensível. Uma
  propriedade que falha ali é vulnerabilidade, não bug de formatação.
- **P0.12 e P0.5 são disponibilidade.** O broker é um servidor de vida longa lendo stdout do
  ripwire e resposta de classificador. Panic em chamada de ferramenta é negação de serviço a
  partir de entrada de fora do processo — e `markup::parse` já teve dois (`D-091`). "Não entra em
  panic" aqui é a propriedade, não uma asserção fraca.
- **P0.7 é confidencialidade.** `redact::remote_text` é a última barreira antes de texto remoto
  chegar a status, log ou agente. A propriedade "nunca contém o segredo, para qualquer entrada"
  é o controle.

**P0.13 — Texto de repositório não pode fechar o bloco que o embrulha** *(alcançável, novo)*
`ripwire_broker::local::{CONTEXT_OPEN, CONTEXT_CLOSE}` são `pub`, e `model::Envelope` tem todos os
campos `pub`, então dá para gerar envelopes arbitrários com `proptest`. O comando `prompt` embrulha
o envelope em `<ripwire-broker-context untrusted="true">…</ripwire-broker-context>` e escapa `<`/`>`
como `<`/`>` justamente para que conteúdo de repositório não possa fechar o bloco antes
da hora (injeção de prompt, D-052). Propriedade: para **qualquer** envelope, incluindo um cujo
`untrusted_repository_data` contenha literalmente `</ripwire-broker-context>`, a serialização
escapada não contém `CONTEXT_OPEN` nem `CONTEXT_CLOSE`. Existe um teste de exemplo para um payload
hostil em `tests/cli.rs`; a versão de propriedade é a que fecha a classe.

### O próprio suíte de testes precisa ser seguro

Propriedades que geram caminhos estão fuzzando um guarda de traversal. Se o guarda tiver um
defeito, um teste descuidado escreve ou apaga fora do lugar.

- **Raiz sempre em `tempfile::TempDir`.** Nunca a raiz do repositório, nunca `$HOME`, nunca um
  caminho vindo de variável de ambiente.
- **Nenhuma escrita ou remoção com caminho gerado** fora do tempdir. Em particular, nada de
  `fs::remove_dir_all` com entrada de `proptest`.
- **Nunca execute conteúdo gerado.** `write_executable()` existe para os fixtures do suíte; não o
  alimente com bytes gerados.
- **Limite os tamanhos gerados.** `MAX_READ_BYTES` é 8 MiB e `LOCATION_ONLY_BYTES` é 1 MiB.
  Estratégia sem teto estoura a memória do runner. Gere arquivos pequenos (na ordem de dezenas de
  KiB) e cubra os limiares de tamanho com poucos casos dirigidos, não com `proptest`.
- **`.proptest-regressions/` é entrada versionada.** O prompt manda versioná-la, e isso está certo
  — mas revise cada arquivo antes de commitar. Vale a mesma regra do corpus do Jev: sintético,
  sem caminho real e sem nada com cara de credencial.
- **Nenhuma propriedade toca a rede.** Não habilite a feature `online` no job de propriedades: a
  superfície pura de `online::*` compila no build default e não precisa dela.

### Fixtures e corpus gravados

`tests/fixtures/` tem 21 arquivos, incluindo `jev/live_v1.json`, uma gravação real do provider. Ela
já se limita a "digests, status, shape and probabilities", a partir de um corpus **sintético**
(`tests/common/jev_corpus.rs`, texto inventado, nunca lido de um workspace). Hoje nenhuma fixture
contém string com cara de credencial — verificado. Mantenha assim, e prenda isso no CI: uma
gravação futura que caia direto do provider é o caminho mais provável de um segredo ou de código
real entrar no repositório.

## Forma exigida do CI

O workflow atual (`.github/workflows/rust.yml`) é mínimo: um job `build` em `ubuntu-latest` com
quatro passos — `cargo build`, `cargo test`, e os mesmos dois com `--features online`. **Sem
`fmt`, sem `clippy`, sem `nextest`, sem cache, sem `audit`/`deny` — e sem nenhum dos
endurecimentos abaixo.** Preencher essas lacunas faz parte da entrega.

### Jobs

- **PR, só features default:** `cargo fmt --all --check`, `cargo clippy --all-targets -D warnings`,
  `nextest` (unitários + propriedade). O build default **tem de continuar sem rede** (CA-10).
- **Job separado:** `--features online` (clippy + nextest). **Não pode ser o único job.**
- Usar a toolchain de `rust-toolchain.toml` (1.98.1); não escolher outra no workflow.
- Filtro do `nextest` para que os testes de propriedade sejam visíveis e executáveis
  separadamente dos testes de exemplo.
- Adicionar `proptest` como dev-dependency; **versionar `.proptest-regressions/`**; documentar
  `PROPTEST_CASES`. Fixá-lo baixo nas propriedades com tempdir (P0.2, P0.3), que fazem E/S por caso.

### Endurecimento do workflow — tudo ausente hoje, conferido

- **`permissions:` no topo, mínimo.** O workflow não declara nenhuma, então herda o padrão do
  repositório/organização, que pode ser `write-all`. Declare `contents: read` no nível do workflow
  e eleve por job só onde for preciso.
- **Fixe as actions por SHA completo.** Hoje é `actions/checkout@v4`, uma tag mutável. Vale para
  toda action de terceiro que você adicionar (nextest, cache).
- **`persist-credentials: false` no checkout**, para o `GITHUB_TOKEN` não ficar no `.git/config`
  disponível aos passos seguintes.
- **`--locked` em todo comando cargo.** O `Cargo.lock` é versionado e o workflow não o respeita,
  então hoje o CI pode resolver versões diferentes das testadas localmente. É reprodutibilidade e
  cadeia de suprimentos ao mesmo tempo.
- **`timeout-minutes` por job.** O suíte inicia subprocessos, tem um ripwire falso que dorme 30 s,
  supervisores de memória e processos vigia. Job sem timeout pendura o runner.
- **`concurrency` com `cancel-in-progress`**, por ref, para push sucessivo não empilhar execução.
- **Nunca `pull_request_target`.** O gatilho hoje é `pull_request`, que é o correto: PR de fork não
  recebe segredo. Não troque para expor segredo a fork.

### Cadeia de suprimentos

- **`cargo-deny` com `deny.toml` versionado** (não existe hoje), cobrindo as quatro seções:
  `advisories`, `licenses` (o crate é MIT — declare a allowlist), `bans` (duplicata, crate yanked) e
  `sources` (só crates.io). **Agendado no `main`, não bloqueando PR** — deriva da advisory-db não é
  falha do autor do PR.
- **Dependabot** (`.github/dependabot.yml` não existe) para `cargo` e `github-actions`. Declare a
  política do pin: `rust-mcp-sdk = "=2.0.0"` é exato de propósito e **só sobe por decisão
  registrada no changelog**, nunca por bump automático.
- **Cache com escopo.** Se usar cache de build, separe a chave por conjunto de features e não
  restaure no `main` um cache gravado por branch de PR — é o caminho clássico de envenenamento.

### Segredos

- **Nunca exportar `RIPWIRE_BROKER_JEV_API_KEY` no CI de PR.** Os testes que a usam são `#[ignore]`
  e devem continuar assim; os dois de `online_live.rs` só rodam com `--ignored`.
- Se algum dia rodarem no CI: só no `main`, com ambiente protegido, nunca em PR de fork.
- Nada de `set -x` nem `--nocapture` em passo que possa ver a chave. O código já reduz o texto
  remoto a categoria e status e redige a chave do `Retry-After` (`redact::remote_text`); o CI não
  pode desfazer isso imprimindo o ambiente.
- **Acrescente `.env` e `.envrc` ao `.gitignore`.** O produto trata `*.env` como nome sensível
  (`SENSITIVE_EXTENSIONS`, D-089), mas o repositório não os ignora, então um arquivo local criado
  por engano é commitável.

### Controles que este repositório permite de graça

Verificados contra a árvore agora; todos passam limpos hoje, então adotá-los não custa trabalho:

- **`#![forbid(unsafe_code)]`** em `src/lib.rs` e `src/main.rs`. O crate não tem **nenhum** `unsafe`.
- **`#![deny(clippy::print_stdout, clippy::dbg_macro)]` em `src/lib.rs`.** Em `serve`, o stdout
  carrega o protocolo MCP: um `println!` perdido na biblioteca corrompe a sessão. Hoje só existe
  `eprintln!` fora do `main.rs` (em `supervise.rs`), e como `main.rs` é outro crate root, os
  `println!` legítimos dos comandos de um disparo não são afetados. `cargo clippy --all-targets
  --features online` passa com os dois lints ligados.
- **CA-10 verificável por máquina.** `cargo tree -e normal` no build default não traz
  `reqwest`, `secrecy`, `rustls` nem `hyper`; com `--features online` traz. Um passo que falhe se
  aparecerem no default transforma o CA-10 em porta de CI, complementando o teste
  `the_build_has_no_network_stack` que já existe.
- **Guarda de fixture sintética.** Um passo que falhe se `tests/fixtures/` ganhar string com cara de
  credencial (`Bearer `, `api_key`, `authorization`, `sk-…`). Hoje não há nenhuma.

### Proteção de branch (fora do workflow, mas parte da entrega)

- Exigir os dois jobs como status checks obrigatórios no `master`.
- Exigir revisão antes do merge. O histórico recente do repositório tem PR mesclado sem revisão;
  se a intenção é manter assim, registre a decisão em vez de deixar implícito.

### Opcional, diga que é opcional se propuser

SBOM (`cargo-auditable` ou `cargo-cyclonedx`) e política de egresso no runner
(`step-security/harden-runner`). O segundo tem apelo real aqui, porque o build default promete não
ter pilha de rede — mas não o entregue como obrigatório.

## Entregáveis

1. A decisão de visibilidade para `markup`/`normalize`/`router`/`budget`/`dedup`, com o custo
   assumido, antes de qualquer código.
2. Tabela P0/P1 priorizada, referenciando os itens desta página (não uma tabela nova especulativa),
   com os quatro controles de segurança (P0.1, P0.2, P0.5, P0.7, P0.12, P0.13) no topo.
3. Os testes de propriedade, usando os auxiliares de `tests/common/` e respeitando as regras de
   segurança do próprio suíte (raiz em tempdir, tamanhos limitados, nada executado).
4. O workflow de CI, preenchendo as lacunas listadas acima, incluindo o endurecimento.
5. `deny.toml` e `.github/dependabot.yml`, que não existem.
6. Os lints gratuitos (`forbid(unsafe_code)`, `deny(clippy::print_stdout)`) e o passo de CA-10.
7. A entrada correspondente em `spec/changelog.md`, seguindo a convenção `D-NNN` do repositório.
