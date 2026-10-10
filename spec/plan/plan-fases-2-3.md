# Plano de implementação — Fases 2 e 3

Status: **Fases 2 e 3 concluídas: pontos de parada 1 (D-040), 2 (D-042), 3 (D-045) e 4 (D-048); S3.15 adiada, S3.17 removida (D-046)** · 2026-09-27 (revisão independente incorporada, ver D-038) · decisões em
[D-028 a D-037](../changelog.md#d-028--plano-das-fases-2-e-3)

Fonte: PRD §8.4, §10.3, §11.1, §16.1, §19 (Fases 2 e 3) e §21.1–21.3.

## 0. Princípios do plano

1. **TDD em fatias verticais.** Cada fatia (S2.x / S3.x) é um ciclo: um teste
   vermelho num seam acordado, o mínimo de código para ficar verde, `cargo test`
   inteiro verde antes da fatia seguinte. Refatoração só na revisão, fora do ciclo.
2. **Impacto mínimo.** O código novo fica em módulos novos. Os módulos existentes
   ganham só pontos de extensão pequenos e opcionais. Tudo que é novo vem **desligado**
   em `BrokerConfig::new`, por isso os 48 testes atuais continuam valendo sem edição
   (ver §5).
3. **Contratos preservados.** O envelope continua `ripwire-broker.context/v1`: só
   entram campos **opcionais e aditivos**, omitidos quando vazios. As três tools
   continuam as mesmas; `context_for_task`, `context_after_edit` e
   `context_before_finish` ganham um único parâmetro opcional (`include_seen`).
4. **CA-10 intacto.** Nenhuma crate de rede. O modelo local roda como
   **subprocesso** (stdin → stdout), como o Ripwire.
5. **Um teste por comportamento, nomeado como especificação**, com valores
   esperados vindos de fixtures ou de exemplos escritos à mão, nunca recalculados
   como o código calcula.

## 1. Seams de teste (a confirmar)

| # | Seam | Onde | Dublê |
| --- | --- | --- | --- |
| 1 | Núcleo `Broker` (existente) | `tests/broker.rs` | `FakeUpstream`; na Fase 3 também `FakeSummarizer` |
| 2 | Superfície MCP e2e (existente) | `tests/mcp_surface.rs` | ripwire real; na Fase 3 um script-summarizer |
| 3 | Upstream real (existente) | `tests/upstream_ripwire.rs` | nenhum |
| 4 | **Hook** — `hook::handle(evento, broker, estado, política)` | `tests/hooks.rs` (novo) | `FakeUpstream` |
| 5 | **CLI do binário** — `hook`, `prompt`, `doctor`, `install` | `tests/cli.rs` (novo) | ripwire real (pula se ausente), diretórios temporários |
| 6 | **Summarizer por comando** — subprocesso real | `tests/summarizer.rs` (novo) | script `sh` em diretório temporário |

Os seams 4 e 6 são públicos na lib (`ripwire_broker::hook`, `ripwire_broker::summarizer`).
Nenhum teste toca função privada.

## 2. Arquitetura resultante

```text
                    ┌──────────── ripwire-broker (binário) ─────────────┐
host MCP ─stdio──▶  │ serve (padrão)  → mcp.rs ─┐                        │
host hook ─stdin──▶ │ hook <host> <ev> → hook.rs ┼─▶ Broker ─▶ Upstream ─┼─stdio─▶ ripwire
terminal ─────────▶ │ prompt | doctor | install  │    │  ├─ session.rs     │
                    │         (cli.rs)           ┘    │  └─ notes.rs ──────┼─stdin─▶ modelo local
                    │ state.rs (sessões) · cache.rs (notas) fora do workspace│         (opcional)
                    └────────────────────────────────────────────────────┘
```

| Módulo | Novo? | Papel |
| --- | --- | --- |
| `cli.rs` | novo | Parse de subcomandos, puro e testável. `main.rs` só delega |
| `session.rs` | novo | Memória de itens entregues (fingerprints), serializável |
| `hook.rs` | novo | Eventos Claude Code e Codex → requests do broker → saída do host |
| `state.rs` | novo | Estado de sessão em disco (hooks), gravação atômica, `0600` |
| `doctor.rs` | novo | Diagnóstico da instalação |
| `install.rs` | novo | Trechos de configuração e merge JSON idempotente |
| `summarizer.rs` | novo | `trait Summarizer` + `CommandSummarizer` (subprocesso) |
| `notes.rs` | novo | Agrupamento por módulo, prompt, saneamento da saída |
| `cache.rs` | novo | Cache de notas endereçado por conteúdo (sha256) |
| `broker.rs` | toca | 3 campos de config; 1 chamada em `envelope()`; 1 passo de notas em `context_for_task` |
| `model.rs` | toca | `Basis::LocalModel`, `Note`, `Envelope.notes`, `Budget.already_delivered` |
| `budget.rs` | toca | `add_notes()` depois de `fill()` |
| `metrics.rs` | toca | `session_hits`, contadores de notas |
| `mcp.rs` | toca | parâmetro `include_seen`; flags novas passam por `Settings` |
| `main.rs` | toca | vira despacho de `cli::parse` |

Dependência nova: **`sha2`** (hash estável para o cache e o nome dos arquivos de sessão;
não é crate de rede e já está no `Cargo.lock`, via rust-mcp-sdk). Nenhuma outra.

---

## 3. Fase 2 — Automação por host

### 3.1 Contexto incremental por sessão (PRD §11.1, §16.1, D-021)

**Comportamento.** Dentro de uma sessão, um item já entregue e **inalterado** volta só
como **referência enxuta**: o mesmo `Item`, mas sem `content` nem `signature`, com
`why_included = "already delivered in this session (unchanged)"`. Testes e riscos
repetidos saem da resposta, e a contagem vai para `budget.already_delivered`.
Limitações nunca são suprimidas. Um item cujo conteúdo mudou volta completo.
`include_seen: true` desliga a supressão naquela chamada (útil depois que o host
compacta o contexto).

**Fingerprint** (pura, em `session.rs`):
- item: `path`, `line`, `symbol`, `signature` e sha256 do `content`;
- teste: `path` e `run`;
- risco: `kind`, `path`, `symbol` e `message`.

Só o que **efetivamente saiu** na resposta (depois do orçamento) é marcado como visto.
Mecânica em `envelope()`, depois de `cap_items`:
1. cada entrada recebe seu fingerprint;
2. as já vistas viram referência enxuta ou são descartadas e contadas;
3. `budget::fill` roda;
4. os fingerprints dos itens completos que ficaram em `env` são registrados.

Uma referência enxuta nunca registra nada. Um item que perdeu o corpo no `try_item`
registra o fingerprint **sem** corpo, e por isso o corpo volta quando couber.

**A evidência de um gate nunca é suprimida (CA-05).** `context_before_finish` não
passa pelo filtro de sessão. Em `context_after_edit`, os riscos que decidem o status
(`cochange_missing`, `contract_change`) também não. Assim o `status` e a sua evidência
sempre vêm juntos, inclusive no `reason` do gate de `Stop`.

**Sessão.** No servidor MCP, a sessão é o processo. Nos hooks, é o `session_id` do
host, persistido por `state.rs`. O `Broker` expõe `restore_session(SessionMemory)` e
`session_snapshot()` para os hooks carregarem e salvarem o estado.

**Padrões.** Desligado em `BrokerConfig::new`, o que preserva os testes atuais.
**Desligado também no `serve`**, com opt-in por `--incremental`: subagentes do Claude
Code compartilham o processo MCP do pai e receberiam referências a um contexto que
nunca viram. **Ligado nos hooks**, que têm um `session_id` real. A referência enxuta
diz no `why_included` que `include_seen=true` traz o conteúdo de volta.

| Fatia | Teste (seam 1, salvo indicação) | Mínimo para verde |
| --- | --- | --- |
| S2.1 | `a_repeated_item_is_sent_once_per_session`: duas chamadas iguais; a 2ª traz as referências enxutas e `estimated_tokens` menor | `session.rs` + filtro em `envelope()` antes de `budget::fill` |
| S2.2 | `a_changed_item_is_sent_again_in_full`: a mesma chamada, com o fixture de corpo alterado | fingerprint com hash do conteúdo; `FakeUpstream::answer_seq` (helper de teste) |
| S2.3 | `limitations_are_never_suppressed_as_seen` (teste-guarda: verde ao nascer, protege contra regressão) | filtro ignora `Entry::Limitation` |
| S2.3a | `the_finish_gate_always_shows_its_evidence`: um risco já entregue em `context_after_edit` reaparece completo em `context_before_finish`, com `attention_required` | `context_before_finish` fora do filtro; riscos de status isentos |
| S2.4 | `repeated_tests_and_risks_are_counted_not_repeated` (`budget.already_delivered`) | campo aditivo `Budget.already_delivered` (omitido se 0) |
| S2.5 | `an_item_cut_by_the_budget_is_not_marked_as_seen`: 1ª chamada com 400 tokens, 2ª com 2500 → o item vem completo | registrar depois de `fill` |
| S2.6 | `include_seen_returns_the_full_context_again` | campo `include_seen` nos três requests |
| S2.7 | `without_incremental_answers_do_not_depend_on_history` (teste-guarda): config padrão, duas chamadas → itens idênticos | guarda do padrão |
| S2.8 | `session_hits_are_counted_without_content` (status) | `Metrics.session_hits` |
| S2.8a | `a_session_snapshot_restores_what_was_delivered` (dois `Broker`; a memória de um é restaurada no outro) | `session_snapshot` / `restore_session` |
| S2.9 | seam 2: `the_server_is_stateless_by_default_and_incremental_on_request` (sem flag → itens idênticos; com `--incremental` → referências) + `include_seen` no schema | `mcp.rs`, flag em `cli.rs` (depois de S2.10) |

### 3.2 CLI com subcomandos (pré-requisito dos hooks)

`ripwire-broker [serve] --workspace DIR …` continua funcionando exatamente como hoje
(as flags sem subcomando equivalem a `serve`). Entram também `hook`, `prompt`, `doctor`
e `install`.

| Fatia | Teste (seam 5, puro sobre `cli::parse`) | Mínimo |
| --- | --- | --- |
| S2.10 | `flags_without_a_subcommand_still_start_the_server` (a forma de hoje, byte a byte) | `cli.rs` com `Command::Serve`; `main` delega |
| S2.11 | `each_subcommand_parses_its_flags` e `unknown_input_is_a_usage_error` | variantes `Hook`, `Prompt`, `Doctor`, `Install` |

As e2e atuais do seam 2 continuam provando que o `serve` não mudou.

### 3.3 Hooks — Claude Code e Codex (PRD §8.4 nível 2; §21.1–21.2)

**Achado que muda a decisão 21.1.** O Codex 0.157 tem hooks (`UserPromptSubmit`,
`PostToolUse`, `Stop` e `SessionStart`, em `~/.codex/hooks.json`) com o **mesmo
contrato** do Claude Code:
- entrada: JSON no stdin com `session_id`, `cwd`, `prompt` e `tool_name`/`tool_input`;
- saída: `{"hookSpecificOutput":{"hookEventName","additionalContext"}}`, e
  `{"decision":"block","reason"}` no `Stop`.

A única diferença relevante é a edição no Codex. Ela chega como `apply_patch`, com os
arquivos nos cabeçalhos `*** Add/Update/Delete File:` e `*** Move to:` de
`tool_input.command`. **Proposta: os dois hosts na mesma fatia**, com um único
`hook.rs` e dois leitores de entrada.

**Comando.** `ripwire-broker hook <claude-code|codex> <user-prompt-submit|post-tool-use|stop> --workspace DIR [--state-dir D] [--every-prompt] [--gate]`.
Ele cria o broker no próprio processo, o que sobe o ripwire a frio: ~100 ms em
~1k símbolos, segundo D-019. Depois lê o estado da sessão, trata o evento, grava o
estado e escreve a saída do host. O **exit code é sempre 0**, e as falhas viram
`systemMessage` (§21.4: nunca bloquear sozinho).

**Granularidade padrão (proposta para §21.2):** injeção automática **no primeiro
prompt e depois de edições**. O gate de `Stop` é opt-in (`--gate`) e injetar em todo
prompt também (`--every-prompt`).

**Opt-out por sessão e visualização (PRD §8.4):**
- `#ripwire-off` / `#ripwire-on` no prompt desliga/religa a sessão (fica no estado).
  Para desligar de vez, remova o hook com `install`. Não há variável de ambiente, o
  que mantém D-022: a configuração é só por argumentos.
- Toda injeção traz um `systemMessage` de uma linha para o usuário, com tool,
  `request_id`, itens e tokens. O conteúdo injetado completo já fica visível no
  transcript do host.
- `ripwire-broker hook-log --session ID` lista as últimas 5 injeções, com contagens
  e tokens. Referências `path#symbol` só entram se o hook tiver rodado com
  `--log-refs`, porque o PRD §16.1 não registra caminhos nem símbolos por padrão.

**Tamanho da injeção.** Os hosts limitam a saída de hook. Por isso os orçamentos
padrão dos hooks são menores (1.500 no prompt e 800 na edição), e o texto injetado
tem um teto de 9.000 caracteres, verificado por teste. O limite exato de cada host é
medido na fatia S2.11a.

**Contratos reais primeiro.** Antes de S2.12, a fatia S2.11a grava como fixtures
(`tests/fixtures/hooks/`) payloads reais dos dois hosts: um `UserPromptSubmit`, um
`PostToolUse` de `Edit` e um de `apply_patch`, e um `Stop`. Para isso usa um hook de
captura temporário, registrado manualmente. Os testes do seam 4 usam esses fixtures,
não JSON imaginado.

| Fatia | Teste (seam 4, salvo indicação) | Mínimo |
| --- | --- | --- |
| S2.11a | (sem teste) capturar os payloads reais dos dois hosts e medir o limite de saída | fixtures + nota no changelog |
| S2.12 | `the_first_prompt_gets_task_context_as_additional_context` (Claude Code) | `hook::handle` para `UserPromptSubmit` → `context_for_task`; sessão restaurada e salva |
| S2.12a | `hook_output_stays_under_the_host_limit` | teto de caracteres |
| S2.13 | `later_prompts_are_not_injected_unless_every_prompt_is_set` | `SessionState.prompts_seen` |
| S2.14 | `an_opt_out_marker_silences_the_session_until_opt_in` | `SessionState.opted_out` |
| S2.15 | `an_edit_injects_what_it_may_have_affected` (Claude Code `Edit`/`Write`/`MultiEdit`, `file_path` absoluto → relativo) | `PostToolUse` → `context_after_edit` |
| S2.16 | `a_codex_patch_names_the_changed_files` (Add, Update, Delete e Move) | leitor de `apply_patch` |
| S2.17 | `an_edit_outside_the_workspace_is_ignored_without_upstream_calls` (CA-08; é política, não falha, por isso não há `systemMessage`) | o guard existente |
| S2.18 | `an_edit_with_nothing_new_injects_nothing` (tudo já visto) | decisão "vazio → sem saída" |
| S2.19 | `the_stop_gate_blocks_once_when_attention_is_required`; com `stop_hook_active` não bloqueia; `unknown` só avisa; sem `--gate` só avisa | `Stop` → `context_before_finish` |
| S2.20 | `a_broker_failure_never_breaks_the_host` (upstream fora → só `systemMessage`) | mapeamento de erro |
| S2.21 | `malformed_or_unknown_events_are_ignored` | leitor tolerante |
| S2.22 | `session_state_is_private_and_holds_no_prompt_or_code` (dir `0700`, arquivo `0600`, nome = sha256 do `session_id`, gravação atômica) | `state.rs` |
| S2.23 | seam 5: `the_hook_command_injects_context_from_the_real_ripwire_once_per_session` (duas invocações do binário) | `Command::Hook` em `main` |
| S2.24 | seam 5: `hook_log_lists_injections_without_paths_unless_asked` | `hook-log`, `--log-refs` |

O estado vai em `--state-dir`, por padrão `$XDG_STATE_HOME/ripwire-broker`, ou
`~/.local/state/ripwire-broker`. Fica **fora do workspace**, pelo princípio
read-only do §15.1: o broker não grava no repositório. Hooks concorrentes da mesma
sessão: o último a gravar vence, e a gravação é atômica. Isso é documentado, não
bloqueado.

### 3.4 Wrapper para clientes sem hook (PRD §8.4 nível 3)

`ripwire-broker prompt --workspace DIR "tarefa"` imprime a tarefa seguida de um bloco
delimitado com o contexto, marcado como dados não confiáveis. Uso:
`cliente "$(ripwire-broker prompt …)"`.

| Fatia | Teste | Mínimo |
| --- | --- | --- |
| S2.25 | seam 5: `the_prompt_wrapper_prints_the_task_and_delimited_context` | `Command::Prompt` |

### 3.5 Instalação e diagnóstico automatizados

**`doctor`** verifica e informa, com `--json` opcional:
- se o ripwire está no PATH e se a versão é ≥ 0.6.4;
- se os verbos obrigatórios existem;
- se o workspace pode ser canonicalizado e se é repositório git **com commit**
  (D-019: sem commit, o gate vira `unknown`);
- se o state dir aceita escrita e se o summarizer está configurado (Fase 3);
- uma chamada de fumaça a `explore`.

O exit code é 0 se tudo passa, 1 se alguma verificação obrigatória falha e 2 para uso
inválido.

**`install <claude-code|codex> --workspace DIR [--hooks] [--write]`** tem **dry-run
por padrão**: imprime o que mudaria e onde.

Com `--write`:
- **Claude Code:** faz merge em `<workspace>/.mcp.json` e, com `--hooks`, em
  `<workspace>/.claude/settings.json`;
- **Codex:** faz merge em `<codex-home>/hooks.json` (`--codex-home`, padrão
  `~/.codex`);
- em qualquer host, o merge preserva as chaves alheias, cria um `.bak` quando altera
  um arquivo existente e é idempotente;
- o `config.toml` do Codex (servidor MCP e `hooks = true`) é **só impresso**, porque
  editar TOML sem parser arriscaria o arquivo do usuário;
- `install --write` é a única operação que grava **dentro** do workspace (`.mcp.json`
  e `.claude/settings.json`), e só por pedido explícito. O README documenta isso.

`doctor` reutiliza `check_version` (hoje privada em `broker.rs`) e `ripwire_version`
(hoje em `main.rs`). As duas passam a ser públicas, a segunda num módulo da lib.

| Fatia | Teste (seam 5) | Mínimo |
| --- | --- | --- |
| S2.26 | `doctor_passes_on_a_healthy_workspace` (ripwire real) | `doctor.rs` |
| S2.27 | `doctor_fails_on_an_old_ripwire` (script falso que diz `0.5.0`) e avisa sobre repositório sem commit | reuso de `check_version` |
| S2.28 | `install_is_a_dry_run_unless_write_is_given` | `install.rs` |
| S2.29 | `install_claude_code_merges_idempotently_and_keeps_foreign_keys` (duas execuções → arquivos idênticos; `.bak`) | merge JSON |
| S2.30 | `install_codex_merges_hooks_json_and_prints_the_toml_snippet` | idem |

### 3.6 Documentação da Fase 2

- README: modo automático, opt-out, `hook show`, `doctor` e `install`.
- `integrations/`: exemplos de `settings.json` e `hooks.json`.
- A skill e o `AGENTS.md` explicam que o contexto pode chegar injetado e que `include_seen` existe.
- PRD: §8.4 (estado), §16.1 (cache hit lógico ativo), §21.1 e §21.2 (resolvidos) e o roadmap.

---

## 4. Fase 3 — Enriquecimento local opcional

### 4.1 Decisões de desenho

**Modelo local por subprocesso, não HTTP.** Configuração:
`--summarizer-cmd "ollama run phi4"`. O comando é dividido em argv, **sem shell**, e o
texto entra pelo stdin e sai pelo stdout. Isso preserva o CA-10: o teste
`the_build_has_no_network_stack` continua verde. Serve qualquer CLI local: ollama,
llama.cpp, `llm`. O próprio `ollama run` conversa com um servidor HTTP em
localhost, mas isso fica fora do processo do broker e é permitido pelo §10.3. O
README documenta isso. Medido nesta máquina com `ollama run phi4` por stdin: **19 s a frio,
1 s aquecido**.

**Notas arquiteturais por módulo.**
- **Agrupamento:** os itens **incluídos** no envelope são agrupados pelos dois
  primeiros componentes do caminho (`src/auth`, por exemplo). Até 3 grupos, pela
  ordem de prioridade.
- **Evidência:** para cada grupo, os itens incluídos (ref, assinatura, `why_included`
  e conteúdo), limitados a ~2.000 caracteres. Ela é montada a partir da versão
  **completa** de cada item, antes do filtro de sessão. Assim, uma referência enxuta
  não esvazia a evidência nem faz o cache errar. A própria nota passa pelo filtro de
  sessão: uma nota já entregue não se repete.
- **Formato:** cada nota é `{scope, text: {untrusted_repository_data}, generated: true, model, derived_from: [refs], cached, source: {verb: "local_model", basis: "local_model"}}`.
- **Escopo:** só em `context_for_task`. As outras duas tools continuam enxutas.

**Cache próprio, endereçado por conteúdo.** A chave é
`sha256("notes/v1" ‖ model_id ‖ evidência canônica)`:
- mudar o código, o modelo ou a versão do prompt invalida a entrada;
- **versão do modelo (§10.3):** com `--summarizer-version-cmd "ollama show phi4
  --modelfile"`, o sha256 da saída desse comando entra na chave, e trocar os pesos sob
  a mesma tag invalida o cache sozinho. O comando roda uma vez por processo. Sem ele, o
  `model_id` é o próprio comando, e o requisito fica **parcialmente atendido**: a troca
  de pesos sob a mesma tag passa despercebida. O `doctor` avisa disso;
- os arquivos ficam em `--cache-dir`, por padrão
  `$XDG_CACHE_HOME/ripwire-broker/<sha(workspace)>`, **fora do workspace**, com `0600`;
- a mesma evidência em outra tarefa reaproveita a nota. **Essa é a leitura proposta
  para "cache semântico":** cache de conteúdo gerado, não busca por similaridade.

**Nunca bloquear a resposta.** O broker espera no máximo `--summarizer-wait-ms`
(padrão 1.500 ms; os testes usam 0 ou um `FakeSummarizer` controlado por
`tokio::sync::Notify`, sem esperar tempo real). Se a nota não ficar pronta a tempo:
- a resposta sai sem ela, com a limitação `note_pending`, e o resumo determinístico
  continua;
- a geração segue **em segundo plano**, com limite rígido `--summarizer-timeout-ms`
  (padrão 60 s), e grava no cache;
- há no máximo **uma** geração em voo;
- falhas viram a limitação `summarizer_unavailable`.

Nos hooks (processo curto) não há segundo plano: só entram notas do cache, e as
faltantes são registradas como `note_pending` sem geração.

**Orçamento.**
- As notas entram **depois** dos itens (`budget::add_notes`), porque são contexto
  periférico (PRD §10.2 #9).
- Uma nota que não cabe vira a limitação `notes_omitted`.
- A propriedade "estimado ≤ solicitado" continua valendo para todas as respostas.

**Saneamento.**
- A saída do modelo perde caracteres de controle e é cortada em 600 caracteres.
- Ela sempre vai dentro de `untrusted_repository_data`, porque é derivada de dados
  do repositório.

### 4.2 Fatias

| Fatia | Teste (seam) | Mínimo |
| --- | --- | --- |
| S3.1 | 1: `without_a_summarizer_the_envelope_has_no_notes` | `BrokerConfig.summarizer: None`; `notes` omitido se vazio |
| S3.2 | 1: `a_note_is_derived_only_from_included_items` (o `FakeSummarizer` grava a evidência recebida; refs ⊆ itens do envelope; `derived_from` confere) | `trait Summarizer`, `notes.rs` |
| S3.3 | 1: `notes_group_items_by_module_up_to_three` | agrupamento |
| S3.4 | 1: `notes_never_break_the_budget` (varredura de 256 a 4.000; limitação `notes_omitted`) | `budget::add_notes` |
| S3.5 | 1: `a_cached_note_is_reused_without_calling_the_model` (`cached: true`, 1 chamada) | `cache.rs` só em memória |
| S3.6 | 1: `the_cache_is_invalidated_by_evidence_or_model_change` (inclui a saída do `version-cmd`) | chave sha256 |
| S3.6a | 1: `a_repeated_task_in_an_incremental_session_hits_the_note_cache` (evidência a partir do item completo) | evidência antes do filtro |
| S3.7 | 1: `a_slow_model_answers_later_from_the_cache` (1ª: `note_pending`; depois de `wait_background()`, a 2ª traz a nota) | espera limitada + tarefa com clones `Arc` (summarizer, cache, contadores), `Mutex<Option<JoinHandle>>` |
| S3.8 | 1: `only_one_background_generation_runs_at_a_time` | semáforo de 1 |
| S3.9 | 1: `a_failing_model_degrades_to_deterministic_output` (`summarizer_unavailable`; status e itens intactos) | mapeamento de erro |
| S3.10 | 1: `model_output_is_sanitized_capped_and_marked_untrusted` | saneamento |
| S3.11 | 1: `the_status_reports_the_summarizer_without_content` (enabled, basename do programa, generated, cache_hits, pending, failures; sem texto de nota nem prompt) | `BrokerStatus.summarizer` |
| S3.12 | 6: `the_command_summarizer_feeds_stdin_and_reads_stdout` | `CommandSummarizer` |
| S3.13 | 6: `arguments_reach_the_program_literally_without_a_shell` (um `;` no argv não executa nada) | argv sem shell |
| S3.14 | 6: `a_hung_model_is_killed_at_the_hard_limit` e `a_nonzero_exit_is_an_error` | `kill_on_drop` + timeout |
| S3.15 | 1: `the_note_cache_survives_a_restart_and_ignores_corrupt_files` (dois `Broker`, mesmo `cache_dir`) | leitura tolerante. **Feita no D-168**, opt-in: [plano](plan-s3-15-cache-de-notas.md) |
| S3.16 | 2: `the_server_adds_notes_with_a_command_summarizer` (script `sh` como modelo) + flags em `cli.rs` | `Settings` |
| S3.17 | 5: `hooks_use_cached_notes_only` | política de hook |
| S3.18 | 6, `#[ignore]` + `RIPWIRE_BROKER_TEST_MODEL="ollama run phi4"`: `a_real_local_model_writes_a_note` | opt-in, fora da suíte padrão |

### 4.3 Documentação da Fase 3

- PRD: §9.1 (campos `notes` e `already_delivered`), §10.3 (estado), §16.1
  (métricas de notas) e roadmap.
- README: como ligar o summarizer e que efeito ele tem na privacidade.
- A skill e o `AGENTS.md` dizem que notas são geradas, não confiáveis e sempre
  verificáveis por `derived_from`.

---

## 5. Validação do plano contra os testes atuais

Linha de base (2026-09-27 13:05): **48 verdes** (36 broker, 5 mcp_surface,
7 upstream_ripwire) e clippy sem avisos.

| Mudança | Testes existentes afetados | Por que não quebram |
| --- | --- | --- |
| Campos novos em `TaskRequest`, `EditRequest`, `FinishRequest` | 4 literais em `tests/broker.rs` | Todos usam `..Default`/`::new`, e os padrões mantêm o comportamento |
| `BrokerConfig` ganha `incremental`, `summarizer` e `cache_dir` | helper `broker()` e 3 testes de config | `BrokerConfig::new` usa `false`/`None`, e o comportamento fica idêntico |
| Sessão no `serve` | e2e do seam 2 | Desligada por padrão no `serve`; só `--incremental` a liga, e S2.9 cobre esse caso |
| `Budget.already_delivered` e `Envelope.notes` | asserts sobre o JSON do envelope | Os campos são omitidos quando vazios; nenhum teste compara o envelope inteiro |
| Parâmetro `include_seen` nos schemas | `publishes_three_read_only_tools…` | O teste exige só `additionalProperties: false` e `readOnlyHint`, que continuam |
| `main` delega a `cli::parse` | todo o seam 2 (sobe o binário com `--workspace`) | S2.10 exige a forma atual sem subcomando |
| `sha2` como dependência direta | `the_build_has_no_network_stack` | O `sha2` já está no `Cargo.lock` via rust-mcp-sdk, e a lista proibida é reqwest, hyper, axum e rustls |
| Status ganha `summarizer` e `session_hits` | `status_reports_operations_without_sensitive_content` | Os campos novos só têm contagens; S3.11 estende a verificação de vazamento |
| `envelope()` passa pelo filtro de sessão | os 36 do seam 1 | Com `incremental=false` o filtro é identidade; S2.7 garante isso |

Critérios de aceite do MVP que o plano precisa manter verdes: CA-01 a CA-10, todos
cobertos pelos testes existentes, que não são editados.

## 6. Ordem de execução e pontos de parada

1. S2.1–S2.9: sessão (núcleo + MCP). **Ponto de parada 1.**
2. S2.10–S2.11: CLI.
3. S2.12–S2.24: hooks. **Ponto de parada 2**, com teste manual num host real.
4. S2.25–S2.30: wrapper, doctor e install. Documentação da Fase 2. **Ponto de parada 3.**
   Aqui vale o §21.3: antes de criar cache persistente, medir `session_hits` e o
   tamanho das injeções em uso real e registrar o resultado. Se a deduplicação por
   sessão bastar, a Fase 3 segue com cache só em memória, e S3.15 vira opcional. Os
   hooks, que são processos curtos, ficariam sem notas.
5. S3.1–S3.11: notas no núcleo com `FakeSummarizer`.
6. S3.12–S3.18: summarizer por comando, cache em disco (conforme o ponto 3) e e2e.
   Documentação da Fase 3. **Ponto de parada 4.**

Ordem dentro da Fase 2: S2.1–S2.8a → S2.10–S2.11 (CLI) → S2.9 (que precisa da flag)
→ S2.11a → hooks.

Em cada ponto de parada rodam `cargo test`, `cargo clippy --all-targets` e
`cargo fmt --check`, e o changelog é atualizado.

## 7. Riscos do plano

| Risco | Mitigação |
| --- | --- |
| Formato dos hooks mudar numa versão futura do host | Leitor tolerante (S2.21) e testes com JSON gravado dos dois hosts |
| Ripwire a frio em repositório grande deixar o hook lento | `timeout` no registro do hook; o `doctor` mede e informa |
| A referência enxuta confundir o agente depois da compactação do host | `include_seen`; a skill ensina quando usar |
| Modelo local lento | Espera limitada, segundo plano e cache |
| Nota com conteúdo alucinado | `derived_from`, `generated: true`, texto não confiável e evidência só de itens incluídos |
| Detalhes de implementação | `Arc<dyn Summarizer>` em `BrokerConfig` exige um `Debug` manual; `tokio` declara `process` e `io-util` explicitamente, sem depender da unificação de features; referências enxutas contam em `items_shown` (documentado) |
| Estado e cache em disco guardarem dados sensíveis | O estado guarda só fingerprints e refs; o cache guarda notas (derivadas de código) com `0600`, fora do workspace; o README documenta |
