# Memória persistente com Jev (`--memory`) — Plano de implementação

**Data:** 2026-10-04 · **Status:** plano validado contra o código; Fases 0 a 5 feitas
([D-136](../changelog.md#d-136--fase-0-do---memory-prd-jev-mem-v03),
[D-137](../changelog.md#d-137--fase-1-do---memory-store-e-coleta),
[D-138](../changelog.md#d-138--fase-2-do---memory-controle-jev),
[D-139](../changelog.md#d-139--fase-3-do---memory-leitura-e-entrega),
[D-140](../changelog.md#d-140--fase-4-do---memory-consolidação),
[D-142](../changelog.md#d-142--fase-5-do---memory-avaliação)), com a T3.11 (hosts) e a T5.3 (rodada
da avaliação) pendentes e manuais; Fase 6 não começada.
**Spec:** [`docs/jev-mem-prd.md`](../../docs/jev-mem-prd.md) v0.3. Onde este plano diz "PRD §N", é esse documento;
"PRD principal" é [`spec/ripwire-broker-mcp.md`](../ripwire-broker-mcp.md). "CA-N" é o critério
verificável N do PRD §14.
**Base:** `master` em `60c77b9`, pacote `0.1.0`, Rust `1.98.1`, edition 2024.
**Decisão que registra este plano:** [D-134](../changelog.md#d-134--plano-de-implementação-do---memory).

**Objetivo:** memória de trabalho entre sessões, isolada por workspace/worktree, atrás de `--memory`
(que implica `--online`): os hooks e as tools de edição/conclusão publicam observações estruturadas
num store local; um worker autorizado as enriquece com Jev; `context_for_task` recupera até 3
memórias dentro do orçamento existente. Sem `--memory`, nada muda.

**Arquitetura:** módulo novo `src/memory/` com o domínio puro e a persistência no build padrão
(sem rede), e o adaptador Jev sob `cfg(feature = "online")`. O envelope ganha `memories[]` e
`provenance.memory`, omitidos quando vazios. Nenhuma dependência nova (PRD §3).

---

## 1. Regra de trabalho: TDD, e o que é "feito"

Vale para **toda** tarefa deste plano, sem exceção. Uma tarefa só recebe `[x]` quando os seis
passos abaixo aconteceram, nesta ordem, e a linha dela no [registro de evidência](#9-registro-de-evidência)
está preenchida.

1. **Vermelho.** Escrever o teste indicado, na costura pública (`tests/`), e **rodá-lo antes de
   qualquer código de produção**. Ele tem de falhar pelo motivo esperado: a asserção nomeada na
   tarefa, ou o erro de compilação do símbolo que a tarefa cria. Um teste que já nasce verde está
   errado ou testa outra coisa: parar e corrigir o teste. Guardar a linha da falha.
2. **Verde.** O menor código que faz esse teste passar. Nada de código para tarefas futuras, nada
   de refatoração junto. Se o verde exigir mexer fora dos arquivos listados na tarefa, a tarefa
   estava mal recortada: registrar e ajustar o plano antes de seguir.
3. **Mutação.** Quebrar de propósito a linha que o teste protege (inverter a condição, trocar o
   limite) e ver o teste falhar; restaurar e dar `touch` no fonte (armadilha do mtime, `handoff.md`).
   É a convenção do repositório e a prova de que o teste morde.
4. **Refatoração**, só com tudo verde, e só se houver o que limpar.
5. **Documentação afetada**, na mesma tarefa: os itens da coluna **Docs** da tarefa, mais a entrada
   `D-NNN` em `spec/changelog.md` quando a tarefa fecha uma decisão. Documentação que descreve um
   comportamento só entra depois que o teste dele está verde.
6. **Portões locais**, todos verdes, sem pipe (um pipe devolve o status do último comando):

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo clippy --all-targets --locked --features online -- -D warnings
cargo test --all-targets --locked
cargo test --all-targets --locked --features online
```

Regras que acompanham:

- **Um teste por comportamento, uma tarefa por teste (ou grupo coeso).** O teste é nomeado como
  frase, em inglês, como o resto da suíte.
- **Nada de testar função privada.** Se o comportamento não é alcançável pela costura pública, a
  costura está errada.
- **Teste de tempo com folga pequena acima do limite**, e mutação para provar que passa pela regra
  e não pela carga (`handoff.md`, D-129).
- **Tarefa sem código** (documental ou manual, marcada `doc` ou `manual`): o "vermelho" é uma
  verificação executável que falha antes e passa depois (um `grep`, um `shasum`, um roteiro com
  saída gravada). Sem verificação, não é tarefa: é intenção.
- **Um PR por fase** (o `master` é protegido, entra por squash). Commits em inglês; este plano e
  `spec/` em português.
- **`cargo nextest` e `cargo deny` não estão instalados nesta máquina.** Localmente vale
  `cargo test`; o CI (`.github/workflows/rust.yml`) roda nextest e é quem decide. As propriedades
  com 4096 casos rodam local com `PROPTEST_CASES=4096 cargo test --locked --test props`.

## 2. Validação do plano contra o PRD e o código

Feita em 2026-10-03, lendo o PRD inteiro e as fontes do PRD §13. Cada achado virou tarefa ou
decisão pendente; nenhum ficou só aqui.

| # | Achado | Evidência no código/repositório | Consequência |
|---|---|---|---|
| V1 | O PRD §1 dá o estudo como ausente; ele **está** no checkout | `docs/jev-mem.md`, `docs/2026-10-03_Jev-Mem.pdf`; SHA-256 confere com o do PRD (`413c5924…5e87`) | T0.1 fecha a pendência antes de qualquer código |
| V2 | O estudo e o PRD divergem em três pontos | estudo §4.3 usa mtime/commit como tempo, §4.4 propõe `role: memory`, §4.1 põe a flag só no `serve` | o PRD vence (§5.4, §11, §4); T0.1 registra |
| V3 | `memory add` aparece no PRD §5.2 e não na tabela do §4 | — | PD-1 (aceita) |
| V4 | `memory drain … --online` colide com a regra atual | `src/cli.rs:368` recusa `--online` fora do `serve` (D-064); `tests/cli.rs::hook_and_prompt_reject_online` | T2.11 muda a regra com teste próprio; PD-2 |
| V5 | O PRD não diz como o **hook** sabe que a memória está ligada | `HookArgs` não tem flag de memória; hook recusa `--online` | PD-3 (proposta: `hook --memory`, só spool local) |
| V6 | `forget --all` "revoga a autorização até nova ativação explícita", sem dizer qual | — | PD-4 (aceita): `memory resume` |
| V7 | `Classifier::classify` devolve `Vec<Option<f64>>`; só existe Noul | `src/online/classifier.rs`, `request.rs` (`JevQuestionType::Noul`), `response.rs::parse_answers` | Fase 2 começa por testes de caracterização do que existe |
| V8 | Limite de perguntas da memória (32) difere do atual (128) | `src/online/request.rs::MAX_QUESTIONS` | constante própria em `memory`, sem tocar a atual |
| V9 | `write_private` sincroniza o arquivo, não o diretório | `src/state.rs:128` (`sync_all` + `rename`) | T1.7 cria a escrita durável do store; `write_private` não muda |
| V10 | O texto MCP hoje **é** o JSON do envelope | `src/mcp.rs:373` (`structured.to_string()`) | a seção legível do PRD §11 exige mudar o bloco de texto só quando há memória; T3.8 |
| V11 | `carries_content`/`has_news` ignoram campos novos | `src/hook.rs:378`, `:383` | T3.9 |
| V12 | O portão de fixtures do CI recusa `Bearer`, `api_key`, `authorization`, `sk-…` em `tests/fixtures/` | `rust.yml`, passo "Fixtures stay synthetic" | os dados sensíveis do CA-3 são gerados **dentro** do teste, nunca em arquivo de fixture |
| V13 | O PRD principal §23.1 proíbe hoje um worker remoto fora de `context_for_task` | PRD §16 | a Fase 2 só entra com o PRD principal alterado no mesmo PR (T2.12) |
| V14 | Choice no modelo pinado nunca foi chamado | PRD §17 | T2.0 (teste de contrato opt-in) antes do resto da Fase 2; se falhar, tempo implícito e consolidação por Choice ficam bloqueados |
| V15 | O domínio da memória precisa compilar sem a feature `online` | `src/online/mod.rs` só protege `credential` e `jev` com `cfg`; `reader`/`redact` compilam no build padrão | `memory status`/`forget`/`add` e a coleta funcionam no build padrão; CA-10 preservado |
| V16 | Os braços do eval são um `enum` fechado | `src/eval/arm.rs::Arm` (4 variantes) | T5.1 acrescenta dois, com contaminação própria |

### Decisões do mantenedor

As cinco propostas foram **aceitas pelo mantenedor em 2026-10-03**
([D-135](../changelog.md#d-135--decisões-pd-1-a-pd-5-do---memory)). Nenhuma tarefa está mais
bloqueada por elas, e as superfícies que elas criam já estão no PRD §4 (T0.2, D-136).

| ID | Pergunta | Decisão | Tarefa |
|---|---|---|---|
| PD-1 | `memory add` entra neste incremento? | Sim, na Fase 1, local e sem rede, como no PRD §5.2 | T1.13 |
| PD-2 | `memory drain` aceita `--online` apesar do D-064? | Sim: `--online` passa a valer para `serve` e `memory drain`, e só | T2.11 |
| PD-3 | Como o hook liga a coleta? | `hook … --memory`: publica no spool, nunca faz HTTP, não implica `--online`; o instalador grava a flag só com `install --memory` | T1.15, T1.16 |
| PD-4 | O que reativa a coleta depois de `forget --all`? | Marcador `revoked` no store; só `memory resume --workspace PATH` o remove (comando novo, local) | T1.12 |
| PD-5 | A Fase 2 pode começar antes de fechar as medições do `--online` (handoff, A/B)? | Sim, atrás de `--memory`, declarada experimental | Fase 2 |

## 3. Restrições globais

Do PRD e das convenções do repositório; valem em todas as tarefas.

- **Sem `--memory`, saída idêntica à de hoje** (CA-1). A primeira tarefa de cada fase que toca uma
  superfície compartilhada começa por um teste de caracterização da saída atual.
- **Nenhuma dependência nova.** `cargo tree --locked -e normal` sem `reqwest|secrecy|rustls|hyper`
  no build padrão (CA-10 do PRD principal; teste `the_build_has_no_network_stack`).
- **Hook nunca faz HTTP** e nunca espera worker (PRD §8.1).
- **Três tools MCP, mesmos nomes e argumentos.** Nenhuma tool de escrita livre na memória.
- **Nada de conteúdo em log:** só contagens, tempos e IDs opacos (PRD §14).
- **Credencial só em `RIPWIRE_BROKER_JEV_API_KEY`**, nunca em arquivo, argumento ou cache.
- **Unix apenas** neste incremento (PRD §6).
- **Stdout do `serve` é só protocolo.** `#![deny(clippy::print_stdout)]` já protege a lib.
- **Ausência nunca vira zero:** resposta faltante é `None`/`Unknown`, em todo o caminho.

## 4. Mapa de arquivos

Proposta do PRD §13, confirmada contra o código. "novo" não existe hoje.

| Arquivo | Papel | Fase |
|---|---|---|
| `src/memory/mod.rs` (novo) | API interna: `admit`, `enqueue`, `drain`, `retrieve`, `forget`, `status` | 1 |
| `src/memory/model.rs` (novo) | registro `memory/v1`, limites, serialização | 1 |
| `src/memory/identity.rs` (novo) | `workspace_id`, entidades, `content_hash`, `node_id` | 1 |
| `src/memory/admission.rs` (novo) | elegibilidade, varredura de segredo/PII, renderizador `memory-observation/v1` | 1 |
| `src/memory/store.rs` (novo) | spool, snapshot, lock, limites, retenção, tombstones, escrita durável | 1 |
| `src/memory/queue.rs` (novo) | jobs, leases, tentativas, ledger de quota de 24 h | 1–2 |
| `src/memory/prompts.rs` (novo) | `memory-prompts/v1` | 2 |
| `src/memory/controller.rs` (novo, `cfg(feature = "online")`) | worker: typing, candidatos, relações, commit por par | 2 |
| `src/memory/index.rs` (novo) | tokenização, idf, entidades, RRF | 3 |
| `src/memory/retrieve.rs` (novo) | routing, expansão, scoring, parada | 3 |
| `src/memory/consolidate.rs` (novo) | cadência, pares, `RepresentationDecision`, nota derivada | 4 |
| `src/memory/metrics.rs` (novo) | contadores por workspace/operação | 1–4 |
| `src/cli.rs`, `src/main.rs` | `--memory`, `--memory-*`, subcomando `memory` | 1–2 |
| `src/online/{request,response,jev,classifier}.rs` | Choice, `Decision`, transporte comum | 2 |
| `src/broker.rs` | publicação nas tools de edição/conclusão; leitura em `context_for_task` | 1, 3 |
| `src/model.rs`, `src/budget.rs`, `src/session.rs`, `src/mcp.rs`, `src/hook.rs` | `memories[]`, orçamento, fingerprint, texto, `carries_content` | 3 |
| `src/install.rs`, `src/doctor.rs` | opt-in, prévia, diagnóstico local | 1 |
| `src/eval/arm.rs`, `src/eval/report.rs`, `src/bin/ripwire-eval.rs` | braços B e C | 5 |
| `tests/memory_{policy,identity,store,controller,retrieval,consolidation,hosts}.rs` (novos) | uma costura por arquivo | 1–4 |
| `tests/{cli,broker,hooks,mcp_surface,online_units,online_protocol,eval,props,props_fs}.rs` | suítes existentes, estendidas | 1–5 |

Antes de tocar `Envelope`, `Classifier`, `NoteEngine`, `SessionMemory` ou os hooks, mapear os
consumidores (`ripwire --impact=SYM`, `graft callers SYM --depth all`) e listar na tarefa os
arquivos atingidos (PRD §13, último parágrafo).

---

## 5. Tarefas

Formato: **Vermelho** é o teste que falha primeiro (arquivo e nome); **Verde** é o código mínimo;
**Docs** é o que tem de estar atualizado para a tarefa contar como feita. Os seis passos do §1
valem para todas.

### Fase 0 — Contratos (sem código de produção)

- [x] **T0.1 · doc · Reconciliar o PRD com o estudo presente.**
  **Vermelho:** `shasum -a 256 docs/2026-10-03_Jev-Mem.pdf` confere com o PRD §1, e
  `grep -n "não foram encontrados" docs/jev-mem-prd.md` ainda encontra a pendência.
  **Verde:** confrontar `docs/jev-mem.md` com o PRD §2; reescrever o §1 (pendência fechada), o §2.2
  (última linha) e o §17; registrar V2. O `grep` deixa de encontrar.
  **Docs:** PRD v0.3; `D-NNN`.
- [x] **T0.2 · doc · Levar PD-1 a PD-5 (aceitas, D-135) ao PRD.**
  **Vermelho:** `grep -n "memory resume\|hook .*--memory\|install .*--memory" docs/jev-mem-prd.md`
  não encontra nada, e `memory add` não está na tabela do §4.
  **Verde:** o PRD §4 ganha as linhas que faltam (`memory add`, `memory resume`, `hook --memory`,
  `install --memory`) e a exceção de `memory drain --online` ao D-064; o `grep` passa a encontrar.
  **Docs:** PRD, changelog, este plano.

### Fase 1 — Store e coleta (build padrão, sem rede)

Ainda sem anunciar memória Jev: só coleta, persistência e comandos locais.

- [x] **T1.1 · `--memory` no parse, implicando online.** CA-2.
  **Vermelho:** `tests/cli.rs::memory_implies_online_and_both_flags_are_equivalent`: as quatro
  combinações do PRD §4; `--memory` e `--online --memory` produzem `ServeArgs` iguais, com
  `online: Some(..)` e `memory: Some(..)`; a origem do online é `Implied` ou `Explicit`; sem
  flags, `memory == None` e `online == None`. Falha por não existir `ServeArgs::memory`.
  **Verde:** `MemoryArgs` e `OnlineOrigin` em `src/cli.rs`; `--memory` em `SWITCHES` e nas flags
  do `serve`; `Flags::online` trata `--memory` como `--online`.
  **Docs:** `USAGE` em `src/cli.rs`.

  ```rust
  #[test]
  fn memory_implies_online_and_both_flags_are_equivalent() {
      let Ok(Command::Serve(off)) = parse(&["--workspace", "/w"]) else { panic!() };
      assert_eq!((off.online, off.memory), (None, None), "offline, no memory, by default");

      let Ok(Command::Serve(only)) = parse(&["--workspace", "/w", "--online"]) else { panic!() };
      assert!(only.online.is_some() && only.memory.is_none(), "--online alone keeps no history");

      let Ok(Command::Serve(m)) = parse(&["--workspace", "/w", "--memory"]) else { panic!() };
      let Ok(Command::Serve(both)) = parse(&["--workspace", "/w", "--online", "--memory"]) else {
          panic!()
      };
      assert!(m.online.is_some(), "--memory implies --online (PRD jev-mem §4)");
      assert_eq!(m.online, both.online);
      assert_eq!(m.memory.as_ref().map(|a| a.read_deadline), Some(Duration::from_millis(750)));
      assert_eq!(m.online_origin, Some(cli::OnlineOrigin::Implied));
      assert_eq!(both.online_origin, Some(cli::OnlineOrigin::Explicit));
  }
  ```

- [x] **T1.2 · Faixas das opções `--memory-*`.** CA-2.
  **Vermelho:** `tests/cli.rs::memory_options_have_defaults_and_refuse_values_out_of_range`:
  defaults 750 / 4 / 4 / 30 / 2000; recusa `--memory-read-deadline-ms 751`, `…-request-limit 5`,
  `…-write-candidates 11`, `…-retention-days 0` e `366`, `…-max-nodes 2001`; aceita
  `…-request-limit 0`; `--memory-*` sem `--memory` é erro de uso; `hook`, `prompt` e `doctor`
  recusam as opções.
  **Verde:** leitura e validação em `Flags`. **Docs:** `USAGE`, README (tabela de flags).
- [x] **T1.3 · Sem a feature, `--memory` falha claro; sem credencial, não há downgrade.** CA-2.
  **Vermelho:** `tests/cli.rs::a_build_without_the_online_feature_refuses_memory_clearly`
  (`cfg(not(feature = "online"))`, código 2, stdout vazio) e
  `memory_without_a_credential_fails_before_publishing_mcp` (`cfg(feature = "online")`).
  **Verde:** `settings` em `src/main.rs` valida a configuração efetiva antes de subir o upstream;
  a mensagem nomeia `--memory`. **Docs:** README.
- [x] **T1.4 · Registro `memory/v1` e seus limites.** CA-3.
  **Vermelho:** `tests/memory_policy.rs::a_record_round_trips_and_an_oversized_one_is_refused_whole`:
  ida e volta por JSON; `content` com 2.001 bytes, 17 entidades, 17 fontes, 9 referências
  temporais ou registro serializado acima de 16 KiB devolvem `Rejected(reason)` sem truncar;
  `types` ausentes são `None`, nunca `0.0`.
  **Verde:** `src/memory/model.rs`, `pub mod memory` em `src/lib.rs`. **Docs:** PRD §5.1 confere.
- [x] **T1.5 · Identidade: workspace, entidades, hashes.** CA-4.
  **Vermelho:** `tests/memory_identity.rs`: `two_worktrees_of_one_repository_have_different_workspace_ids`
  (repositório real com `git worktree add`, via `common::sample_repo`);
  `homonymous_symbols_in_different_files_never_share_an_entity`;
  `a_symbol_without_an_unambiguous_descriptor_falls_back_to_the_file_entity`;
  `a_rename_without_evidence_yields_a_new_entity`; `the_node_id_ignores_clock_scores_and_retries`.
  Em `tests/props.rs`: `ids_never_collide_for_ambiguous_tuples` (o hash leva o comprimento de cada
  componente: `("ab","c")` ≠ `("a","bc")`).
  **Verde:** `src/memory/identity.rs`, reutilizando `Workspace` e `sha2`.
  **Docs:** — (contrato já no PRD §5.3).
- [x] **T1.6 · Tempo: papel explícito, nada inferido.** CA-5.
  **Vermelho:** `tests/memory_identity.rs::mtime_commit_and_a_clock_rollback_never_become_event_time`:
  observação automática tem `timestamp_role = observation` e `event_time = None`; mudar o mtime
  do arquivo ou a data do commit não muda o registro; com o relógio regredido, `ingest_seq`
  continua crescendo e nenhuma duração negativa aparece. Em `props`:
  `ingest_sequence_is_strictly_monotonic`.
  **Verde:** relógio injetável no store; sequência persistida. **Docs:** —.
- [x] **T1.7 · Admissão e renderizador `memory-observation/v1`.** CA-3.
  **Vermelho:** `tests/memory_policy.rs`: `forbidden_sources_never_reach_the_spool` (`.env`,
  arquivo ignorado, binário, symlink para fora, `../` — os valores com cara de segredo são
  montados **no teste**, V12); `a_secret_or_pii_shaped_value_rejects_the_whole_record_and_is_only_counted`;
  `the_rendering_is_deterministic_and_never_claims_tests_passed` (o texto do exemplo do PRD §5.2,
  byte a byte; ausência de regressão não vira "testes passaram"). Em `props_fs`:
  `no_generated_path_escapes_the_workspace_into_a_record`.
  **Verde:** `src/memory/admission.rs` sobre `online::reader::WorkspaceReader` e
  `online::redact`. **Docs:** README (o que nunca é guardado).
- [x] **T1.8 · Store: escrita durável, permissões, leitura segura.** CA-7.
  **Vermelho:** `tests/memory_store.rs`: `the_store_is_private_and_lives_outside_the_repository`
  (0700/0600, sob `<state-dir>/memory/<workspace_id>/`); `a_symlink_or_a_foreign_owner_reads_as_unavailable`;
  `an_unknown_schema_or_a_corrupt_snapshot_is_never_overwritten` (bytes do arquivo iguais depois
  da tentativa); `a_reader_never_observes_a_partial_generation` (leitor em laço durante 200
  publicações).
  **Verde:** `src/memory/store.rs`: temporário privado, `sync_all`, `rename`, `sync` do diretório
  (V9); abertura com `O_NOFOLLOW | O_NONBLOCK`. **Docs:** —.
- [x] **T1.9 · Spool → snapshot, idempotente, com tetos.** CA-4, CA-7.
  **Vermelho:** `tests/memory_store.rs`: `replaying_the_same_observation_yields_one_node_and_one_increment`;
  `a_crash_between_commit_and_spool_removal_does_not_duplicate` (falha injetada em cada passo:
  antes do rename, depois do rename, antes de remover o spool); `a_full_store_refuses_new_writes_with_a_reason`
  (2.000 nós; spool 1.000 entradas; os tetos de bytes com limites injetados menores);
  `a_second_writer_does_not_wait_and_reports_the_lock`. Em `props_fs`: `replay_is_idempotent`.
  **Verde:** ingestão numa geração nova, lock de arquivo por workspace, limites.
  **Docs:** —.
- [x] **T1.10 · Retenção e relógio regressivo.** CA-11.
  **Vermelho:** `tests/memory_store.rs`: `expired_nodes_take_their_edges_jobs_and_derived_notes_with_them`;
  `a_derived_note_never_outlives_its_parents`; `a_clock_rollback_suspends_expiry_by_age_but_keeps_the_caps`.
  **Verde:** varredura no início do worker e a cada hora ativa. **Docs:** README (retenção).
- [x] **T1.11 · `forget` e a não ressurreição.** CA-7, CA-11.
  **Vermelho:** `tests/memory_store.rs`: `forget_by_id_removes_the_node_its_descendants_and_blocks_reingestion`;
  `an_old_spool_entry_cannot_resurrect_a_forgotten_node`; `forget_leaves_no_temporary_or_backup_with_text`
  (varre o diretório por um marcador único do conteúdo). Em `props_fs`:
  `deletion_is_monotonic_across_generations`.
  **Verde:** tombstones e geração. **Docs:** README (o que `forget` não promete: cópias do SO e
  dados já enviados ao provider).
- [x] **T1.12 · `forget --all` revoga a coleta; `memory resume` a reativa.** CA-11. PD-4.
  **Vermelho:** `tests/memory_store.rs::after_forget_all_nothing_is_collected_until_resumed` e
  `tests/cli.rs::memory_resume_is_local_and_clears_the_revocation`.
  **Verde:** marcador de revogação; subcomando `memory resume`, local e sem rede. **Docs:** PRD §6, README.
- [x] **T1.13 · Subcomandos locais `memory status | forget | add`.** CA-2. PD-1.
  **Vermelho:** `tests/cli.rs`: `memory_subcommands_parse`; `memory_status_and_forget_need_no_network_credential_or_feature`
  (binário rodado sem a variável de ambiente, com um `ripwire` inexistente: não inicia upstream nem
  cliente; `status --json` traz filas, schema, tamanhos e o último erro categorizado);
  `memory_add_refuses_a_forbidden_input_path_and_untrusted_text_stays_data`.
  **Verde:** `Command::Memory` em `src/cli.rs`; despacho em `src/main.rs` antes de `settings`.
  **Docs:** `USAGE`, README (seção nova "Memory").
- [x] **T1.14 · As tools de edição e conclusão publicam observações.** CA-1, CA-6.
  **Vermelho:** `tests/broker.rs`: primeiro a caracterização
  `without_memory_the_three_tools_answer_exactly_as_before` (envelope serializado igual, nenhum
  diretório `memory/` criado); depois `after_edit_and_before_finish_publish_one_observation_after_the_envelope`
  e `a_slow_spool_reports_enqueue_unconfirmed_and_never_delays_the_envelope` (store lento
  injetado; a resposta volta dentro de 25 ms além da linha de base).
  **Verde:** `BrokerConfig::memory`; publicação depois do envelope estrutural, em executor
  limitado. **Docs:** README; PRD principal fica para a T2.12/T6.2.
- [x] **T1.15 · O hook coleta sem rede.** CA-6. PD-3.
  **Vermelho:** `tests/hooks.rs`: `a_hook_with_memory_enqueues_and_never_opens_a_socket` (o
  processo roda sem credencial, e uma porta local de sentinela nunca recebe conexão);
  `a_short_lived_hook_leaves_a_recoverable_queue`; `ripwire_off_stops_capture_for_that_session_only`.
  Medição do SLO (p95 ≤ 10 ms, p99 ≤ 25 ms em release): teste `#[ignore]`
  `the_hook_overhead_meets_the_slo`, rodado à mão e registrado.
  **Verde:** `HookArgs::memory`; `src/hook.rs` chama `memory::enqueue`. **Docs:** `USAGE`, README.
- [x] **T1.16 · `install`, `doctor` e a prévia dizem os dois efeitos.** CA-2.
  **Vermelho:** `tests/cli.rs`: `install_with_memory_writes_the_flag_and_never_the_key` (a prévia
  nomeia persistência local **e** envio de histórico; não acrescenta `--online` redundante);
  `doctor_reports_the_memory_store_without_using_the_network`.
  **Verde:** `InstallArgs::memory`, checagem nova em `src/doctor.rs`.
  **Docs:** `USAGE`, README, `integrations/` (exemplos e skill).

**Saída da Fase 1:** CA-1 a CA-7 e CA-11 cobertos no que não depende de Jev; `handoff.md` e a
tabela de fases atualizados; PR único.

**Como a Fase 1 saiu ([D-137](../changelog.md#d-137--fase-1-do---memory-store-e-coleta)):** a T1.7 veio antes da
T1.6, cujo vermelho precisa do construtor de observações; o `serve --memory` ainda não liga a
publicação das tools (passou para a T2.11, junto com o worker); os testes de processo do hook
ficaram em `tests/cli.rs`, ao lado da infraestrutura e2e, e rodam só com o ripwire real.

### Fase 2 — Controle Jev (feature `online`)

- [x] **T2.0 · manual · Choice existe no modelo pinado.** V14.
  **Vermelho:** `tests/online_live.rs::the_pinned_model_answers_a_choice` (`#[ignore]`, sintético,
  precisa da chave) — ainda não existe. **Verde:** escrito e rodado uma vez à mão; o resultado e a
  versão vão para o changelog. Se falhar, T2.7 (tempo implícito) e a Fase 4 por Choice param.
- [x] **T2.1 · Caracterização do protocolo atual.**
  **Vermelho → verde imediato não vale:** confirmar que `tests/online_protocol.rs` e
  `tests/online_units.rs` cobrem o corpo exato do request Noul e o parse atual; onde não cobrem,
  acrescentar o teste e prová-lo por mutação antes de mexer no transporte.
- [x] **T2.2 · Request com Choice e estado de memória.** CA-9.
  **Vermelho:** `tests/online_units.rs::a_choice_question_serializes_type_instructions_and_criteria`
  (corpo igual à fixture `tests/fixtures/jev/memory_choice_request.json`);
  `a_memory_request_is_split_before_32_questions_or_38000_bytes_and_never_cuts_a_pair`.
  **Verde:** `JevQuestionType::Choice`, payload de estado independente de `SemanticStage`.
- [x] **T2.3 · Respostas tipadas: `Decision`.** CA-8.
  **Vermelho:** `tests/online_units.rs`: `a_decision_is_noul_choice_or_unknown_and_never_zero`;
  uma tabela de casos recusados: modelo errado, ID desconhecido ou repetido, tipo trocado, `NaN`,
  infinito, fora de `[0,1]`, Choice sem a opção escolhida, com opção a mais, soma fora de `1e-3`
  (sem renormalizar). Erro de modelo/ID invalida o lote; campo inválido só a sua decisão. O gate
  usa `probabilities[choice]`, não `confidence`. Em `props`:
  `no_parsed_probability_is_ever_outside_the_unit_interval`.
  **Verde:** `parse_decisions` em `src/online/response.rs`; `parse_answers` intacto.
- [x] **T2.4 · Transporte comum, adaptador Noul preservado.** CA-9.
  **Vermelho:** `tests/online_protocol.rs::the_memory_transport_posts_through_the_same_client`
  (mesmo endpoint, Bearer, sem redirect/proxy, teto de 256 KiB, uma única conexão para descoberta
  e memória). Os testes existentes do arquivo seguem verdes sem edição.
  **Verde:** método de envio tipado em `JevClient`; `Classifier::classify` passa a usá-lo.
- [x] **T2.5 · Prompts `memory-prompts/v1`.** CA-9.
  **Vermelho:** `tests/memory_controller.rs::every_stage_names_its_state_fields_and_carries_the_untrusted_guidance`
  (as oito etapas do PRD §7; nenhuma pergunta cita a resposta de outra; versão distinta de
  `notes/v1`). **Verde:** `src/memory/prompts.rs`.
- [x] **T2.6 · Fila durável: jobs, leases, tentativas, quota.** CA-6, CA-10.
  **Vermelho:** `tests/memory_store.rs`: `an_abandoned_lease_is_recovered_by_the_lock_not_by_a_pid`;
  `a_job_runs_at_most_twice_and_then_stays_failed_until_asked`;
  `the_24h_ledger_survives_a_restart_and_a_clock_rollback` (1.000 tentativas, 20.000 perguntas,
  com tetos injetados menores; retry e split consomem). **Verde:** `src/memory/queue.rs`.
- [x] **T2.7 · Worker: typing, candidatos, relações.** CA-9, CA-10.
  **Vermelho:** `tests/memory_controller.rs`, com um `Classifier` de teste (`tests/common/classifier.rs`):
  `a_node_exists_before_any_inference`; `candidates_are_deterministic_and_capped_at_k`
  (0, 4 e 10; empate por ID); `write_cost_matches_the_formula` (`4 + 4K + A` Nouls e `T` Choices,
  para K = 0, 4, 10); `an_edge_needs_060_and_absence_is_not_zero`;
  `caused_by_and_causes_point_in_opposite_directions`; `alias_is_asked_only_when_ids_do_not_intersect_and_never_merges_ids`;
  `implicit_time_is_a_choice_and_unknown_adds_no_edge`. Em `props`:
  `the_two_causal_directions_are_never_confused`.
  **Verde:** `src/memory/controller.rs`.
- [x] **T2.8 · Commit por par, atômico; resposta tardia descartada.** CA-7.
  **Vermelho:** `tests/memory_controller.rs`: `a_timeout_in_the_middle_of_a_pair_commits_nothing_of_that_pair`;
  `a_late_answer_for_an_old_generation_is_dropped`; `an_answer_for_a_forgotten_node_does_not_recreate_it`;
  `the_enrichment_counter_increments_exactly_once_per_node`.
  **Verde:** transação por par com digest e geração.
- [x] **T2.9 · Falhas do provider.** CA-9.
  **Vermelho:** `tests/memory_controller.rs` com o servidor local de `tests/online_protocol.rs`:
  `auth_failures_suspend_the_worker_until_reauthorized` (401/403);
  `a_429_waits_only_inside_the_budget`; `a_5xx_is_retried_once_inside_the_four_attempts`;
  `cancellation_stops_http_and_leaves_the_job_pending`; `no_model_swap_and_no_silent_heuristic_on_failure`.
  **Verde:** política de retry do worker.
- [x] **T2.10 · Concorrência e tetos compartilhados.** CA-2, CA-10.
  **Vermelho:** `tests/online_scheduler.rs::memory_and_discovery_share_four_requests_and_one_client`;
  `tests/memory_controller.rs::one_remote_job_per_workspace`. **Verde:** o teto `max_in_flight`,
  hoje do `Scheduler` de descoberta (`src/online/scheduler.rs`), passa a ser um só para o processo.
- [x] **T2.11 · Ciclo de vida: worker no `serve`, `memory drain`.** CA-2, CA-6. PD-2.
  **Vermelho:** `tests/cli.rs`: `memory_drain_needs_online_and_a_credential`;
  `online_alone_never_processes_old_memory_jobs`; `the_server_leaves_no_process_with_the_credential_on_exit`;
  `memory_and_online_memory_start_one_worker`. `tests/memory_controller.rs::drain_stops_at_60s_or_20_jobs`
  (tempo pausado do tokio).
  **Verde:** worker em `BrokerServer::start`; cancelamento na saída; `drain`; e a ligação do
  `serve --memory` ao publicador das tools (`BrokerConfig::memory`, vinda da T1.14), com teste de
  ponta a ponta.
  **Docs:** `USAGE`, README.
- [x] **T2.12 · doc · PRD principal autoriza o worker.** V13.
  **Vermelho:** `grep` no §23.1 do PRD principal ainda encontra a regra da ausência só de
  `--online`. **Verde:** aplicar os itens §23.1, §23.2/§23.3 e §23.5 do PRD §16.
  **Docs:** PRD principal, changelog, diagrama (`spec/diagrams/`, archify), `handoff.md`.
- [x] **T2.13 · Métricas de custo.** CA-10.
  **Vermelho:** `tests/memory_controller.rs::questions_attempts_bytes_and_cache_are_counted_per_operation_without_content`
  (nenhum path, texto ou chave no que é exposto; cache hit não conta como avaliação nova).
  **Verde:** `src/memory/metrics.rs`, exposto em `Broker::status` e em `memory status`.

**Saída da Fase 2:** CA-8, CA-9, CA-10 cobertos; zero HTTP no hook reconfirmado; PR único.

**Como a Fase 2 saiu ([D-138](../changelog.md#d-138--fase-2-do---memory-controle-jev)):** a T2.0 está
escrita e pendente (sem a chave nesta máquina); o controlador compila no build padrão, só a
ligação ao `JevClient` exige a feature; os testes de falha do provider usam um classificador
falso de roteiro, porque o mapeamento HTTP já está coberto pela T2.4; o teto de requisições do
processo é um classificador `Shared`, não uma mudança no `Scheduler`; o diagrama não foi
atualizado (archify fora desta sessão).

### Fase 3 — Leitura e host

- [x] **T3.1 · Índice lexical e de entidades, RRF.** CA-12.
  **Vermelho:** `tests/memory_retrieval.rs`: `tokens_are_unicode_alphanumeric_lowercase_without_stemming`;
  `lexical_rank_is_the_sum_of_idf_over_shared_terms`; `rrf_fuses_lexical_and_entity_ranks_into_at_most_eight_anchors`
  (valores calculados à mão no teste; empate por ID). Em `props`: `ranking_is_a_pure_function_of_the_snapshot`.
  **Verde:** `src/memory/index.rs`.
- [x] **T3.2 · Routing e divisão do orçamento de expansão.** CA-12.
  **Vermelho:** `tests/memory_retrieval.rs`: `views_activate_at_010_and_unknown_never_activates`;
  `the_budget_of_twelve_is_split_by_largest_remainder_in_fixed_tie_order`;
  `depth_is_one_unless_multi_hop_is_at_least_half`. Em `props`: `the_split_never_exceeds_twelve_and_is_never_negative`.
  **Verde:** `src/memory/retrieve.rs`.
- [x] **T3.3 · Scoring, beam e limites do grafo.** CA-12.
  **Vermelho:** `tests/memory_retrieval.rs`: `the_score_is_the_weighted_sum_and_needs_four_valid_answers`
  (0,40/0,20/0,15/0,15/0,10; entra com relevância ≥ 0,60 e score ≥ 0,60);
  `no_node_is_scored_twice_and_every_visit_is_counted`; `expansion_respects_twelve_sixteen_depth_two_128_and_beam_four`;
  `recency_only_breaks_ties`; `only_active_views_are_expanded_with_their_direction`.
- [x] **T3.4 · Parada e todos os `stop_reason`.** CA-12.
  **Vermelho:** `tests/memory_retrieval.rs`: um teste por valor — `sufficient`, `low_expected_gain`,
  `empty`, `deadline`, `request_limit`, `question_limit`, `graph_limit`, `cancelled`,
  `provider_error`, `budget_omitted` — e `without_a_valid_stopping_answer_it_is_never_sufficient`.
- [x] **T3.5 · Prazo de 750 ms e quatro requests.** CA-12.
  **Vermelho:** `tests/memory_retrieval.rs` (tempo pausado): `the_deadline_cuts_http_and_local_loops`;
  `each_attempt_gets_at_most_250ms_and_there_are_no_automatic_retries`;
  `the_fourth_request_is_reserved_for_stopping`; `request_limit_zero_serves_only_cache_and_says_degraded`;
  `memory_takes_at_most_four_slots_of_the_jev_request_limit`.
- [x] **T3.6 · Fontes mudadas e escritas pendentes.** CA-11.
  **Vermelho:** `tests/memory_retrieval.rs`: `a_memory_whose_source_changed_is_omitted_and_counted_stale`;
  `hashes_and_generation_are_revalidated_right_before_delivery`; `pending_writes_are_reported_and_not_awaited`;
  `another_worktree_of_the_same_head_sees_nothing`.
- [x] **T3.7 · Envelope: `memories[]` e `provenance.memory`.** CA-1, CA-15.
  **Vermelho:** `tests/broker.rs`: `an_envelope_without_memory_serializes_exactly_as_before`
  (caracterização, antes de mexer em `Envelope`); `memories_carry_sources_basis_time_basis_and_untrusted_text`;
  `no_item_ever_has_a_memory_role`. **Verde:** `src/model.rs`, com `skip_serializing_if`.
  **Docs:** PRD principal §8/§9 (item do PRD §16).
- [x] **T3.8 · Orçamento, deduplicação e texto MCP.** CA-13, CA-15.
  **Vermelho:** `tests/broker.rs`: `memory_fits_inside_the_budget_at_most_three_600_tokens_and_20_percent`;
  `risks_and_tests_are_never_evicted_to_make_room_for_memory`; `truncation_after_stopping_marks_assessment_before_truncation`;
  `a_memory_delivered_in_this_session_is_not_repeated`. `tests/mcp_surface.rs`:
  `the_text_block_carries_the_historical_memory_section_once` e
  `without_memory_the_text_block_is_the_envelope_json_as_today` (V10). Em `props`:
  `the_memory_budget_is_never_negative_and_never_exceeds_the_total`.
  **Verde:** `src/budget.rs`, `src/session.rs` (fingerprint de memória), `src/mcp.rs`.
- [x] **T3.9 · Hooks entregam memória.** CA-15.
  **Vermelho:** `tests/hooks.rs`: `an_envelope_with_only_memories_carries_content`;
  `a_new_memory_is_news_and_a_seen_one_is_not`; `the_injected_context_has_the_untrusted_memory_section`;
  `ripwire_off_stops_injection_for_that_session`. **Verde:** `carries_content`, `has_news`,
  `render` em `src/hook.rs`.
- [x] **T3.10 · `context_for_task` lê em paralelo e nunca cai por causa da memória.** CA-11, CA-12.
  **Vermelho:** `tests/broker.rs`: `memory_is_read_alongside_the_structural_context`;
  `a_provider_failure_a_full_disk_or_a_corrupt_store_keeps_the_structural_answer`;
  `memory_never_changes_ready_or_attention_required`; `a_cold_large_snapshot_omits_memory_with_a_limitation_and_warms_up`;
  `the_query_is_never_persisted`.
- [ ] **T3.11 · manual · Consumo real por host.** CA-15.
  **Vermelho:** `tests/memory_hosts.rs` não tem fixture gravada de Claude Code nem de Codex.
  **Verde:** roteiro manual por host, com versão registrada; a fixture gravada entra no teste. Host
  que descarta o campo fica marcado "não validado" no README.
  **Docs:** `integrations/`, skill, README.

**Saída da Fase 3:** CA-12, CA-13, CA-15 cobertos; PR único.

### Fase 4 — Consolidação

- [x] **T4.1 · Cadência durável: 20 enriquecimentos ou 24 h.** CA-7.
  **Vermelho:** `tests/memory_consolidation.rs`: `a_crash_at_write_19_keeps_the_counter_at_19`;
  `pairs_pending_for_24h_trigger_without_the_twentieth_write`; `no_background_timer_runs_without_a_process`;
  `the_cursor_does_not_repeat_the_same_pairs`.
- [x] **T4.2 · Decisões por par e `RepresentationDecision`.** CA-14.
  **Vermelho:** `tests/memory_consolidation.rs`: `a_round_asks_at_most_four_pairs_twenty_questions_in_five_seconds`;
  `originals_are_never_deleted`; `links_and_decisions_work_without_a_summarizer`.
- [x] **T4.3 · Nota derivada só pelo gate.** CA-14.
  **Vermelho:** `tests/memory_consolidation.rs`, com `tests/common/summarizer.rs`:
  `only_merge_or_promote_at_085_with_contradiction_below_085_calls_the_summarizer`;
  `parents_that_do_not_fit_are_not_summarized`; `a_note_naming_unknown_paths_or_ids_is_discarded`;
  `a_timeout_or_invalid_output_keeps_the_pair_separate_and_changes_no_gate`;
  `without_a_trusted_version_cmd_generated_notes_are_not_cached_on_disk`.
  **Verde:** `src/memory/consolidate.rs`, prompt e cache `memory-consolidation/v1`, reutilizando
  `CommandSummarizer`. **Docs:** README, PRD principal §7/§10.

**Saída da Fase 4:** CA-7 (cadência) e CA-14 cobertos; PR único.

**Como a Fase 4 saiu ([D-140](../changelog.md#d-140--fase-4-do---memory-consolidação)):** os pares
de uma rodada são a vizinhança da escrita (os candidatos de cada observação enriquecida, gravados
no commit do enriquecimento depois da revisão); as
perguntas da consolidação passaram a falar de `pairs[c]` para um request levar vários pares; uma
rodada que falhou zera o contador e recomeça a espera, para não virar laço; o cache de notas guarda
só IDs e só existe com sumarizador de versão confiável; o `memory drain` consolida sem
sumarizador (decisões e ligações, sem nota); o teste extra
`a_pair_missing_any_one_answer_is_not_decided` nasceu de um mutante sobrevivente.

### Fase 5 — Avaliação

**Recorte ajustado antes de começar ([D-141](../changelog.md#d-141--recorte-da-fase-5-do---memory-braço-determinístico-e-sequências)):**
o braço C pede um modo do servidor que não existia (T5.0), e a memória só passa de uma sessão a
outra se as sessões de uma sequência rodarem no mesmo caminho (o `workspace_id` inclui a raiz
canônica); o runner ganha o campo `sequence` na T5.1.

- [x] **T5.0 · `serve --memory --memory-selection deterministic`.** CA-16. D-141.
  **Vermelho:** `tests/cli.rs::memory_selection_is_jev_unless_deterministic_is_asked`;
  `tests/broker.rs::a_deterministic_read_asks_the_classifier_nothing_and_says_how_it_chose`
  (mesma revalidação das fontes, `stop_reason: deterministic`, `basis` próprio, sem `scores`);
  `tests/memory_controller.rs::deterministic_memory_collects_and_ingests_but_never_enriches`.
  **Verde:** `src/cli.rs`, `src/memory/retrieve.rs`, `src/memory/runtime.rs`, `src/model.rs`
  (`scores` opcional: ausência não vira zero). **Docs:** `USAGE`, README, PRD principal §9.1.
- [x] **T5.1 · Braços B e C, sequências e store isolado.** CA-16.
  **Vermelho:** `tests/eval.rs`: `the_memory_arms_parse_and_start_the_right_server`
  (`broker-memory`, `broker-memory-deterministic`); `contamination_is_detected_for_the_new_arms`;
  `each_round_gets_an_isolated_store`; `a_sequence_runs_in_order_in_one_place_and_shares_its_store`.
  **Verde:** `src/eval/arm.rs`, `src/eval/corpus.rs` (`sequence`), `src/eval/runner.rs`.
- [x] **T5.2 · Relatório com custo separado.** CA-16.
  **Vermelho:** `tests/eval.rs::the_report_separates_ingestion_retrieval_and_agent_latency_and_records_versions`.
- [ ] **T5.3 · manual · Rodada e gates do PRD §14.** Corpus ≥ 30 tarefas em sequências, fora deste
  repositório. Sem evidência suficiente, o recurso continua experimental. Nenhum número do LoCoMo
  é transferido; nenhuma comparação com Mem0. **O que o instrumento ainda não faz, e a rodada tem de
  tratar ([D-142](../changelog.md#d-142--fase-5-do---memory-avaliação)):** o histórico de cada braço
  são as suas próprias sessões avaliadas, não um histórico de treino comum e separado (B e C diferem
  em histórico além de seleção; o B ainda enriquecido); os braços rodam na ordem de `--arms`, sem
  aleatorização, e sem separar cache frio e quente; a ingestão de uma sessão é paga pela seguinte
  (o worker morre com a sessão; a coluna "jobs pendentes ao fim" mostra o que ficou); uma memória
  cuja fonte o agente editou fica velha na sessão seguinte se o `base` dela não tiver os mesmos bytes;
  o braço C também cede até 4 pedidos de `--jev-request-limit` à leitura (não usa nenhum); e a
  memória automática do próprio Claude Code por diretório, se ativa em `-p`, valeria para todos os
  braços dentro de uma sequência (conferir antes da rodada paga).

**Saída da Fase 5:** CA-16 coberto no instrumento (T5.0–T5.2); a rodada (T5.3) é manual; PR único.

**Como a Fase 5 saiu ([D-142](../changelog.md#d-142--fase-5-do---memory-avaliação)):** o recorte
mudou antes do código (D-141: o braço C por flag do `serve`, as sequências no corpus); o transcript
deixava de ler o envelope quando a seção legível de memória vinha depois do JSON, o que zeraria o
recall de apresentados nos braços B e C; a versão do agente sai do evento `init` de cada sessão.

### Fase 6 — Fechamento

- [ ] **T6.1 · Auditoria do código incluído.** Roteiro no §7.
- [ ] **T6.2 · doc · Incorporar ao PRD principal** os itens restantes do PRD §16, e atualizar
  `handoff.md`, README, diagrama e a tabela de fases.

---

## 6. Cobertura dos critérios do PRD §14

Todo critério tem ao menos uma tarefa; nenhuma tarefa fica sem critério (as `doc` servem à
rastreabilidade do PRD §16/§17).

| CA | Tarefas |
|---|---|
| 1 compatibilidade sem `--memory` | T1.14, T3.7, T3.8 |
| 2 quatro combinações, feature, credencial, comandos locais | T1.1, T1.2, T1.3, T1.13, T1.16, T2.10, T2.11 |
| 3 filtros de admissão | T1.4, T1.7 |
| 4 identidade e idempotência | T1.5, T1.9 |
| 5 tempo | T1.6 |
| 6 hook sem HTTP, fila recuperável, quota | T1.14, T1.15, T2.6, T2.11 |
| 7 injeção de falhas | T1.8, T1.9, T1.11, T2.8, T4.1 |
| 8 parser | T2.3 |
| 9 requests sintéticos | T2.2, T2.4, T2.5, T2.7, T2.9 |
| 10 ledger e custos | T2.6, T2.7, T2.10, T2.13 |
| 11 retenção, forget, indisponibilidade | T1.10, T1.11, T1.12, T3.6, T3.10 |
| 12 `stop_reason` e prazos | T3.1 a T3.5, T3.10 |
| 13 orçamento | T3.8 |
| 14 sumarizador | T4.2, T4.3 |
| 15 entrega por host | T3.7, T3.8, T3.9, T3.11 |
| 16 avaliação A/B/C | T5.1, T5.2, T5.3 |

## 7. Auditoria final (T6.1)

Roda no fim de cada fase sobre o diff da fase, e no fim de tudo sobre o conjunto.

1. **Portões do CI, todos:** os cinco comandos do §1, mais
   `PROPTEST_CASES=4096 cargo test --locked --test props`, `cargo test --locked --test props_fs`,
   a porta do CA-10 (`cargo tree --locked -e normal` sem crate de rede) e a guarda de fixtures.
2. **TDD conferido no histórico:** para cada tarefa, o registro do §9 tem a falha vermelha, e o
   teste aparece no mesmo commit ou antes do código que o satisfaz.
3. **Mutação por amostragem:** uma mutação por módulo novo de `src/memory/`; toda mutação tem de
   derrubar um teste.
4. **Invariantes por busca:** nenhum `reqwest`/`secrecy` fora de `cfg(feature = "online")`;
   nenhum `println!` na lib; nenhum `unsafe`; nenhum texto de observação em `eprintln!`/métrica.
5. **Raio de impacto:** `ripwire --impact` nos símbolos compartilhados alterados (`Envelope`,
   `Classifier`, `carries_content`, `settings`); `ripwire --quality-delta` e `--test-gate`.
6. **Revisão independente** do diff (`/code-review`) e de segurança (`/security-review`), dado que o
   incremento persiste texto e amplia o que sai da máquina.
7. **Docs contra o código**, como no D-133: toda flag do `USAGE` no README; todo número do PRD
   igual à constante no código.

## 8. Linha de base

Medida em 2026-10-03 no `master` em `60c77b9`, antes de qualquer código deste plano. A Fase 1
começa a partir destes números; uma tarefa nunca os reduz.

| Verificação | Resultado |
|---|---|
| `cargo fmt --all --check` | passa |
| `cargo clippy --all-targets --locked -- -D warnings` | passa |
| `cargo clippy --all-targets --locked --features online -- -D warnings` | passa |
| `cargo test --all-targets --locked` | 462 passam, 0 falham, 2 ignorados (22 binários) |
| `cargo test --all-targets --locked --features online` | 476 passam, 0 falham, 4 ignorados (22 binários) |
| CA-10: `cargo tree --locked -e normal` sem `reqwest\|secrecy\|rustls\|hyper` | passa |
| Fixtures sem cara de credencial | passa |

Os números batem com os do `handoff.md`. Não rodados nesta máquina, por não estarem instalados:
`cargo nextest` (o CI roda) e `cargo deny` (agendado, `supply-chain.yml`).

**Auditoria do código incluído neste plano.** O único código que o plano traz é o teste da T1.1.
Ele foi acrescentado a `tests/cli.rs`, compilado e retirado: falha como o plano diz que deve
falhar (`no field memory on type ServeArgs`, `no field online_origin`, `cannot find OnlineOrigin
in cli`), e por nenhum outro motivo; `tests/cli.rs` voltou ao estado do commit. Os símbolos, linhas
e arquivos existentes citados no §2 e no §4 foram conferidos contra o código nesta data.

## 9. Registro de evidência

Preenchido por quem executa. Sem a linha completa, a tarefa não está feita.

| Tarefa | Falha vermelha (teste e mensagem) | Verde (commit) | Mutação que derrubou | Docs atualizadas | Portões |
|---|---|---|---|---|---|
| T0.1 | `grep -n "não foram encontrados" docs/jev-mem-prd.md` encontra a linha 13 da v0.2; `shasum -a 256` do PDF dá `413c5924…5e87`, igual ao PRD | PR da Fase 0 (branch `docs/jev-mem-phase-0`): PRD v0.3, §1, §2.2, §2.4 (novo), §15, §17; o `grep` sai com 1 | a verificação rodada contra `git show HEAD:docs/jev-mem-prd.md` (v0.2) volta a encontrar a pendência | PRD v0.3; D-136; este plano | 5 portões verdes: 462 (2 ignorados) e 476 (4 ignorados) |
| T0.2 | `grep -n "memory resume\|hook .*--memory\|install .*--memory" docs/jev-mem-prd.md` sai com 1; `memory add` fora da tabela do §4 | mesmo PR: §4 (quatro linhas novas, `memory drain` como exceção ao D-064), §6, CA-2, §16; o `grep` encontra as linhas 145–147 | o mesmo `grep` contra a v0.2 conta 0 | PRD v0.3; D-136; este plano | os mesmos 5 portões |
| T1.1 | `tests/cli.rs::memory_implies_online_and_both_flags_are_equivalent`: `no field memory on type ServeArgs`, `cannot find OnlineOrigin in cli` | `f688c01` | origem `Implied`→`Explicit` (cli.rs:393); `--memory` fora de `Flags::online` (cli.rs:386) | `USAGE` | 463 / 477 |
| T1.2 | `tests/cli.rs::memory_options_have_defaults_and_refuse_values_out_of_range`: `no field max_nodes/read_request_limit/retention_days/write_candidates on type MemoryArgs` | `79a2e20` | teto 750→751; `--memory-*` sem `--memory` aceito; mínimo da faixa ignorado (`min..`→`0..`) | `USAGE`; README (tabela de flags) | 464 / 478 |
| T1.3 | `tests/cli.rs`: `a_build_without_the_online_feature_refuses_memory_clearly` e `memory_without_a_credential_fails_before_publishing_mcp` caem em "names the flag asked for" (a mensagem dizia só `--online`) | `9336b1f` | rótulo sempre `--online` (main.rs), nos dois builds | README (linha `--memory`) | 465 / 479 |
| T1.4 | `tests/memory_policy.rs::a_record_round_trips_and_an_oversized_one_is_refused_whole`: `cannot find memory in ripwire_broker` | `5311488` | teto de bytes ×2; conteúdo em `chars` em vez de bytes; 9 referências aceitas; schema não verificado; `types` sem `default` (o `default` por campo, redundante, saiu) | PRD §5.1 conferido, sem mudança | 466 / 480 |
| T1.5 | `tests/memory_identity.rs` (cinco testes) e `tests/props.rs::ids_never_collide_for_ambiguous_tuples`: `unresolved import ripwire_broker::memory::identity` | `132fe56` | comprimento fora do hash (props); git-dir e common-dir fora do hash; descritor parcial aceito; fontes fora do `content_hash`; kind fora do `node_id`; workspace fora da entidade de arquivo (sobreviveu; teste ampliado). Mutante equivalente: só o git-dir fora (determinado por raiz + common-dir), mantido pelo PRD §5.1 | — (PRD §5.3) | 473 / 487 |
| T1.7 (antes da T1.6) | `tests/memory_policy.rs` (três testes; `forbidden_sources_are_refused_before_the_spool`, pois o spool ainda não existe) e `tests/props_fs.rs::no_generated_path_escapes_the_workspace_into_a_record`: `unresolved import ripwire_broker::memory::admission` | `f0e790b` | inelegível ignorado; sem ordenação; sem dedup; e-mail aceito; `sk-`, `AKIA`, `ghp_`, JWT e `password=` desligados um a um; evidência não varrida; "testes passaram" no texto; expiração sem `observed_at`; escopo vazio aceito | README (seção "Persistent memory", o que nunca é guardado) | 477 / 491 |
| T1.6 (depois da T1.7) | `tests/memory_identity.rs::mtime_commit_and_a_clock_rollback_never_become_event_time` e `tests/props.rs::ingest_sequence_is_strictly_monotonic`: `unresolved import ripwire_broker::memory::time` | `5071a79` | sequência que não avança; duração saturada em vez de `None`; `wrapping_add`; sequência tirada do relógio | — | 479 / 493 |
| T1.8 | `tests/memory_store.rs` (quatro testes; dono estrangeiro trocado por diretório aberto, `a_symlink_or_an_open_directory_reads_as_unavailable`): `unresolved import ripwire_broker::memory::store` | `5cfba22` | `publish` sem checar o atual; permissão aberta aceita; sem `O_NOFOLLOW`; schema não checado; escrita direta em vez de temporário + rename (o leitor viu geração parcial, 2 de 2). Sem teste: `sync` do diretório (queda de energia) e uid do dono (exige root) | — | 483 / 497 |
| T1.9 | `tests/memory_store.rs` (quatro testes) e `tests/props_fs.rs::replay_is_idempotent`: `unresolved imports Full, Limits, Refusal, Step`; `no method named enqueue/ingest/...` | `438a190` | duplicata não detectada; `ingest_seq` não atribuído (sobreviveu com o fixture `ingest_seq = n`; fixture corrigido para 0, como o hook manda); teto de nós `>`; spool não removido; teto de entradas `>`; replay ocupando vaga; teto total e teto do snapshot desligados; lock ignorado | — | 488 / 502 |
| T1.10 | `tests/memory_store.rs`: `expired_nodes_take_their_derived_notes_with_them` (arestas e jobs ficam para a Fase 2, que os cria), `a_derived_note_never_outlives_its_parents`, `a_clock_rollback_suspends_expiry_by_age_but_keeps_the_caps`: `no method named sweep` | `e63edff` | sem suspensão; `<=`→`<`; descendentes só um nível; `any`→`all`; leitura confiável não guardada; geração não avança | README (retenção) | 491 / 505 |
| T1.11 | `tests/memory_store.rs` (três testes) e `tests/props_fs.rs::deletion_is_monotonic_across_generations`: `no method named forget`, `no field forgotten on Ingested` | `372a626` | tombstone ignorada na ingestão; spool não limpo; tombstone não gravada; tombstone eterna; sem descendentes; geração parada | README (o que `forget` não promete) | 495 / 509 |
| T1.12 | `tests/memory_store.rs::after_forget_all_nothing_is_collected_until_resumed` e `tests/cli.rs::memory_resume_is_local_and_clears_the_revocation`: `no method named forget_all/is_revoked/resume`, `cannot find MemoryAction in cli`, `no variant Memory` | `258e478` | `enqueue` e `ingest` ignorando o marcador (dois); spool não limpo; sem tombstones; `resume` que não remove o marcador | `USAGE`; README (tabela de comandos, revogação); PRD §6 já na v0.3 | 497 / 511 |
| T1.13 | `tests/cli.rs`: `memory_subcommands_parse`, `memory_status_and_forget_need_no_network_credential_or_feature`, `memory_add_refuses_a_forbidden_input_path_and_untrusted_text_stays_data`: `no variant named Status/Forget/ForgetAll/Add for MemoryAction` | `e3bb04b` | `--all --id` aceitos juntos; arquivo de entrada sem a política do leitor; texto da nota sem varredura; erro do store omitido no `status`; revogação omitida no texto; atribuição trocada; contagem de nós zerada | `USAGE`; README (tabela de comandos, seção de comandos da memória) | 500 / 514 |
| T1.14 | `tests/broker.rs`: `without_memory_the_three_tools_answer_exactly_as_before`, `after_edit_and_before_finish_publish_one_observation_after_the_envelope`, `a_slow_spool_reports_enqueue_unconfirmed_and_never_delays_the_envelope`: `unresolved import memory::publish`, `memory::store::Spool`. Depois do verde, `an_unassessed_finish_claims_nothing_and_an_edit_without_files_observes_what_changed` cobriu dois ramos (provado por mutação) | `bc1c8df` | `unknown` publicado; escopo vazio sem `files`; sem teto de escritas em voo; timeout contado como confirmado; espera sem prazo; conclusão sem publicação. O teste de confirmação passou a esperar 10 s (25 ms dependia da velocidade do disco) | README (quando se coleta; o `serve --memory` só liga na T2.11) | 504 / 518 |
| T1.15 | `tests/cli.rs`: `hook_takes_memory_and_never_implies_online`, `a_hook_with_memory_enqueues_and_never_opens_a_socket`, `a_short_lived_hook_leaves_a_recoverable_queue` (ripwire real; pulados sem ele, como os demais e2e): `no field memory on type HookArgs`. `tests/hooks.rs::a_hook_with_memory_enqueues_its_edit` e `ripwire_off_stops_capture_for_that_session_only` nasceram verdes (o hook já usa o broker da T1.14): caracterização, com mutação | `bbb24bd` | hook sem configuração de memória (2 e2e caem); `HookArgs::memory` sempre falso; publicação da edição removida (caracterização cai). SLO medido à mão em release, ripwire real, 60 pares alternados: p95 +6,9/+4,1 ms, p99 +3,6/−0,9 ms (1ª medição sem alternar deu −97 ms, viés de aquecimento) | `USAGE`; README (coleta pelos hooks) | 509 / 523 (2 ignorados + 1 novo) |
| T1.16 | `tests/cli.rs::install_with_memory_writes_the_flag_and_never_the_key`: `unknown argument --memory`; `doctor_reports_the_memory_store_without_using_the_network`: `no check memory`. Depois do verde, `install_with_memory_for_codex_forwards_the_key_by_name` cobriu o ramo do Codex (provado por mutação) | `f648c11` | hooks sem `--memory`; `--online` redundante; chave não referenciada; aviso do `--online` no lugar do da memória; checagem do doctor sem o guarda de "sem store"; revogação não avisada; ramo do Codex sem `--memory` e sem `env_vars` | `USAGE`; README (tabela, integração, doctor). `integrations/` sem mudança: nada novo chega ao agente nesta fase (T3.11) | 512 / 526 |
| Revisão | Achados 1–8, 10 e 11 do revisor independente (D-137): `an_enqueue_racing_forget_all_never_survives_it`, `leftover_temporaries_are_counted_cleaned_and_erased`, `forget_all_on_an_unreadable_store_still_erases_the_spool`, `one_bad_spool_entry_does_not_block_ingestion_and_a_newer_schema_is_kept`, `a_linked_spool_directory_is_never_written_through`, `memory_add_takes_a_relative_file_and_checks_it_against_the_workspace`, `a_panicking_write_never_keeps_its_slot` e a asserção nova de `online_without_a_credential_fails_before_publishing_mcp`, todos vistos vermelhos | `a7eb3a7` e o seguinte | 11 mutações, todas derrubadas | D-137 (seção da revisão) | 519 / 533 |
| T2.1 | Caracterização já existente: `tests/online_units.rs` congela o pedido Noul (`prompts_v1_are_frozen_*`, digest da gravação real em `the_live_recording_still_matches_prompts_v1`) e o parse (`invalid_probabilities_are_unknown_never_zero`, `a_response_that_breaks_the_contract_is_rejected_whole`); `online_protocol.rs` fixa o corpo enviado verbatim. Nenhum teste novo necessário | — | `instructions` renomeado; `guidance` omitido; tipo `choice` aceito como noul; modelo não checado: as quatro derrubadas | — | 519 / 533 |
| T2.2 | `tests/online_units.rs`: `a_choice_question_serializes_type_instructions_and_criteria` (fixture `memory_choice_request.json`) e `a_memory_request_is_split_before_32_questions_or_38000_bytes_and_never_cuts_a_pair`: `unresolved import memory::wire`, `online::request::StateRequest`, `no function noul/choice on JevQuestion` | `51b9742` | teto de perguntas +4; teto de bytes ×2; `criteria: null` serializado; ids a partir de q1; grupo gigante depois de um que coube (sobreviveu; teste ampliado, laço simplificado) | — (as chaves do estado saem em ordem alfabética: `serde_json` sem `preserve_order`; determinístico, bom para o cache) | 521 / 535 |
| T2.3 | `tests/online_units.rs::a_decision_is_noul_choice_or_unknown_and_never_zero` (tabela de recusas) e `tests/props.rs::no_parsed_probability_is_ever_outside_the_unit_interval`: `unresolved import parse_decisions, Decision, Unknown`; `no variant DuplicateQuestion` | `139a22d` | id repetido aceito; tolerância 1e-1; escolha fora das opções; opção a mais aceita; probabilidade da escolhida trocada pela maior (sobreviveu; caso novo com a escolha menos provável); faixa não checada; modelo não checado; tipo trocado aceito | — (`parse_answers` intacto) | 523 / 537 |
| T2.4 | `tests/online_protocol.rs::the_memory_transport_posts_through_the_same_client`: `unresolved import MemoryClassifier`, `no method decide on JevClient`. Os testes existentes do arquivo seguem verdes sem edição (a trait nova perdeu `model()` para não criar ambiguidade) | `b47b2f4` | teto de resposta ×2; corpo trocado pelo estado; 401 como `Rejected`; resposta alterada | — | 523 / 538 |
| T2.0 | `tests/online_live.rs::the_pinned_model_answers_a_choice` (`#[ignore]`): sem a chave, falha em `live_client` | `95e69d4` | — (teste de contrato real) | D-138 | **rodado à mão em 2026-10-03 com a chave do `.env`**: `jev-1.13.0` respondeu Choice no formato do PRD §7, aceito por `parse_decisions`: `after`, probabilidades {after: 1.0, before: 0.0, unknown: 0.0}, 312 ms |
| T2.5 | `tests/memory_controller.rs::every_stage_names_its_state_fields_and_carries_the_untrusted_guidance`: `unresolved import memory::prompts` | `968adcc` | aviso de não confiável retirado; campo de estado sem pergunta; versão igual à das notas; opção `same_time` retirada; índice do candidato fixo em 0 | — | 524 / 539 |
| T2.6 | `tests/memory_store.rs`: `an_abandoned_lease_is_recovered_by_the_lock_not_by_a_pid`, `a_job_runs_at_most_twice_and_then_stays_failed_until_asked`, `the_24h_ledger_survives_a_restart_and_a_clock_rollback`: `unresolved import memory::queue`, `no method lease_next/finish/retry_failed/charge`, `no field jobs`, `no variant Quota` | `67eb893` | lease vivo ignorado; `>=`→`>` nas execuções (sobreviveu: a guarda repetida em `finish` saiu e entrou o caso do lease abandonado na 2ª execução); `not_before` ignorado; job não criado na ingestão; `forget` sem levar o job; leitura mais alta do relógio ignorada (sobreviveu; caso novo de cobrança com relógio atrasado); teto de tentativas e registro da cobrança desligados | — | 527 / 542 |
| T2.7 | `tests/memory_controller.rs` (seis testes) e `tests/props.rs::the_two_causal_directions_are_never_confused`: `unresolved import memory::controller`, `model::{Graph, EdgeBasis}`, `no field edges on State` | `42ddd4b` | limiar 0,59; direção de `caused_by` trocada (props); alias sempre perguntado; tempo com um lado só; desempate por distância retirado (indistinguível do id a partir do 1º nó; caso novo a partir do 6º); desconhecido não marca `partial`; falha do typing sem registro; índice global nos lotes divididos (checagem nova); `unknown` com 0,8 virando aresta (sobreviveu; caso corrigido) | — (controlador no build padrão: só o `JevClient` exige a feature) | 535 / 550 |
| T2.8 | `tests/memory_controller.rs`: `a_timeout_in_the_middle_of_a_pair_commits_nothing_of_that_pair`, `a_late_answer_for_an_old_generation_is_dropped`, `an_answer_for_a_forgotten_node_does_not_recreate_it`, `the_enrichment_counter_increments_exactly_once_per_node`: `commit_enrichment takes 4 arguments but 5 were supplied`, `no field enriched` | `211b2b5` | par parcial aceito; geração vista ignorada; contador a cada execução; contador também em falha; ponta de aresta esquecida aceita (sobreviveu; teste novo `an_edge_to_a_candidate_forgotten_meanwhile_is_dropped`) | — | 540 / 555 |
| T2.9 | `tests/memory_controller.rs`: `auth_failures_suspend_the_worker_until_reauthorized`, `a_429_waits_only_inside_the_budget` (tempo pausado), `a_5xx_is_retried_once_inside_the_four_attempts`, `cancellation_stops_http_and_leaves_the_job_pending`, `no_model_swap_and_no_silent_heuristic_on_failure`: `unresolved import controller::Worker`. Com classificador falso de roteiro, não com o servidor local: o mapeamento HTTP→erro já está coberto pela T2.4 e pela suíte do `--online` | `05678ac` | worker não suspende; 429 esperando além do prazo; segundo 429 esperado (sobreviveu; caso novo); retry transitório sem limite; 8 tentativas; 401 tratado como falha comum; cooldown trocado pelo padrão (sobreviveu; asserção exata) | — | 545 / 560 |
| T2.10 | `tests/online_scheduler.rs::memory_and_discovery_share_four_requests_and_one_client`: `unresolved import classifier::Shared`; `tests/memory_controller.rs::one_remote_job_per_workspace`: o segundo worker pegou outro job (asserção) | `776953a` | licença retirada da memória; licença retirada da descoberta; lock remoto ignorado | — (o `Scheduler` ficou intocado: o teto do processo é um classificador `Shared` com semáforo, que a T2.11 liga no `serve`) | 547 / 562 |
| T2.11 | `tests/memory_controller.rs`: `online_alone_never_processes_old_memory_jobs`, `memory_and_online_memory_start_one_worker`, `the_server_leaves_no_process_with_the_credential_on_exit`, `drain_stops_at_60s_or_20_jobs` (tempo pausado): `unresolved import memory::runtime`; `tests/cli.rs::memory_drain_needs_online_and_a_credential`: `no variant Drain`. Os testes de ciclo de vida são de biblioteca: um teste do binário com `serve --memory` faria o worker chamar o provider real | `903c452` | worker que sobrevive ao servidor; teto de jobs `>`; prazo do drain sem corte (sobreviveu com jobs de 10 s, que terminam em 60 s exatos; job de 25 s); spool não incorporado; runtime sem `--memory`; drain sem `--online` | `USAGE`; README (`memory drain`, enriquecimento, `serve --memory` ligado) | 552 / 567 |
| T2.13 | `tests/memory_controller.rs::questions_attempts_bytes_and_cache_are_counted_per_operation_without_content`: `no method metrics on Worker`; `tests/cli.rs::memory_status_shows_the_24h_budget_in_use`: `(None, None)`; `tests/broker.rs::the_status_shows_what_the_memory_worker_cost`: `no field worker on MemoryConfig` | `3febaae` | retries, falhas por categoria, perguntas, bytes, jobs concluídos e o acúmulo do worker desligados um a um; status sem o worker; quota de 24 h zerada | README (custo no status, quota no `memory status`). Sem cache de decisões ainda: nada a contar como acerto de cache (registrado no D-138) | 555 / 570 |
| T2.12 | `grep 'Ausência de \`--online\` significa' spec/ripwire-broker-mcp.md` encontrava o invariante 1 do §23.1 | `f1230c1` | a mesma busca contra o commit anterior volta a encontrar | PRD principal §23.1 (ativação, invariantes 1 e 4, invariantes 12–14), §23.2, §23.3, §23.5; PRD jev-mem §16 (itens marcados aplicados). **Diagrama não atualizado:** o archify é um skill desta máquina do mantenedor, fora desta sessão (registrado no D-138) | só documentação |
| Revisão F2 | Achados 1–7, 9–11 do revisor independente (D-138): `a_spent_quota_keeps_jobs_pending_and_uses_no_run`, `a_relations_failure_leaves_the_job_pending_and_is_not_counted`, `the_enrichment_counter_increments_exactly_once_per_node` (refeito via `finish`), `memory_retry_brings_failed_jobs_back`, `the_worker_bookkeeping_waits_briefly_for_another_writer`, `without_memory_discovery_keeps_its_client_and_its_per_query_ceiling`, `commits_stay_inside_the_edge_and_snapshot_caps`, `drain_says_when_it_could_not_run_instead_of_reporting_empty`, `memory_responses_are_not_counted_as_discovery_bytes` e a asserção nova de `memory_drain_needs_online_and_a_credential`, todos vistos vermelhos | `7ca28b1`, `2aab7f2`, `e2229a6` e o seguinte | 16 mutações derrubadas; a marca `counted` sobreviveu por ser redundante (`Done` é terminal) e saiu | D-138 (seção da revisão); README (`memory retry`, opções do `drain`, quota) | 562 / 578 |
| T3.1 | `tests/memory_retrieval.rs`: `tokens_are_unicode_alphanumeric_lowercase_without_stemming`, `lexical_rank_is_the_sum_of_idf_over_shared_terms`, `rrf_fuses_lexical_and_entity_ranks_into_at_most_eight_anchors` (valores à mão) e `tests/props.rs::ranking_is_a_pure_function_of_the_snapshot`: `unresolved import memory::index` | `30a2740` | sem minúsculas; idf sem o `1 +`; k = 61; desempate invertido; sem o teto de 8; sem o ranking de entidades; só o primeiro termo da consulta | — | 566 / 582 |
| T3.2 | `tests/memory_retrieval.rs`: `views_activate_at_010_and_unknown_never_activates`, `the_budget_of_twelve_is_split_by_largest_remainder_in_fixed_tie_order`, `depth_is_one_unless_multi_hop_is_at_least_half` e `tests/props.rs::the_split_never_exceeds_twelve_and_is_never_negative`: `unresolved import memory::retrieve` | `c266085` | limiar 0,0999; maiores restos trocado por ordem invertida; sem a expansão mínima por visão (props); profundidade com `>`; desconhecido sem marcar parcial; recência com `>` | — | 570 / 586 |
| T3.3 | `tests/memory_retrieval.rs`: `the_score_is_the_weighted_sum_and_needs_four_valid_answers`, `no_node_is_scored_twice_and_every_visit_is_counted`, `expansion_respects_twelve_sixteen_depth_two_128_and_beam_four`, `recency_only_breaks_ties`, `only_active_views_are_expanded_with_their_direction`: `cannot find function read`, `unresolved imports Read, ReadConfig, StopReason` | `7cf98e1` | peso 0,41; sem o limiar de relevância; desconhecido como 0; nó avaliado duas vezes; profundidade +1; beam 5; aresta causal de entrada seguida; filtro de visão ativa (sobreviveu: repetia o orçamento por visão; virou um caminho só); recência ignorada; teto de arestas 300 (caso novo de 200 arestas); 40 expansões. Mutante equivalente: o teto de 16 nós (8 âncoras + um lote de 8) | — | 575 / 591 |
| T3.4 | `tests/memory_retrieval.rs`: um teste por `stop_reason` (`sufficient`, `low_expected_gain`, `empty`, `deadline`, `request_limit`, `question_limit`, `graph_limit`, `cancelled`, `provider_error`, `budget_omitted`) e `without_a_valid_stopping_answer_it_is_never_sufficient`: `no field cancel/max_questions on ReadConfig`, `cannot find function fit` | `6b28670` | pré-checagem do cancelamento (sobreviveu: contava uma requisição; teste exige zero); teto de requisições `>`; teto de perguntas desligado; erro do provider como prazo e vice-versa; limiar 0,94 (sobreviveu; caso de 0,949); ganho 0,14; limite do grafo ignorado; `BudgetOmitted` desligado; desconhecido na parada sem marcar parcial | — | 586 / 602 |
| T3.5 | `tests/memory_retrieval.rs` (tempo pausado): `the_deadline_cuts_http_and_local_loops`, `each_attempt_gets_at_most_250ms_and_there_are_no_automatic_retries`, `the_fourth_request_is_reserved_for_stopping`, `request_limit_zero_serves_only_cache_and_says_degraded`, `memory_takes_at_most_four_slots_of_the_jev_request_limit`: `cannot find function slots`, `no field degraded` | `1fd2b8d` | sem o teto de 250 ms; sem a reserva da parada; degradado não declarado; sem o teto de 4 vagas; 400 ms por tentativa. O laço local de expansão não checa o prazo (limitado a 128 arestas; sem como provar em tempo pausado); o prazo é checado antes de cada requisição. Junto: `lease_next` passou a esperar o escritor (falha isolada de `memory_retry_brings_failed_jobs_back` sob carga) | — | 591 / 607 |
| T3.6 | `tests/memory_retrieval.rs`: `a_memory_whose_source_changed_is_omitted_and_counted_stale`, `hashes_and_generation_are_revalidated_right_before_delivery`, `pending_writes_are_reported_and_not_awaited`, `another_worktree_of_the_same_head_sees_nothing`: `cannot find function read_store` | `6e73286` | filtro inicial de frescura (sobreviveu: a revalidação final o cobria; teste exige que a memória velha nem vá a scoring); geração não comparada (sobreviveu; caso do mesmo id em outra geração); frescura final desligada; pendentes não contadas | — | 595 / 611 |
| T3.7 | `tests/broker.rs`: `an_envelope_without_memory_serializes_exactly_as_before` (caracterização, escrita antes de mexer no `Envelope`; verde depois, protegida por mutação), `memories_carry_sources_basis_time_basis_and_untrusted_text`, `no_item_ever_has_a_memory_role`: `no field memories on Envelope`, `no field memory on Provenance`, `cannot find function items/provenance` | `c7b1833` | `skip_serializing_if` retirado de `memories` e de `provenance.memory`; `time_basis` fixo; texto vazio; `stale_omitted` zerado | PRD principal §9.1 (memória histórica no envelope) | 598 / 614 |
| T3.8 | `tests/broker.rs`: `memory_fits_inside_the_budget_at_most_three_600_tokens_and_20_percent`, `risks_and_tests_are_never_evicted_to_make_room_for_memory`, `truncation_after_stopping_marks_assessment_before_truncation`, `a_memory_delivered_in_this_session_is_not_repeated`; `tests/mcp_surface.rs`: `the_text_block_carries_the_historical_memory_section_once`, `without_memory_the_text_block_is_the_envelope_json_as_today` (V10), `a_tool_result_carries_the_memory_section_in_its_text`; `tests/props.rs::the_memory_budget_is_never_negative_and_never_exceeds_the_total`: `cannot find function attach/text_of`, `module budget is private` | `5ebfaf0` | 4 itens; 700 tokens e 30% (sobreviveram: nenhum caso encostava no teto; casos de 225 tokens e de orçamento 1.500); sem o limite do que sobra (sobreviveu: memórias grandes demais; caso de 60 tokens de folga); sessão ignorada; `assessment_before_truncation` desligado; `budget_omitted` desligado; `already_delivered` não somado; seção sempre presente; `tool_result` sem `text_of` (sobreviveu; teste novo do resultado serializado). O custo de uma memória é o seu JSON inteiro (~95 tokens além do texto) | — | 606 / 622 |
| T3.9 | `tests/hooks.rs`: `an_envelope_with_only_memories_carries_content`, `a_new_memory_is_news_and_a_seen_one_is_not`, `the_injected_context_has_the_untrusted_memory_section`, `with_memory_on_the_hook_output_stays_under_the_host_limit`, `ripwire_off_stops_injection_for_that_session` (caracterização: já verde; protegida por mutação); `tests/broker.rs::memory_bookkeeping_never_pushes_the_envelope_past_its_budget`: `function has_news is private`, `a memory is something to act on`, `never told`, seção ausente, `14 × 200: 9034 chars`, `300: the estimate is the delivered size (224 ≠ 282)` | `4953084` | `carries_content` sem memórias; `has_news` sem memórias; `render` sem a seção; seção sempre (sobreviveu: nenhum caso estourava; casos 13 × 220 e 17 × 100); aviso sem a contagem; sem `memory_reserve`; sem `provenance.memory` no lugar durante o encaixe; sem atualizar `estimated_tokens`; `widest_limitations` vazio; opt-out ignorado: todas mortas. Defeito de T3.8/T3.10 achado aqui: escrituração da memória depois do encaixe estourava o orçamento em ~100 tokens | PRD principal §9.1 (reserva, hooks); README "Reading" e "In the hooks" | 620 / 636 |
| T3.10 | `tests/broker.rs`: `memory_is_read_alongside_the_structural_context`, `a_provider_failure_a_full_disk_or_a_corrupt_store_keeps_the_structural_answer`, `memory_never_changes_ready_or_attention_required`, `a_cold_large_snapshot_omits_memory_with_a_limitation_and_warms_up`, `the_query_is_never_persisted`, `a_memory_goes_out_once_per_session`, `a_store_broken_during_the_read_delivers_no_memory`; `tests/memory_controller.rs::serve_with_memory_reads_with_its_flags`: `unresolved import ReadSetup`, `no field read on MemoryConfig`; depois `context_for_task reads memory` (o `serve` não montava a leitura) | `7c6a734` | sem `attach`; limitações descartadas; memórias não lembradas na sessão; sem cache quente (20 s frios); esperar a carga inteira; memórias mantidas sem a reconferência; `ProviderError` fora de `memory_incomplete`: todas mortas. Sem mutação: a leitura em paralelo (só tempo) e "a consulta nunca é gravada" (guarda; não há escrita a mutar) | PRD principal §9.1 (leitura, limitações `memory_cold`/`memory_unavailable`/`memory_incomplete`); README "Reading" e linha de `--memory` | 614 / 630 |
| T3.11 | **Pendente (manual).** Pede uma sessão real do Claude Code e do Codex com `serve --memory` e a chave, e a fixture gravada de cada uma; nenhum host foi validado nesta fase | — | — | README: integração por host "não validada" até a rodada | — |
| Revisão F3 | Achados 1–12 do revisor independente (D-139): `memory_and_the_count_of_those_left_out_never_pass_the_budget_by_a_token`, `untrusted_memory_fields_cannot_forge_lines_in_the_section`, `an_entity_is_named_only_by_the_whole_path`, `depth_two_expands_only_from_a_beam_of_the_four_strongest_relations`, `notes_and_memory_together_stay_inside_the_budget`, `discovery_and_the_memory_read_share_one_jev_request_limit`, `only_what_a_read_is_about_to_use_has_its_sources_checked`, `checking_sources_counts_against_the_deadline`, `a_stale_memory_reached_by_a_relation_is_not_scored_either`, `the_last_checks_keep_their_time_when_the_provider_takes_all_of_it`, `a_short_deadline_still_leaves_the_requests_most_of_it`, `bookkeeping_writes_keep_the_snapshot_warm`, `a_snapshot_rewritten_with_the_same_size_and_time_is_still_seen`, `the_generation_is_recorded_beside_every_snapshot`, `reads_are_charged_to_the_24_hour_quota`, `a_spent_quota_stops_reads_from_asking`, `reads_in_a_row_count_what_the_ones_before_spent`, `a_read_asks_no_more_questions_than_the_quota_has_left`, `a_store_from_before_the_quota_file_keeps_the_quota_it_recorded`, `the_quota_a_snapshot_recorded_carries_over_to_its_own_file`, todos vistos vermelhos; os testes da T3.6 refeitos por `Recall` (o de `fit` saiu com ele); `the_deadline_cuts_http_and_local_loops` renomeado `the_deadline_cuts_the_requests`; `the_text_block_carries_the_historical_memory_section_once` e o teste da seção nos hooks pedem o texto uma vez só | `6c110e6`, `e7b8e7f`, `7f902e2`, `a356d4c`, `0368b6c`, `4b6a9f3`, `8e5d480`, `db1ac0e` | 33 mutações derrubadas; sobreviveram a parada por prazo dentro do laço de hashes (o `timeout_at` já responde; ela só libera a thread de bloqueio) e o recomeço de um carregamento em pânico (inalcançável pela interface); a checagem "geração do selo = geração carregada" sobreviveu e saiu, por desnecessária | D-139 (seção da revisão); README (leitura, quota, `--memory-read-request-limit`); spec §9.1 e §23.5 | 639 / 655 |
| T4.1 | `tests/memory_consolidation.rs`: `a_crash_at_write_19_keeps_the_counter_at_19`, `pairs_pending_for_24h_trigger_without_the_twentieth_write`, `no_background_timer_runs_without_a_process`, `the_cursor_does_not_repeat_the_same_pairs`: `unresolved import ripwire_broker::memory::consolidate`, `no field consolidation on type State` | `950a007` | gatilho de 20 → 19; espera de 24 h menos 1 ms; cursor ignorado; contador não gravado na rodada; início da espera em 0 em vez da hora do lease; espera não recomeçada com pares pendentes; `drain` sem rodada: todas mortas | — | suíte alvo verde; portões completos na T4.3 |
| T4.2 | `tests/memory_consolidation.rs`: `a_round_asks_at_most_four_pairs_twenty_questions_in_five_seconds`, `links_and_decisions_work_without_a_summarizer`: `cannot find value DEADLINE/MAX_QUESTIONS/MAX_ATTEMPTS/LINK in consolidate`; depois `one pair was worth linking` (nenhuma aresta); `originals_are_never_deleted` nasceu verde (guarda: não há remoção a desfazer), provada por mutação | `914ae2b` | 5 pares; prazo ×2; 1 tentativa; limiar da ligação 0,2; ligação não gravada; grafo da ligação trocado; o mais antigo removido quando obsoleto: todas mortas. Resposta ausente virando 0 **sobreviveu**: teste novo `a_pair_missing_any_one_answer_is_not_decided` (no commit da T4.3) a mata para as quatro Nouls. Equivalente: a guarda `Decision::Choice` e a probabilidade da opção cobrem o mesmo caso | — | suíte alvo verde; portões completos na T4.3 |
| T4.3 | `tests/memory_consolidation.rs` (com `tests/common/summarizer.rs`): `only_merge_or_promote_at_085_with_contradiction_below_085_calls_the_summarizer`, `parents_that_do_not_fit_are_not_summarized`, `a_note_naming_unknown_paths_or_ids_is_discarded`, `a_timeout_or_invalid_output_keeps_the_pair_separate_and_changes_no_gate`, `without_a_trusted_version_cmd_generated_notes_are_not_cached_on_disk`: `cannot find NOTE_PROMPT_VERSION`, `no method versioned/with_summarizer`, `no field note/notes`; `the_worker_gives_its_rounds_the_servers_summarizer`: `no method named set_summarizer` | `d9a7ddb` | `>=` → `>` no 0,85; `<` → `<=` na contradição; `keep_separate` autorizando nota; teto dos pais ×2; validador que aceita tudo; IDs não verificados; caminho com `/` não verificado (sobreviveu: o caso tinha extensão; caso `docs/internal/README` acrescentado); 601 caracteres aceitos; cache sem versão confiável; cache não lido; `forget` sem limpar o cache; erro do sumarizador virando nota; pais faltando em `derived_from`; modelo não gravado; pai fora do prompt; sumarizador não passado à rodada; notas fora das métricas: todas mortas. Contador de notas nas métricas acrescentado junto com o código, provado só por mutação. Sem teste próprio: a expiração pelo mínimo dos pais (a retenção já leva os descendentes) | README (Consolidation, métricas); PRD principal §7.2, §10.3, §23.3 | 653 / 669 |
| Revisão F4 | Achados do revisor independente (D-140): `a_round_that_cannot_be_committed_does_not_run_again_at_once` (`Err(Full(Snapshot))`), `a_round_that_sent_nothing_keeps_its_trigger`, `the_pairs_of_a_store_at_its_cap_are_found_without_scanning_it` e `the_pairs_are_the_neighbours_an_enrichment_compared` (`no field pairs`, `no function Pair::of`), `a_drain_without_time_for_a_whole_round_does_not_start_one` (1 request ≠ 0), `what_is_forgotten_during_a_round_is_neither_summarized_nor_recorded` (o sumarizador recebeu o prompt), `a_note_that_cannot_be_added_is_neither_pointed_at_nor_cached`, `a_note_naming_unknown_paths_or_ids_is_discarded` ampliado (`.env` aceito), `the_parents_reach_the_summarizer_fenced`, todos vistos vermelhos; `a_round_split_over_requests_moves_the_cursor_only_past_what_went_out` e `a_forget_all_during_a_split_round_stops_the_requests_left` nasceram verdes depois da correção que os pede e foram provados por mutação | `3587297`, `5e17173`, `9de7445`, `e28792d`, `d5510ee`, `e396259`, `4d4738d`, `eaca339` | sem o recuo do snapshot cheio; sem a espera do worker; espera ×2; commit com nada enviado; cursor no último dos quatro; índice local trocado (sobreviveu até o teste ganhar a segunda volta com os dois requests); pares não gravados; o worker passando `&[]`; `forget` mantendo os pares; `pending` ignorando decisões; prazo do drain pela metade; revogação ignorada entre requests; pais não relidos antes das notas; cache e ponteiro de nota que não entrou; tombstone e teto de nós ignorados; arquivo oculto, nome de uma letra, extensão longa, hash curto e prefixo conhecido desligados um a um; ponto inicial cortado; pai cru no prompt: todas mortas. Uma rodada de mutações correu com o teste do validador vermelho (caso errado) e foi refeita. Sem teste: a chave com política e schema (constantes) | D-140 (seção da revisão); README (Consolidation); PRD principal §10.3 | 663 / 679 |
| T5.0 | `tests/cli.rs::memory_selection_is_jev_unless_deterministic_is_asked`: `unresolved import Selection`, `no field selection on MemoryArgs`; `tests/broker.rs::a_deterministic_read_asks_the_classifier_nothing_and_says_how_it_chose`: `no field selection on ReadConfig`; `tests/memory_controller.rs::deterministic_memory_collects_and_ingests_but_never_enriches`: seleção `Jev` ≠ `Deterministic` | `81858b9` | `deterministic` lido como `jev`; ramo determinístico desligado; `stale_omitted` não contado; `basis` trocado; `scores` zerados; `stop_reason` trocado; o runtime enriquecendo; a seleção fora da leitura: todas mortas | `USAGE`; README (linha da flag); PRD principal §9.1 | (portões no fim da fase) |
| T5.1 | `tests/eval.rs`: `the_memory_arms_parse_and_start_the_right_server`, `contamination_is_detected_for_the_new_arms`, `each_round_gets_an_isolated_store`, `a_sequence_runs_in_order_in_one_place_and_shares_its_store`: `no variant BrokerMemory/BrokerMemoryDeterministic`, `no method needs_credential`, `mcp_config takes 2 arguments` | `6c0b839` | credencial sem o braço B; sem o `XDG_STATE_HOME`; o C sem `--memory-selection`; sequência não agrupada; cópia não refeita entre sessões; lugar novo a cada sessão; store não removido (sobreviveu: o agente falso não criava o store; passou a criar): todas mortas | README (corpus `sequence`, braços, isolamento) | (idem) |
| T5.2 | `tests/eval.rs::the_report_separates_ingestion_retrieval_and_agent_latency_and_records_versions`: o resultado nem era lido (o `echo` do `sh` do macOS quebrava a linha), depois `presented_recall` nulo (o JSON seguido da seção de memória), depois a seção ausente | `3dbd8a4` | `from_str` de volta; espera não somada; requests e memórias entregues não somados; ingestão sem descontar as leituras; campos de memória nos braços sem memória; sem a medida de antes; seção fora; modelo trocado; agente sem o modelo; versões fora do relatório: todas mortas. Rodar o agente com `--version` contava como execução (e deixou `src/auth.py` na raiz, removido na revisão) | README (saída, custo da memória) | (idem) |
| Revisão F5 | Achados do revisor independente (D-142): `a_sequence_partly_recorded_is_refused_not_resumed_in_part`, `a_sequence_stays_in_one_repository`, `a_session_after_an_invalid_one_says_its_history_is_incomplete`, `versions_that_change_between_runs_are_refused`, e o teste do relatório ampliado (jobs deixados, braço A, `--json`, células exatas), todos vistos vermelhos | `9722f9f`, `b442be8`, `41bf6fd`, `7600d15` | rodada parcial aceita; repositório e nome vazio da sequência; histórico incompleto não marcado; versões trocadas aceitas; `jobs_pending` zerado; braço A fora; custo fora do `--json`; `basis` sem a seleção: todas mortas | D-142; README; PRD jev-mem §10 (`deterministic`); este plano (T5.3) | 675 / 691 |
