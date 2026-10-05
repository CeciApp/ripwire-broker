# Testes de propriedade e CI/CD — ripwire-broker

Revalidado em **2026-10-04** contra o workspace local, branch `fix/lows-online`,
HEAD `dc56f0a`, incluindo os arquivos presentes no diretório de trabalho. Não é uma
verificação do `master` remoto. O arquivo `/Users/aquental/Downloads/ci-cd.md` foi
usado como referência secundária; os contratos abaixo vêm dos fontes e da configuração.

## Escopo e retificações

Esta especificação orienta a manutenção dos testes de propriedade (PBT) e do CI.
O plano incremental está em [ci-cd-plan.md](../plan/ci-cd-plan.md).
A proposta histórica [proposta-pbt-e-ci-cd.md](../plan/proposta-pbt-e-ci-cd.md)
registra as sete fatias já entregues em D-107–D-113; não devem ser reimplementadas.

Na versão anterior, os três marcadores de incerteza apareciam na introdução histórica,
e a seção de visibilidade repetia o marcador de desconhecimento. Não havia uma lista
restante de pendências etiquetadas: havia afirmações desatualizadas tratadas como fatos.
Esta revisão substitui essas afirmações por evidência e distingue comportamento existente,
requisito futuro e verificação externa.

| Afirmação anterior | Retificação e evidência |
| --- | --- |
| `markup` e `budget` privados; sem `proptest` | Ambos são `pub mod` em `src/lib.rs`; `proptest = "1.11.0"` está em `Cargo.toml`. |
| Toolchain fixada equivale a MSRV declarado | `rust-toolchain.toml` fixa `1.98.1`; não há `package.rust-version`. Não há MSRV declarado. |
| Um binário, 12 alvos de integração | `cargo metadata --offline --locked --no-deps` identifica dois binários e 23 alvos de integração. |
| CI mínimo; faltam deny, Dependabot e lints | Esses controles já existem, descritos adiante. |
| Nenhuma propriedade implementada | Há propriedades em `props`, `props_fs`, `broker` e `online_scheduler`, inclusive para memória. |
| `pub mod` implica teste inline | Exportar módulo não cria teste inline. Não foram encontrados `#[cfg(test)]` ou `mod tests` em `src/`. |
| Propriedades absolutas de lotes, segredo e hashes | Os contratos abaixo corrigem a ordem dos lotes, a exceção de tamanho, o marcador de redação e o alcance dos testes de hash. |
| Proteção de branch ausente e revisão obrigatória por implementar | D-107 registra checks `default`/`online` com `strict` e decisão de não exigir aprovação humana. Estado remoto exige consulta própria. |

## Configuração atual

Fontes: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `src/lib.rs`, `src/main.rs`
e `src/bin/ripwire-eval.rs`.

- Pacote único `ripwire-broker` 0.1.0, edition 2024, MIT; sem seção de workspace.
- Toolchain `1.98.1`, componentes `clippy` e `rustfmt`. Usar essa configuração no CI;
  não inventar nem anunciar um MSRV com base nela.
- Biblioteca `ripwire_broker`; binários `ripwire-broker` (default-run) e `ripwire-eval`;
  exemplos `jev_record` e `spike`. Os alvos são descobertos pelo Cargo.
- Única feature explícita: `online = ["dep:secrecy", "dep:reqwest"]`.
  Não existe uma linha `default = []`; sem seleção de features, `online` fica desligada.
- Dependências diretas: `futures-util` 0.3 (`std`, sem defaults), `rust-mcp-sdk = "=2.0.0"`
  (`server`, `client`, `macros`, `stdio`, sem defaults), `async-trait` 0.1,
  `serde` 1 (`derive`), `serde_json` 1, `sha2` 0.10, `ignore` 0.4, `tokio-util` 0.7,
  `tokio` 1 (`rt-multi-thread`, `macros`, `time`, `sync`, `process`, `io-util`),
  `unicode-width` 0.2 e `libc` 0.2. Opcionais: `secrecy` 0.10 e `reqwest` 0.12
  (`rustls-tls`, `http2`, sem defaults).
- Dev-dependencies: `tokio` com `test-util`, `h2` 0.4, `tempfile` 3 e `proptest` 1.11.0.
  Esses números são requisitos do manifesto; as versões resolvidas são as do lockfile.
- `libc` é dependência direta usada para flags de abertura de arquivos; seu comentário
  sobre não introduzir crate nova refere-se ao grafo transitivo preexistente.
- `#![forbid(unsafe_code)]` já existe na biblioteca e no binário principal;
  `#![deny(clippy::print_stdout, clippy::dbg_macro)]` já existe na biblioteca.
  Esses atributos não se propagam automaticamente ao crate root de `ripwire-eval`.

## Produto e fronteiras

Servidor MCP por stdio: agente → broker → processo `ripwire <workspace> --mcp`.
`src/mcp.rs` publica `context_for_task`, `context_after_edit`, `context_before_finish`
e o recurso operacional `ripwire-broker://status`. O envelope usa
`model::SCHEMA_VERSION = "ripwire-broker.context/v1"` e contém itens, testes, riscos,
limitações, notas, memórias, procedência e orçamento. Conteúdo de fonte é dado não confiável.

O núcleo em `src/broker.rs` usa roteamento, upstream, normalização, deduplicação e orçamento.
O adaptador semântico online enriquece `context_for_task`; memória acrescenta coleta,
persistência, recuperação e processamento em segundo plano quando habilitada.
Em `serve`, `--memory` implica `--online`; **em `hook`, `--memory` apenas publica localmente**.
O build default não liga o cliente HTTP (CA-10); isso não é uma sandbox de rede para
subprocessos nem uma proibição de download de dependências durante o CI.

`src/cli.rs` reconhece `serve`, `hook`, `hook-log`, `hook-stats`, `prompt`, `doctor`,
`install`, `statusline`, `memory`, além de `__supervise` e `__watch` internos.
`ripwire-eval` oferece `check`, `validate`, `run`, `report`; execuções reais de agentes
não fazem parte do CI de propriedades.

## Visibilidade e decisão de testes

A fonte de autoridade é `src/lib.rs`, complementada por `src/online/mod.rs`.

| Superfície | Situação e decisão |
| --- | --- |
| `markup`, `budget` | Públicos, documentados como internos sem promessa de estabilidade. Usar as funções acessíveis já existentes. |
| `bounded`, `fifo`, `dedup`, `normalize`, `router` | Privados. Manter privados e testar comportamento por `Broker` + `FakeUpstream`. |
| Demais módulos declarados em `src/lib.rs` | `pub mod`, incluindo memória, eval, statusline, estado, sessão, workspace e worktree; visibilidade do símbolo também deve ser conferida. |
| `online::coordinator` | Privado; `OnlineConfig`, `OnlineEngine`, `OnlineTotals` reexportados. |
| `online::merge` | `pub(crate)`. |
| `online::credential`, `online::jev` | Públicos apenas com `cfg(feature = "online")`; módulos puros online compilam no default. |
| Fingerprints e `reference` de `session`, `SessionMemory::{has,remember}` | `pub(crate)`; testar pela sessão/broker. |
| `Snapshot.text` | Campo privado; construir snapshot real em tempdir e acessar por métodos públicos. |

`budget::fill` é declarado `pub`, mas seu argumento usa `normalize::Entry`, cujo módulo
é privado. Portanto P0.11 continua pela costura pública. `estimate_tokens` e `add_memories`
são utilizáveis diretamente. Não é necessário promover mais módulos ou criar testes inline.

## Constantes e limites confirmados

| Fonte | Valores |
| --- | --- |
| `src/broker.rs` | `MIN_BUDGET_TOKENS = 256`, `MIN_ONLINE_BUDGET_TOKENS = 512`, `MAX_BUDGET_TOKENS = 100_000`; padrões 2500/1500/1800 para task/after-edit/before-finish; `max_item_tokens = 800`. |
| `src/broker.rs` | `MIN_RIPWIRE_VERSION = (0,6,4)`; versão ilegível passa em `check_version`. |
| `src/broker.rs` | `GATE_RISKS = ["cochange_missing", "contract_change"]`: exceção à supressão de sessão, não promessa de caber em qualquer orçamento. |
| `src/online/decision.rs` | Admissão estrita `p > 0.25`; seleção estrita `p > 0.50`; `(0.25,0.50]` vira pista de leitura. |
| `src/online/request.rs` | 128 perguntas, 38.000 bytes de JSON, até 8 unidades em seleção; 14 KiB de texto por lote com exceção de unidade única descrita em P0.4. |
| `src/online/reader.rs` | Preview 16 KiB, lookahead 4 KiB, chunk 3 KiB, unidade 24 KiB, location-only acima de 1 MiB, leitura máxima 8 MiB. |
| `src/markup.rs` | `MAX_DEPTH = 64`. |
| `src/notes.rs` | `PROMPT_VERSION = "notes/v1"`, grupos 3, `MAX_EVIDENCE_CHARS = 2000`, nota 600 caracteres, cache 500 entradas. |
| `src/online/cache.rs`, `src/online/prompt.rs` | `POLICY_VERSION = "policy/v1"`, prompt `VERSION = "v1"`, cache 4000 entradas. |
| `src/session.rs`, `src/metrics.rs` | Histórico de fingerprints 5000; requisições recentes 32. |
| `src/budget.rs` | Memória: até 3 itens, 600 tokens, 20% do orçamento solicitado e somente o espaço restante. |

A allowlist `REQUIRED_VERBS` contém `explore`, `from_trace`, `find_symbol`, `fetch_body`,
`impact`, `memory_recall`, `situational_awareness`, `edit_check`, `affected`, `quality_delta`.
Não inclui escrita nem `quality_baseline`.

`budget::estimate_tokens` serializa JSON compacto e usa `s.len().div_ceil(4)` em bytes;
falha de serialização retorna `u32::MAX`. Não é contagem de tokens de um modelo.
`MAX_EVIDENCE_CHARS` é o nome da constante: `notes::evidence` compara `String::len()`
em bytes e permite a primeira linha mesmo acima do limiar. Não usar esse nome como
prova de um teto absoluto de caracteres.

## Contratos P0 e cobertura existente

Preservar os identificadores históricos. Priorizar os controles P0.7, P0.5, P0.12,
P0.1, P0.2 e P0.13; estender testes existentes somente onde falta uma asserção.

| Item | Contrato revalidado | Evidência de teste existente |
| --- | --- | --- |
| P0.1 — workspace | `Workspace::relative(&str) -> Result<String,String>` normaliza lexicalmente, confere ancestral existente e recusa escape da raiz; `check_symbol` aplica isso a seeds `@arquivo:linha`. Oráculo com canonicalização em árvore estável, sem alegar segurança contra toda corrida do filesystem. | `tests/props_fs.rs`: `a_path_the_guard_accepts_always_stays_inside_the_root` e `a_line_seed_obeys_the_same_rule_as_a_path`; testes de workspace também em `tests/broker.rs`. |
| P0.2 — elegibilidade | `snapshot` retorna apenas conteúdo elegível; `files_in` lista nomes e não substitui esse filtro. Um arquivo vazio é elegível. `is_fresh` exige elegibilidade atual e hash igual. | `tests/props_fs.rs`: `asking_for_an_arbitrary_path_never_reads_a_refused_file`, `a_changed_byte_makes_a_snapshot_stale`, tabela de política e teste de listagem. |
| P0.3 — unidades | `units(&Snapshot, &[u64]) -> Vec<Unit>` cobre texto contiguamente e em fronteiras UTF-8, com teto 24 KiB. Linhas longas podem ser divididas no meio; alinhamento de linha não é absoluto. Texto vazio e location-only não geram unidades; linhas de símbolo existentes orientam cortes. | `units_cover_the_text_exactly_once_and_within_the_cap` em `tests/props_fs.rs`. |
| P0.4 — lotes | `batches` respeita 128 perguntas e 38.000 bytes; seleção respeita 8 unidades. Uma unidade sozinha pode exceder 14 KiB de texto, mas nunca o teto JSON. Itens enviados e `too_large` são subsequências da entrada; juntos preservam seu multiconjunto. Concatenar as duas saídas não recupera necessariamente a ordem global. `request_bytes` equivale ao tamanho JSON de `build`. | `every_batch_is_within_every_limit_and_nothing_is_lost`, `request_bytes_is_exactly_the_json_length` em `tests/props.rs`. |
| P0.5 — respostas | `parse_answers(&str, &[String], &str)` retorna `Result<Vec<Option<f64>>, InvalidResponse>`. Modelo divergente, ID desconhecido e ID repetido invalidam a resposta inteira. JSON malformado retorna erro; valor ausente, tipo incompatível ou probabilidade inválida num JSON aceito vira `None`. Ordem é a dos IDs solicitados. | Família `parse_answers` e testes de probabilidades/IDs em `tests/props.rs`; exemplos em `tests/online_units.rs`. |
| P0.6 — limiares | Igualdade ao limiar não passa; `None` permanece desconhecido. `file_decision` usa o maior valor conhecido: fragmento admitido basta para admitir mesmo havendo lacuna; lacuna impede apenas rejeição definitiva. Monotonicidade deve usar probabilidades válidas. | Propriedades de limiares e fragmentos em `tests/props.rs`. |
| P0.7 — redação | `remote_text(&str, Option<&str>, usize) -> String` filtra ASCII imprimível/espaço, substitui a forma imprimível não vazia do segredo por `[redacted]`, aplica trim e corta por bytes. A promessa literal de nunca conter qualquer segredo não vale se o próprio segredo for substring do marcador, como `redacted`. Distinguir substituição de credencial de coincidência com marcador fixo. | `remote_text_*` e `a_tighter_cap_only_ever_removes` em `tests/props.rs`; ampliar domínio/oráculo conforme plano. |
| P0.8 — hashes | `online::cache::key(&KeyParts)` e `notes::key(model_id, scope, evidence)` são determinísticos e usam SHA-256 com prefixos de comprimento. Testar partes, versões e concatenações ambíguas; não prometer prova de ausência de colisões nem que a chave muda somente se a decisão semântica mudar. | Testes de estabilidade e alteração de partes em `tests/props.rs`. |
| P0.9 — notas | `notes::sanitize(&str)` remove controles salvo newline, elimina escapes CSI/OSC, limita a 600 caracteres e é idempotente. `groups` mantém no máximo três grupos na ordem de primeira ocorrência. | Propriedades de sanitize em `tests/props.rs`, exemplos de notas em `tests/notes.rs`. |
| P0.10 — CLI | `cli::parse(Vec<String>) -> Result<Command,String>` é puro; erros incluem `USAGE`. Em serve, `--jev-*` exige online explícito ou implícito por memória. `--online` também é válido em `install` e obrigatório em `memory drain`; não proibir genericamente fora de serve. | Propriedades em `tests/props.rs`, casos dos comandos e memória em `tests/cli.rs`. |
| P0.11 — orçamento | `shown` conta itens+testes+riscos; `shown+omitted` usa candidatos após normalização/dedup e filtragem aplicável, não o payload bruto. `truncated == (omitted > 0)` e `next_step` acompanha truncamento. Limitações são preservadas; se esqueleto e limitações excederem o teto, o envelope pode excedê-lo sem entradas mostradas. Reservas de notas/memória impedem assumir encaixe máximo. | `the_budget_bookkeeping_stays_consistent_at_any_budget` em `tests/broker.rs`; testes de notas/memória e propriedade de `add_memories` em `tests/props.rs`. |
| P0.12 — markup | `markup::parse(&str) -> Option<Node>` deve terminar sem panic e respeitar profundidade limitada; há casos de round-trip e aninhamento acima do teto. | `markup_parse_is_total_and_bounded` e testes adjacentes em `tests/props.rs`. |
| P0.13 — bloco de contexto | `local::wrap(task, &Envelope)` escapa `<`/`>` no JSON como `\u003c`/`\u003e`. Texto dentro do envelope não fecha o bloco. O argumento `task` fica fora do bloco e não é escapado por essa função; a propriedade não é sobre tarefas arbitrárias contendo delimitadores. | `repository_text_can_never_close_its_own_block` em `tests/props.rs`. |

P0.2 tem 12 motivos em `Ineligible`: `Outside`, `SensitiveName`, `Hidden`,
`DependencyOrBuild`, `Symlink`, `NotRegular`, `Unreadable`, `Ignored`, `TooLarge`,
`Binary`, `NotUtf8`, `PrivateKey`. Nome sensível é verificado no componente final;
ocultos e symlinks são conferidos nos componentes percorridos. Binário, UTF-8 e
marcadores de chave são avaliados **depois da leitura**; a garantia é não retornar
snapshot inelegível, não que nenhum byte inelegível seja lido. O descritor aberto é
revalidado com `O_NOFOLLOW`/`O_NONBLOCK`, tipo regular e limite de bytes.

## P1 e memória

- **P1.1:** `online::retry_after::parse(value, now)` aceita segundos decimais ou a
  forma de data implementada em `imf_fixdate`; data passada resulta em zero.
  Inteiro decimal acima de `u64` resulta em `Duration::MAX`. O parser valida campos
  numéricos, mas só confere comprimento e vírgula do dia da semana; não alegar validação
  estrita de toda a gramática HTTP. Propriedades em `props`, regressões em `online_units`.
- **P1.2:** `online::ranked_paths` exclui itens `Role::Doc`, deduplica caminhos,
  ordena por melhor prioridade e primeira posição entre itens não documentais;
  linhas são distintas e ordenadas. Propriedade em `tests/props.rs`.
- **P1.3:** `Scheduler` com `FakeClassifier` verifica concorrência, limites de tentativas,
  associação de respostas, cancelamento, cooldown e divisão de lote. Já há propriedade
  `the_scheduler_keeps_its_limits_and_never_mixes_up_an_answer` em `tests/online_scheduler.rs`.
- **P1.4:** isolamento de features já tem `the_build_has_no_network_stack` em
  `tests/mcp_surface.rs` e passo CA-10 no CI; não duplicar.
- **Memória:** `props` já cobre identidade, sequência de ingestão, probabilidades,
  direções causais, ranking, divisão de expansões e orçamento. `props_fs` cobre admissão
  de caminhos, replay idempotente e exclusão monotônica entre gerações. Há ainda seis
  alvos `memory_*`. Preservar essa cobertura ao ajustar filtros e persistência.

## Inventário e execução dos testes

Os 23 alvos de integração são `broker`, `cli`, `eval`, `hooks`, `mcp_surface`,
`memory_consolidation`, `memory_controller`, `memory_identity`, `memory_policy`,
`memory_retrieval`, `memory_store`, `notes`, `online`, `online_live`, `online_protocol`,
`online_scheduler`, `online_units`, `props`, `props_fs`, `statusline`, `summarizer`,
`upstream_ripwire`, `worktree`. Alvo descoberto não significa testes ativos em toda feature.

| Local das propriedades | Casos configurados | Persistência atual |
| --- | --- | --- |
| `tests/props.rs` | `Config::default()`, respeitando `PROPTEST_CASES`; CI repete com 4096 | `tests/props.proptest-regressions`, versionado |
| `tests/props_fs.rs` | 48, fixados no helper | `None` |
| `tests/broker.rs` — P0.11 | 32 | `None` |
| `tests/online_scheduler.rs` — P1.3 | 40 | `None` |

Logo `binary(props)` não seleciona toda propriedade do projeto. Os dois jobs executam
os quatro alvos na suíte completa; os passos adicionais destacam apenas `props` e `props_fs`.

Há oito atributos `#[ignore]` nos fontes de testes: três testes de provider em
`online_live`, dois em `summarizer` (modelo real e helper de isolamento de ambiente),
um helper em `worktree` e medições em `online` e `cli`. Helpers ignorados podem ser
iniciados explicitamente por outro teste; não tratar todos como chamadas ao provider.
Não usar `--ignored`/`--run-ignored` indiscriminadamente no CI de PR.

Reutilizar `tests/common/`: `FakeUpstream`, `FakeClassifier`, `Counters`,
`FakeSummarizer`, fixtures, auxiliares de workspace/processo e corpus Jev sintético.
Propriedades não chamam provider nem executam conteúdo gerado. Protocolo HTTP de
`tests/online_protocol.rs` usa loopback; é diferente de acesso ao provider real.

## CI e cadeia de suprimentos já implementados

`.github/workflows/rust.yml` dispara em push e PR destinados a `master`:

- `contents: read`, concurrency por workflow/ref, cancelamento de execução anterior;
- jobs estáveis `default` e `online` em `ubuntu-latest`, timeout de 20 minutos,
  passos de suíte com 12 minutos;
- checkout e install-action fixados por SHA completo; checkout com
  `persist-credentials: false`; instalação de nextest;
- default: `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`,
  `cargo nextest run --locked`, propriedades puras com 4096 casos e filesystem à parte;
- online: clippy e nextest com `--locked --features online`;
- CA-10 por `cargo tree --locked -e normal` e regex `reqwest|secrecy|rustls|hyper`;
  guarda de fixtures por regex `Bearer |api_key|authorization|sk-[A-Za-z0-9]{10}`;
- sem cache de build e sem gatilho `pull_request_target`. `cargo fmt` não recebe
  `--locked`; a exigência aplica-se aos comandos que resolvem dependências e o suportam.

A regex de fixtures detecta alguns padrões de credenciais; não prova que todo conteúdo
é sintético nem substitui revisão do corpus. Os testes de grafo CA-10 também incluem
`h2` e `axum`, além dos nomes do passo shell. As propriedades são executadas novamente
no build online pela suíte completa, sem que isso autorize rede externa.

`.github/workflows/supply-chain.yml` executa `cargo deny --all-features check`:
cron `17 6 * * 1`, acionamento manual e push em `master` quando mudam `deny.toml`,
`Cargo.lock`, `Cargo.toml` ou o workflow. Não roda em PR. O job público é `cargo-deny`,
com 15 minutos, permissões mínimas e actions por SHA. A ferramenta cargo-deny não tem
versão fixada, por decisão explícita no arquivo. Cron atua na branch padrão configurada
no GitHub; o filtro local de push aponta para `master`.

`deny.toml` já cobre advisories sem exceções, oito licenças permitidas, versões duplicadas
como aviso, curingas negados e fontes limitadas a crates.io. Comentários com contagens
históricas de pacotes não são inventário atual nem atestado de auditoria limpa hoje.
Dependabot cobre Cargo e GitHub Actions semanalmente e ignora `rust-mcp-sdk` e
`sha2 >= 0.11`. O pin exato do SDK só muda por decisão registrada. `.gitignore` já
ignora `.env` e `.envrc`.

D-107 em `spec/changelog.md` registra checks obrigatórios `default`/`online` com `strict`
e dispensa de revisão humana obrigatória no modelo de manutenção então adotado.
Preservar os nomes; conferir a configuração remota em etapa operacional própria.
Não converter ausência de consulta remota em afirmação de proteção ausente.

Não há workflow de release, upload de binários ou deploy nesta árvore. O escopo atual
é integração contínua e cadeia de suprimentos; CD de distribuição requer proposta
separada com destinos, plataformas e política de release definidos.

## Trabalho restante e critérios de aceitação

1. Persistir seeds de `props_fs`, P0.11 e P1.3 com caminhos explícitos, mantendo
   os tetos 48/32/40. O arquivo de `props` já existente permanece versionado.
2. Tornar o contrato de P0.7 testável sem falsa promessa sobre substrings do marcador;
   acrescentar casos dirigidos de segredo curto, Unicode, marcador e truncamento.
3. Tornar visíveis os quatro locais de PBT nos comandos/documentação do CI, mantendo
   4096 casos restritos ao alvo puro. Evitar repetir toda suíte apenas para exibir nomes.
4. Fazer os passos CA-10 e fixtures falharem também se a inspeção falhar. Hoje usam
   comandos em condição `if` e podem tratar erro de ferramenta como ausência de match.
   Distinguir sucesso sem ocorrência, ocorrência proibida e falha operacional.
5. Verificar remotamente os checks obrigatórios antes de qualquer alteração futura
   nos nomes dos jobs; conservar a decisão de revisão humana registrada em D-107.
6. Após a implementação futura, rodar fmt, clippy e suíte nas duas configurações;
   verificar replay de seeds, CA-10 e guardas; registrar resultados, falhas e decisões
   no changelog sem reutilizar números históricos como se fossem novos.

Cache, SBOM e harden-runner continuam opcionais. Cache foi deliberadamente dispensado
em D-107; só reconsiderar após medir benefício e definir isolamento entre PR e branch
protegida. Não são bloqueadores desta proposta.

Raízes de filesystem sempre em tempdir; caminhos gerados são consultas, nunca destinos
arbitrários de escrita/remoção. Conteúdo gerado não vira executável. Limitar volume por
caso e cobrir arquivos grandes com casos dirigidos. Seeds e fixtures devem ser sintéticos,
sem credenciais, caminhos privados ou código de workspace real.
Não exportar a chave Jev no CI de PR. Não reimplementar SDK, TLS/DNS ou golden tests como PBT.

## Verificação desta revisão documental

Foram lidos manifesto, toolchain, fontes relevantes, testes, workflows, configuração de
supply chain e decisões históricas. `cargo metadata --offline --locked --no-deps` confirmou
os alvos e ausência de MSRV declarado. `cargo tree --offline --locked -e normal` não
contém `reqwest`, `secrecy`, `rustls`, `hyper` ou `h2`; o mesmo comando com `--features online`
contém essas dependências. A inspeção com a regex do CI não encontrou correspondência
em `tests/fixtures/`.

A execução focal de propriedades e seus resultados estão registrados no plano.
Esta revisão não executou toda a suíte, clippy, nextest, cargo-deny com base de advisories
atualizada nem consultou a proteção remota. Não afirma aprovação desses controles.
