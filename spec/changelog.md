# Changelog de decisões — ripwire-broker

| Data | Resumo | Referência |
| --- | --- | --- |
| 2026-09-27 01:10 | Seams de teste acordados: núcleo `Broker`, superfície MCP e2e, cliente upstream real | [D-001](#d-001--seams-de-teste) |
| 2026-09-27 01:10 | SDK 2.0.0 (stateless `2026-07-28`) validado contra ripwire 0.6.4 (`2025-06-18`) sem `initialize` | [D-002](#d-002--compatibilidade-de-protocolo-sdk--ripwire) |
| 2026-09-27 01:10 | Superfície real do ripwire 0.6.4: 33 verbos, 4 de escrita/baseline excluídos | [D-003](#d-003--superfície-upstream-e-allowlist) |
| 2026-09-27 01:10 | Payloads upstream em dois formatos (tags XML-like e JSON); leitor tolerante próprio | [D-004](#d-004--formato-dos-payloads-e-parser-tolerante) |
| 2026-09-27 01:10 | Timeout próprio, reinício controlado e no máximo um reinício por chamada | [D-005](#d-005--timeout-reinício-e-classificação-de-erros) |
| 2026-09-27 01:10 | Fixtures gravadas do ripwire real num repositório sintético | [D-006](#d-006--fixtures-do-ripwire) |
| 2026-09-27 01:10 | Estimativa de tokens = bytes do JSON serializado / 4; orçamento aplicado ao envelope inteiro | [D-007](#d-007--orçamento-e-estimativa-de-tokens) |
| 2026-09-27 01:10 | Deduplicação por símbolo, localização, corpo, teste, risco e limitação | [D-008](#d-008--deduplicação) |
| 2026-09-27 01:10 | Regras determinísticas do roteador de intenção | [D-009](#d-009--roteamento-de-intenção) |
| 2026-09-27 01:10 | `context_after_edit`: `impact` só quando `edit_check` detecta mudança de contrato | [D-010](#d-010--context_after_edit) |
| 2026-09-27 01:10 | Workspace guard: canonicalização, `..`, absolutos, symlinks e line-seeds | [D-011](#d-011--workspace-guard) |
| 2026-09-27 01:18 | Achado do spike: `tools/list` tipado do SDK rejeita a resposta 2025-06-18 do ripwire; leitura via resultado genérico | [D-012](#d-012--toolslist-contra-servidor-pré-2026) |
| 2026-09-27 01:18 | `context_before_finish`: regras de `ready`/`attention_required`/`unknown` e interpretação de `strict` | [D-013](#d-013--context_before_finish-e-semântica-de-strict) |
| 2026-09-27 01:18 | Allowlist aplicada em runtime e validação de capabilities na inicialização | [D-014](#d-014--allowlist-e-validação-de-capabilities) |
| 2026-09-27 01:18 | Status e métricas locais: só contagens, durações e tipos de erro, sem mensagens | [D-015](#d-015--status-e-métricas) |
| 2026-09-27 01:18 | Resumo determinístico, limite de tamanho por item e orçamento mínimo de 256 | [D-016](#d-016--resumo-limite-por-item-e-orçamento-mínimo) |
| 2026-09-27 01:18 | Schemas das tools escritos à mão (estritos) em vez do derive `JsonSchema` do SDK | [D-017](#d-017--schemas-estritos-das-tools) |
| 2026-09-27 01:18 | Modo degradado: servidor sobe sem ripwire e reconecta sob demanda | [D-018](#d-018--modo-degradado) |
| 2026-09-27 01:18 | Medições da Fase 0 (schemas, payloads, latência, overhead) | [D-019](#d-019--medições-da-fase-0) |
| 2026-09-27 01:18 | Integração por skill (Claude Code) e AGENTS.md (Codex); toolchain fixada em 1.98.1 | [D-020](#d-020--integração-com-agentes-e-toolchain) |
| 2026-09-27 01:18 | Pendências conhecidas das Fases 0/1 | [D-021](#d-021--pendências-conhecidas) |
| 2026-09-27 12:50 | Sem `.env`: configuração só por argumentos; `env` do host reservado para segredos futuros | [D-022](#d-022--configuração-sem-env) |
| 2026-09-27 12:58 | Versão mínima do ripwire (0.6.4) validada na conexão | [D-023](#d-023--versão-mínima-do-ripwire) |
| 2026-09-27 12:58 | Pedidos de revisão reconhecidos no modo `auto` | [D-024](#d-024--revisão-reconhecida-no-modo-auto) |
| 2026-09-27 12:58 | Caso incerto: `explore` com metade do orçamento e limitação `route_uncertain` | [D-025](#d-025--orçamento-conservador-no-caso-incerto) |
| 2026-09-27 12:58 | Correlação tool call × chamadas upstream (`request_id`, `recent_requests`) | [D-026](#d-026--correlação-entre-tool-call-e-chamadas-upstream) |
| 2026-09-27 12:58 | Revisão código × PRD: PRD alinhado às decisões já tomadas; lacunas menores aceitas | [D-027](#d-027--revisão-código--prd) |
| 2026-09-27 13:10 | Plano das Fases 2 e 3 (proposta): fatias TDD, seams, validação contra os testes atuais | [D-028](#d-028--plano-das-fases-2-e-3) |
| 2026-09-27 13:10 | Proposta: contexto incremental por sessão com referência enxuta e `include_seen` | [D-029](#d-029--contexto-incremental-por-sessão-proposta) |
| 2026-09-27 13:10 | Achado: Codex 0.157 tem hooks com o mesmo contrato do Claude Code; proposta de hooks nos dois hosts | [D-030](#d-030--hooks-nos-dois-hosts-proposta) |
| 2026-09-27 13:10 | Proposta: granularidade padrão = primeiro prompt + edições; gate de `Stop` opt-in | [D-031](#d-031--granularidade-da-automação-proposta) |
| 2026-09-27 13:10 | Proposta: estado de sessão dos hooks em disco, fora do workspace, sem prompt nem código | [D-032](#d-032--estado-de-sessão-em-disco-proposta) |
| 2026-09-27 13:10 | Proposta: `install` em dry-run por padrão e merge JSON idempotente; `doctor` | [D-033](#d-033--instalação-e-diagnóstico-proposta) |
| 2026-09-27 13:10 | Proposta: modelo local por subprocesso (não HTTP), preservando o CA-10; phi4 medido em 19 s frio / 1 s aquecido | [D-034](#d-034--modelo-local-por-subprocesso-proposta) |
| 2026-09-27 13:10 | Proposta: notas arquiteturais por módulo e cache endereçado por conteúdo (sha256) | [D-035](#d-035--notas-por-módulo-e-cache-proposta) |
| 2026-09-27 13:10 | Proposta: espera limitada, geração em segundo plano e fallback determinístico | [D-036](#d-036--espera-limitada-e-fallback-proposta) |
| 2026-09-27 13:10 | Proposta: extensões só aditivas ao envelope v1 (`notes`, `budget.already_delivered`) | [D-037](#d-037--extensões-aditivas-ao-envelope-v1-proposta) |
| 2026-09-27 13:18 | Revisão independente do plano: 1 bloqueador (evidência do gate) e 8 ajustes incorporados; D-029, D-031 e D-032 revistas | [D-038](#d-038--revisão-do-plano-das-fases-2-e-3) |
| 2026-09-27 13:20 | Plano aprovado; D-029 a D-037 (com as revisões de D-038) passam de proposta a decisão; execução até o ponto de parada 1 | [D-039](#d-039--aprovação-do-plano) |
| 2026-09-27 13:22 | Ponto de parada 1: contexto incremental por sessão no núcleo (S2.1–S2.8a), 58 testes verdes | [D-040](#d-040--ponto-de-parada-1-contexto-incremental-no-núcleo) |
| 2026-09-27 13:45 | Contratos reais dos hooks capturados (Claude Code 2.1.283, Codex 0.157.1); limite prático de ~10k caracteres nos dois | [D-041](#d-041--contratos-reais-dos-hooks-e-limite-de-saída) |
| 2026-09-27 13:45 | Ponto de parada 2: CLI com subcomandos, `include_seen`/`--incremental` no MCP, hooks nos dois hosts, estado em disco, `hook-log`; teste ao vivo; 79 verdes | [D-042](#d-042--ponto-de-parada-2-cli-e-hooks) |
| 2026-09-27 13:45 | Achado do teste ao vivo: o roteador escolhe `apply_patch` como símbolo e `find_symbol` recusa, deixando o prompt sem contexto (decisão pendente) | [D-043](#d-043--achado-roteador-escolhe-o-identificador-errado) |
| 2026-09-27 13:48 | Decisão sobre D-043: símbolo inexistente cai para `explore` com a limitação `symbol_not_found` | [D-044](#d-044--símbolo-inexistente-cai-para-explore) |
| 2026-09-27 13:53 | Ponto de parada 3: `prompt`, `doctor`, `install` e documentação da Fase 2; 87 verdes; Fase 2 concluída | [D-045](#d-045--ponto-de-parada-3-wrapper-diagnóstico-e-instalação) |
| 2026-09-27 14:02 | Início da Fase 3 com cache de notas só em memória; cache em disco (S3.15) adiado até medir `session_hits` | [D-046](#d-046--fase-3-com-cache-em-memória) |
| 2026-09-27 14:25 | Achado com o modelo real: `ollama run` emite códigos de terminal pela pipe; saneamento remove ANSI, e `--nowordwrap` é exigido pelo `doctor` | [D-047](#d-047--códigos-de-terminal-na-saída-do-modelo) |
| 2026-09-27 14:25 | Ponto de parada 4: Fase 3 implementada (notas por modelo local, cache em memória); 110 verdes + 1 opt-in com phi4 real | [D-048](#d-048--ponto-de-parada-4-fase-3) |
| 2026-09-27 15:36 | Cancelamento pelo cliente implementado (RF-14); D-019 estava errada; status não trava com ripwire ocupado (`busy`) | [D-049](#d-049--cancelamento-pelo-cliente-e-status-que-não-trava) |
| 2026-09-27 15:36 | Limite de memória do ripwire por supervisor interno (`--ripwire-max-rss-mb`) | [D-050](#d-050--limite-de-memória-do-ripwire-por-supervisor) |
| 2026-09-27 15:36 | `request_id` contínuo nos hooks; `Shape` substitui os 7 parâmetros de `envelope()`; pendências de D-021 fechadas; 118 verdes | [D-051](#d-051--pendências-técnicas-fechadas) |
| 2026-09-27 15:44 | Code review: 9 achados verificados (2 altos, 4 médios, 3 baixos); usuário decidiu corrigir todos por TDD | [D-052](#d-052--code-review-das-fases-2-e-3) |
| 2026-09-27 15:53 | Os 9 achados do code review corrigidos por TDD; 127 verdes | [D-053](#d-053--correções-do-code-review) |
| 2026-09-27 15:55 | Projeto publicado como repositório público `CeciApp/ripwire-broker` no GitHub | [D-054](#d-054--publicação-no-ceciapp) |
| 2026-09-27 15:59 | Licença MIT adicionada (`LICENSE`, `Cargo.toml`, README) | [D-055](#d-055--licença-mit) |
| 2026-09-27 17:35 | Adaptador `--online` fundido no PRD 0.3 (`ripwire-broker-mcp.md`) a partir da spec Jev v0.1; Fase 4→6 (Times e CI), novas Fases 4 e 5 | [D-056](#d-056--fusão-do-adaptador---online-no-prd) |
| 2026-09-27 17:40 | Specs antigas movidas para `spec/old/`; `ripwire-broker-mcp.md` passa a ser o PRD vigente | [D-057](#d-057--reorganização-das-specs) |
| 2026-09-27 17:55 | Plano das Fases 4 e 5 (proposta): Sprint 0, 31 fatias na Fase 4 e 14 na Fase 5, cinco pontos de parada | [D-058](#d-058--plano-das-fases-4-e-5) |
| 2026-09-27 17:55 | Proposta: feature Cargo `online` + flag `--online`; credencial só no `env`; teste de CA-10 sobre o grafo do build padrão | [D-059](#d-059--feature-online-e-ca-10-proposta) |
| 2026-09-27 17:55 | Proposta: classificador só nas rotas que terminam em `explore`; demais rotas com `semantic_skipped` | [D-060](#d-060--gate-por-rota-proposta) |
| 2026-09-27 17:55 | Proposta: `RankedPath`, rescore dos paths do planner, lookahead de um nível e unidades por chunk | [D-061](#d-061--candidatos-e-unidades-proposta) |
| 2026-09-27 17:55 | Proposta: rascunho de `prompts/v1` e tetos 4 em voo / 24 requests | [D-062](#d-062--prompts-v1-e-tetos-proposta) |
| 2026-09-27 17:55 | Proposta: `provenance.online.discovery`, `Basis::RemoteClassifier`, `Item.semantic`; `interrupted` só por prazo de descoberta | [D-063](#d-063--envelope-online-e-interrupted-proposta) |
| 2026-09-27 17:55 | Proposta: cache por pergunta; `doctor --jev-probe` sintético; `install --online`; hooks e wrapper offline | [D-064](#d-064--cache-diagnóstico-e-integração-proposta) |
| 2026-09-27 18:01 | Usuário aprovou D-059 a D-064 e o início do Sprint 0 | [D-065](#d-065--aprovação-das-propostas-das-fases-4-e-5) |
| 2026-09-27 18:01 | Sprint 0 parcial: S4.0a, S4.0c (golden provisório) e S4.0d feitos; S4.0b aguarda a credencial; 131 verdes | [D-066](#d-066--sprint-0-parcial) |
| 2026-09-27 18:14 | S4.0b: gravação live contra `jev-1.13.0` confirma o contrato e `prompts/v1`; golden congelado; Sprint 0 concluído, 132 verdes | [D-067](#d-067--gravação-live-e-ponto-de-parada-0) |
| 2026-09-27 18:19 | S4.1–S4.6: flags `--online`/`--jev-*`, recusa sem a feature, credencial redigida, teste de CA-10 sobre o grafo resolvido, consentimento, CI nas duas configurações | [D-068](#d-068--configuração-credencial-e-garantia-offline) |
| 2026-09-27 18:24 | S4.7–S4.11: validação da resposta, thresholds estritos, batcher com tamanho exato, `trait Classifier` e `JevClient` (reqwest/rustls atrás da feature) | [D-069](#d-069--protocolo-validação-e-cliente-http) |
| 2026-09-27 18:28 | S4.12–S4.14: `WorkspaceReader` (elegibilidade, snapshot com sha256, preview) e unidades; ponto de parada 1, 152/159 verdes | [D-070](#d-070--leitura-do-workspace-e-ponto-de-parada-1) |
| 2026-09-27 18:37 | S4.15–S4.19: scheduler com `JoinSet`, fila limitada, teto em voo, limite de requests, auth e cancelamento abortam irmãs; 158/165 verdes | [D-071](#d-071--scheduler-mínimo) |
| 2026-09-27 18:54 | S4.20–S4.31: adaptador ligado ao `context_for_task` (gate por rota, admissão→seleção, merge aditivo, cache, status); piso online de 512 tokens; ponto de parada 2 (barra de merge da Fase 4), 182/191 verdes, p95 local 8,3 ms | [D-072](#d-072--composição-em-context_for_task-e-ponto-de-parada-2) |
| 2026-09-27 18:58 | S5.1: frescor — lote com fonte alterada não é enviado; evidência de arquivo alterado é descartada antes da saída; p95 local 21,9 ms | [D-073](#d-073--frescor-das-fontes) |
| 2026-09-27 19:02 | S5.2: retry por etapa e divisão ao meio no scheduler; cada tentativa conta no limite e revalida o frescor; 191/200 verdes | [D-074](#d-074--retry-e-divisão-de-lotes) |
| 2026-09-27 19:07 | S5.3: `429` com cooldown compartilhado (`Retry-After` em segundos ou data HTTP), sem ocupar vaga e cancelável; 196/205 verdes | [D-075](#d-075--429-e-cooldown-compartilhado) |
| 2026-09-27 19:07 | Teste manual autorizado com `--online` neste repositório: contrato, cache, gate por rota, status e credencial conferidos; rescore sem candidatos novos confirmado | [D-076](#d-076--teste-manual-com---online) |
| 2026-09-27 19:10 | S5.4: o cancelamento MCP chega ao request HTTP pelo drop estruturado (sem token extra); provado por teste e por mutação; 197/207 verdes | [D-077](#d-077--cancelamento-até-o-http) |
| 2026-09-27 19:14 | S5.5: prazo de descoberta com `interrupted` e evidência preservada; `--jev-max-source-bytes` limita a fonte renderizada; 200/210 verdes | [D-078](#d-078--prazo-de-descoberta-e-limite-de-fonte) |
| 2026-09-27 19:16 | S5.6: texto remoto sanitizado, limitado e com a credencial redigida; o `Retry-After` era o único vazamento possível; 201/212 verdes | [D-079](#d-079--redaction-de-texto-remoto) |
| 2026-09-27 19:28 | S5.7–S5.9: lookahead de um nível, `--jev-lookahead-max`, ganho além do Ripwire no status; ao vivo acha `src/budget.rs` (p=0,90), mas os 24 requests padrão se esgotam na admissão em chamada fria (decisão pendente); 2ª falha intermitente não reproduzida | [D-080](#d-080--lookahead-de-um-nível) |
| 2026-09-27 19:35 | Usuário escolheu previews de 4 KiB no lookahead e ordem por probabilidade; a frio com os padrões, `src/budget.rs` aparece (p=0,89) em 19 requests; 3ª falha intermitente | [D-081](#d-081--equilíbrio-do-lookahead) |

---

## D-001 — Seams de teste

Desenvolvimento por TDD em fatias verticais. Seams confirmados com o usuário:

1. **Núcleo `Broker`** (`context_for_task`, `context_after_edit`, `context_before_finish`,
   `status`) exercitado por sua API pública com um `Upstream` falso que responde com
   payloads gravados do ripwire real (`tests/broker.rs`).
2. **Superfície MCP ponta a ponta**: binário do broker via stdio (`tests/mcp_surface.rs`).
3. **Cliente upstream contra `ripwire --mcp` real** (spike da Fase 0,
   `tests/upstream_ripwire.rs`).

Os seams 2 e 3 usam um repositório git temporário e são pulados quando `ripwire` não está
no `PATH`.

## D-002 — Compatibilidade de protocolo SDK × ripwire

- `rust-mcp-sdk` 2.0.0 implementa o MCP `2026-07-28` **stateless**: o cliente não envia
  `initialize`; cada request leva `_meta` com a versão.
- ripwire 0.6.4 anuncia `2025-06-18` e **não** implementa `server/discover`, mas aceita
  `tools/list` e `tools/call` sem handshake.
- Resultado do spike: o cliente do SDK funciona com o ripwire atual. Não há negociação
  real de versão com o upstream; o risco fica registrado (PRD §20, "Mudança no contrato
  MCP do Ripwire") e é coberto pelos testes do seam 3.
- Dependência fixada exatamente em `=2.0.0`, com features `server`, `client`, `macros`
  e `stdio` e sem default features (PRD §7.5).

## D-003 — Superfície upstream e allowlist

`tools/list` do ripwire 0.6.4 retorna 33 verbos. Quatro não são read-only e nunca são
chamados nem expostos: `replace_symbol_body`, `insert_before_symbol`,
`insert_after_symbol` e `quality_baseline` (este cria um baseline, o que o CA-09 proíbe).

Verbos usados pelo broker: `explore`, `from_trace`, `find_symbol`, `fetch_body`, `impact`,
`memory_recall`, `situational_awareness`, `edit_check`, `affected` e `quality_delta`.

## D-004 — Formato dos payloads e parser tolerante

- `explore` e `from_trace` retornam `<ctx>`; `impact`, `affected` e `edit_check` retornam
  elementos XML-like precedidos de um comentário de legenda.
- `find_symbol`, `fetch_body`, `situational_awareness` e `quality_delta` retornam JSON
  dentro de `text`.
- `memory_recall` retorna texto com blocos `━━ path (relevance r) ━━`.
- Decisão: um leitor próprio e tolerante (`src/markup.rs`), limitado a elementos,
  atributos entre aspas, CDATA e comentários. Não usamos um parser XML estrito porque o
  ripwire não garante XML válido nos textos das assinaturas.
- Um payload que não pode ser lido vira a limitação `unparsed_upstream`; nada é inferido
  dele.

## D-005 — Timeout, reinício e classificação de erros

- Erro JSON-RPC do ripwire (ex.: `-32602`, símbolo inexistente) → `Refused`. Nunca é
  repetido (PRD §14.2).
- Falha de transporte ou processo → `Unavailable`: o processo é substituído e a chamada é
  repetida **uma única vez** (CA-07). Uma segunda falha é reportada.
- Timeout → o processo travado é encerrado e relançado, e a chamada lenta **não** é
  repetida. Retorna `Timeout`; a chamada seguinte usa o processo novo.
- O timeout do broker (`tokio::time::timeout`) é o autoritativo. O timeout do transporte
  do SDK fica em 2× esse valor, apenas como proteção.
- O processo é lançado por array de argumentos (`ripwire <workspace> --mcp`), sem shell.

## D-006 — Fixtures do ripwire

`tests/fixtures/ripwire/*.txt` foram gravadas do ripwire 0.6.4 real num repositório
sintético Python (auth/routes/tests/docs) com histórico de co-change. O cenário pós-edição
tem mudança de assinatura de `login` (2 → 3 parâmetros) e uma função nova complexa.
Nenhum código de outros projetos foi usado.

## D-007 — Orçamento e estimativa de tokens

- Estimativa: `ceil(bytes do JSON serializado do envelope / 4)`, aplicada ao envelope
  **inteiro**, incluindo metadados.
- Seleção gulosa na ordem do PRD §10.2. As limitações entram sempre. Quando um item com
  corpo não cabe, tenta-se o mesmo item sem o corpo (corpos sob demanda). Ao final, se a
  contabilidade estourar, os últimos itens adicionados são removidos.
- `budget` reporta `requested_tokens`, `estimated_tokens`, `truncated`, `shown`,
  `omitted` e `next_step`.

## D-008 — Deduplicação

A deduplicação roda depois da ordenação por prioridade, e a primeira ocorrência vence.
As chaves são símbolo (`path::symbol`), localização (`path:line`), corpo idêntico,
caminho de teste, `(kind, path, symbol)` de risco e `(kind, detail)` de limitação.
Motivo: o `explore` real devolve a mesma seção de documentação com dois nomes e `login`
tanto no ranking quanto em callers.

## D-009 — Roteamento de intenção

A ordem de avaliação em `mode=auto`:

1. Stack trace (Traceback, `panicked at`, linhas `File "...", line N` ou
   `path.ext:N`) → `debug` → `from_trace` com a trace literal.
2. Pedido de documentação/decisão sem verbo de mudança → `docs` → `memory_recall`.
3. Verbo de mudança (PT/EN) → `change`. Com símbolo nomeado: `find_symbol` +
   `fetch_body` + `impact`. Sem símbolo: `explore`.
4. Símbolo nomeado (entre crases, snake_case, camelCase, `a::b` ou `f()`) → `symbol` →
   `find_symbol` + `fetch_body`.
5. Caso contrário → `orient` → `explore`.

`mode` explícito (`orient`, `debug`, `change`, `review`) sobrepõe a classificação.
`review` usa `situational_awareness` da working tree. "Adicione autenticação à rota…" é
classificado como `change`, de acordo com o PRD §9.1/§12.1.

## D-010 — context_after_edit

`situational_awareness` recebe `files` como **string separada por vírgulas**, porque o
ripwire recusa um array. Depois vem `edit_check` para até `max_edit_checks` (5) símbolos.
`impact` é chamado somente para símbolos cuja mudança de contrato foi confirmada pelo
`edit_check` (PRD §9.2, "apenas quando necessário"). Callers incompatíveis têm prioridade
maior que o alcance transitivo. Nenhum corpo é incluído após uma edição.

## D-011 — Workspace guard

A raiz é canonicalizada uma vez, no início. Cada caminho recebido é resolvido
lexicalmente (`..`), e o maior prefixo existente é canonicalizado (seguindo symlinks).
O resultado precisa ficar sob a raiz e é repassado ao ripwire como caminho relativo. A
parte de arquivo dos símbolos `@FILE:LINE` passa pela mesma regra. Uma violação retorna
o erro estruturado `workspace_violation` **antes** de qualquer chamada upstream (CA-08).

## D-012 — `tools/list` contra servidor pré-2026

O teste e2e revelou que `request_tool_list` do SDK falha com "Not a ListToolsResult": a
resposta `2025-06-18` do ripwire não tem `cacheScope`, `ttlMs` e `resultType`,
obrigatórios no `2026-07-28`. `tools/call` funciona porque esses campos são opcionais no
`CallToolResult`. O cliente upstream agora envia `tools/list` por `request()` e lê a lista
do resultado genérico (`ServerResult::Result.extra.tools`). Isso é coberto por
`capabilities_are_discovered_from_a_pre_2026_server`.

## D-013 — context_before_finish e semântica de `strict`

- A ordem das chamadas é `situational_awareness` → `quality_delta` → `affected` (esta
  última só se houver arquivos alterados, semeada com eles).
- `attention_required`: há regressão não-minor em `quality_delta` (linhas `r`), risco
  `cochange_missing` ou `contract_change`, ou, com `strict=true`, achados `sev=minor`.
- `unknown`: `situational_awareness` ou `quality_delta` recusou, deu timeout ou respondeu
  algo ilegível, e não existe motivo conhecido para atenção. Uma regressão conhecida
  prevalece sobre a falta de evidência.
- `ready`: nenhum dos casos acima. Testes a rodar são obrigações **informadas** e não
  bloqueiam.
- Upstream indisponível (`Unavailable` após o único reinício) → erro estruturado, sem
  envelope (RF-12, CA-07).
- Interpretação: o CA-05 exige `attention_required` para qualquer regressão, o que
  conflita com a descrição de `strict` no §9.3. Prevaleceu o CA-05, e `strict` passou a
  apenas escalar achados menores. O `gating=0` do ripwire (regressões em símbolos novos
  não bloqueiam o gate dele) é ignorado: para o broker, regressão introduzida é regressão.
- `context_after_edit` também retorna `attention_required` quando há `contract_change`
  ou `cochange_missing`.

## D-014 — Allowlist e validação de capabilities

`REQUIRED_VERBS` é, ao mesmo tempo, a lista de verbos exigidos no `connect` (RF-01/RF-03:
falta de verbo → `incompatible_upstream`, com os nomes) e a allowlist aplicada em
**runtime** em todo caminho de chamada upstream (`guarded_call`). Verbos de edição e
`quality_baseline` não podem ser chamados.

## D-015 — Status e métricas

- As métricas ficam em memória e são expostas só no resource de status: chamadas, erros,
  duração total e upstream (µs), tokens pedidos/estimados, itens mostrados/omitidos,
  contagem por status, chamadas por verbo upstream e respostas truncadas.
- `last_error` guarda apenas o **tipo** (`upstream_refused`...), porque as mensagens do
  ripwire podem citar símbolos e caminhos.
- `--redact-workspace` oculta o caminho do workspace no status.
- Não há telemetria remota nem arquivo de log.

## D-016 — Resumo, limite por item e orçamento mínimo

- `summary` é inferência do broker, montada só com nomes, caminhos e contagens (foco,
  regressões, contratos, co-change, itens, testes e limitações). Texto do repositório
  nunca entra nele.
- Cada `content` é limitado a `max_item_tokens` (800 tokens = 3200 bytes, cortado em
  fronteira UTF-8) e gera a limitação `item_truncated`.
- `budget_tokens < 256` → `invalid_input` antes de qualquer chamada upstream: esse é o
  piso do envelope vazio mais limitações.

## D-017 — Schemas estritos das tools

O derive `JsonSchema` do SDK não aceita enums (entra em pânico) nem gera
`additionalProperties: false`. Os três schemas foram escritos à mão em `src/mcp.rs`, com
`additionalProperties: false`, `enum` em `mode` e `minimum`/`maximum` em
`budget_tokens`, e os argumentos são lidos com `serde(deny_unknown_fields)`. Os tipos do
SDK (`Tool`, `CallToolResult`, `ServerHandler`) continuam em uso. As tools levam
`readOnlyHint: true` e `openWorldHint: false`, e o resultado vai em `structuredContent`
e também como texto JSON.

## D-018 — Modo degradado

Se o ripwire não inicia ou não tem os verbos exigidos, o servidor MCP sobe mesmo assim.
Tools retornam erro estruturado com `isError: true`, e o status mostra
`available: false` e o tipo do último erro. Cada nova chamada tenta conectar de novo.
Motivo: RF-12 (o agente segue sem o broker, com a indisponibilidade explícita).

## D-019 — Medições da Fase 0

Números de `cargo run --release --example spike`, macOS, ripwire 0.6.4, repositório
Elixir com 104 arquivos e 952 símbolos:

| Medida | Valor |
| --- | --- |
| `tools/list` do ripwire (33 schemas) | 46.228 bytes (~11,5 mil tokens) |
| Schemas das 3 tools do broker | 2.936 bytes (~734 tokens) |
| `explore` frio / aquecido | 97 ms / 2,8 ms · 5,7 KB |
| `situational_awareness` frio / aquecido | 53 ms / 41 ms · 0,6 KB |
| `quality_delta` frio / aquecido | 260 ms / 232 ms · 0,2 KB |
| `context_for_task` ponta a ponta (20×) | p50 2,6 ms · p95 2,9 ms · ~1.767 tokens |
| `context_before_finish` | ~277 ms (dominado por `quality_delta`) |
| Overhead próprio do broker | ~0,13 ms por chamada (meta PRD: p95 < 50 ms) |

Outros resultados do spike:

- Timeout seguido de reinício e queda seguida de um reinício dentro da chamada foram
  validados com um binário substituto (`tests/common::flaky_ripwire`).
- Cancelamento: quando o cliente cancela, o future da tool é descartado junto com a
  espera upstream. O processo ripwire não é interrompido, e a chamada seguinte é
  atendida normalmente. Não há teste automatizado disso (ver D-021).
- Num repositório **sem commits**, `situational_awareness` recusa ("no git diff"). O gate
  então retorna `unknown`, e `context_after_edit` retorna `upstream_refused`.

## D-020 — Integração com agentes e toolchain

- Claude Code: skill `integrations/claude-code/skills/ripwire-broker/SKILL.md` e exemplo
  `mcp.json`.
- Codex: trecho `integrations/codex/AGENTS.md` e `config.toml`.
- As instruções para agentes estão em inglês, como os nomes e campos das tools.
- `rust-toolchain.toml` fixa o Rust 1.98.1, e `Cargo.lock` registra as dependências
  (§14.3). O teste `the_build_has_no_network_stack` garante que nenhum crate
  HTTP/TLS entre na árvore (CA-10).

## D-021 — Pendências conhecidas

- Cancelamento (RF-14) sem teste automatizado e sem repassar o cancelamento ao ripwire,
  que não expõe essa função.
- Não há limite de memória para o processo ripwire (§15.3); só o timeout está
  implementado.
- A deduplicação vale só dentro de uma resposta. O contexto incremental por sessão é da
  Fase 2.
- Não há reconexão automática do upstream fora de chamadas de tool.
- A skill foi validada só estruturalmente; a eficácia com Codex/Claude Code depende da
  avaliação A/B do §16.2.

## D-022 — Configuração sem `.env`

- O broker não tem segredos (é local e offline), então não há arquivo `.env`. A
  configuração vem só de argumentos (`--workspace`, `--ripwire`, `--timeout-ms`,
  `--redact-workspace`).
- Hosts MCP (Claude Code, Codex e outros) iniciam o servidor stdio como processo filho e
  repassam apenas `args`, `env` e o diretório de trabalho da própria configuração. Nenhum
  host carrega `.env` sozinho, e o diretório de trabalho varia, então o `.env` a aplicar
  seria ambíguo.
- Recomenda-se passar `--workspace` com caminho absoluto.
- Segredos futuros (bearer token do Streamable HTTP, Fase 4) virão de variável de
  ambiente definida na entrada do host (PRD §7.3), nunca de argumento ou arquivo
  versionado.
- Documentado na seção "Configuration" do README. Aceitar variáveis de ambiente como
  alternativa aos argumentos continua sendo uma opção, ainda não implementada.

## D-023 — Versão mínima do ripwire

- PRD §15.3 pede versão mínima além da allowlist. `Broker::connect` recusa com
  `incompatible_upstream` um ripwire abaixo de **0.6.4**, a versão em que as fixtures
  foram gravadas e os formatos de payload validados.
- A versão vem de `ripwire --version` (argumento em array, sem shell). Se não puder ser
  lida (`unavailable`/`unknown`), a conexão **não** é recusada: a validação dos verbos
  obrigatórios decide. Uma versão ilegível não prova incompatibilidade.
- Teste: `a_ripwire_older_than_the_minimum_is_rejected_at_startup`.

## D-024 — Revisão reconhecida no modo `auto`

- RF-04 exige distinguir "revisão", mas antes só o `mode=review` explícito chegava lá.
- Agora o roteador reconhece palavras **inteiras** `review`, `reviewing`, `revise`,
  `revisar`, `revisa`, `revisão` e `revisao`, de modo que "preview" não conta. A rota
  usa `situational_awareness` da working tree.
- Ordem de avaliação atualizada (substitui a de D-009): trace → **revisão** →
  documentação → mudança → símbolo → orientação. A revisão vem antes da mudança porque
  "review the change to X" pede revisão, não edição.
- Risco aceito: em inglês, "revise" também pode significar "modificar". Como é o
  imperativo comum em português, foi mantido; `mode` contorna o roteador quando ele erra.
- Teste: `a_review_request_is_recognized_without_an_explicit_mode`.

## D-025 — Orçamento conservador no caso incerto

- PRD §9.1: "Caso incerto → `explore` com orçamento conservador".
- Definição: é incerto o `orient` a que o modo `auto` chega por falta de qualquer sinal
  (trace, revisão, documentação, mudança ou símbolo). O `mode=orient` explícito e a
  mudança sem símbolo continuam pedindo o orçamento inteiro ao `explore`.
- Conservador = metade do `budget_tokens`, com piso de 256, **pedido ao ripwire**. O
  orçamento do envelope continua sendo o solicitado.
- A resposta traz a limitação `route_uncertain`, com `source.basis=broker_inference`,
  sugerindo `mode` ou um símbolo para uma resposta mais completa.
- Testes: `an_uncertain_task_is_explored_with_a_conservative_budget` e
  `an_explicit_orient_mode_asks_explore_for_the_whole_budget`.

## D-026 — Correlação entre tool call e chamadas upstream

- PRD §14.2. Cada tool call recebe um `request_id` monotônico, exposto em
  `provenance.request_id`. Isso é uma adição compatível ao schema v1 (RF-10).
- O status passa a ter `metrics.recent_requests`: as últimas 32 requisições, cada uma
  com `request_id`, tool, `outcome` (status do envelope ou tipo de erro), `total_us` e
  `upstream[]` (verbo, `us`, `outcome` = `ok` ou tipo de erro). Não há conteúdo, prompt,
  símbolo nem caminho.
- As chamadas upstream são atribuídas por um contexto **task-local** do tokio, e não por
  estado global. Por isso chamadas concorrentes não se misturam, e o teste verifica isso
  com `tokio::join!`.
- Erros estruturados não levam `request_id` (o formato de `BrokerError` não mudou). Eles
  aparecem em `recent_requests` com o `outcome` do erro.
- Teste: `each_answer_can_be_correlated_with_its_upstream_calls`.

## D-027 — Revisão código × PRD

Revisão feita a pedido do usuário. Decisões:

- **PRD atualizado** para refletir decisões já registradas: §7.4 (sem negociação com o
  upstream, D-002/D-012), §7.5 (schemas à mão, D-017), §8.3 e §9.3 (ordem das chamadas,
  semântica de `strict` e testes informados, D-013), §9.1 (exemplo do envelope real),
  RF-14 e Fase 0 (cancelamento pendente, D-021) e §15.3 (limite de memória pendente,
  D-021).
- **Decisões em aberto** marcadas com o estado no MVP: §21.1 (ambos os hosts
  integrados; o host dos hooks fica para a Fase 2) e §21.4 (o broker não bloqueia;
  falhar em CI fica para a Fase 4).
- **Lacunas menores resolvidas no PRD, não no código:**
  - Regras de roteamento ficam como constantes testadas e não são configuráveis (§14.4).
  - Testes de propriedade sobre fixtures substituem golden files (§14.4).
  - A métrica de verbo upstream não é desabilitável, porque fica só em memória (§16.1).
  - As métricas ficam no núcleo, e não em `McpObserver`, porque só o núcleo conhece a
    tool call de cada chamada upstream (§7.5).
  - O núcleo lê do SDK apenas a constante de versão de protocolo para o status (§7.5).
  - Os papéis `config` e `risk` ficam reservados no schema v1 (§10.1).
  - "Cache hit lógico" só se aplica na Fase 2 (§16.1).
- **Defeito corrigido:** o doc comment de `REQUIRED_VERBS` estava acima de
  `MIN_BUDGET_TOKENS` em `src/broker.rs`.
- Suíte após as mudanças: 48 testes passando e clippy limpo.

## D-028 — Plano das Fases 2 e 3

Plano completo em [plan-fases-2-3.md](plan-fases-2-3.md). Estado: **proposta, aguardando
aprovação do usuário**. As decisões D-029 a D-037 também são propostas até essa aprovação.

- TDD em fatias verticais: 30 fatias na Fase 2 (S2.1–S2.30) e 18 na Fase 3 (S3.1–S3.18),
  cada uma com o teste vermelho, o seam e o mínimo para verde.
- Seams: os três existentes (D-001) e três novos: `hook::handle` com `FakeUpstream`,
  a CLI do binário e o summarizer por comando com um script `sh`.
- Impacto mínimo: o código novo fica em módulos novos, e tudo vem desligado em
  `BrokerConfig::new`. Os 48 testes atuais não são editados.
- Validação contra os testes atuais: linha de base de 48 verdes. A tabela do §5 do plano
  lista cada mudança, os testes que ela toca e por que não quebram.
- Quatro pontos de parada, cada um com `cargo test`, clippy, fmt e changelog.

## D-029 — Contexto incremental por sessão (proposta)

- Um item já entregue e inalterado volta como referência enxuta: o mesmo `Item`, sem
  `content` nem `signature`. Um item alterado volta completo, porque o fingerprint inclui
  o sha256 do conteúdo.
- Testes e riscos repetidos saem da resposta e são contados em
  `budget.already_delivered`. Limitações nunca são suprimidas.
- Só o que sobreviveu ao orçamento é marcado como visto.
- `include_seen: true` ignora a sessão numa chamada, para o caso de o host ter compactado
  o contexto.
- Fica desligado em `BrokerConfig::new` e ligado no `serve`, com `--no-incremental` para
  desligar. Isso resolve a pendência de D-021 e ativa o "cache hit lógico" do §16.1.

## D-030 — Hooks nos dois hosts (proposta)

- Achado: `codex-cli 0.157.1` tem hooks (`UserPromptSubmit`, `PostToolUse`, `Stop`,
  `SessionStart`, em `~/.codex/hooks.json`) com o mesmo contrato do Claude Code 2.1: JSON
  no stdin com `session_id`, e `hookSpecificOutput.additionalContext` na saída.
- A diferença: no Codex, a edição chega como `apply_patch`, e os arquivos vêm nos
  cabeçalhos `*** Add/Update/Delete File:` e `*** Move to:`.
- Proposta para o §21.1: os dois hosts na mesma fatia, com um único `hook.rs` e dois
  leitores de entrada.
- O hook nunca falha para o host: exit 0, e as falhas viram `systemMessage` (§21.4).

## D-031 — Granularidade da automação (proposta)

Proposta para o §21.2:

- injeção automática no **primeiro prompt** e **depois de edições**;
- `--every-prompt` e o gate de `Stop` (`--gate`) são opt-in;
- opt-out por sessão com `#ripwire-off` / `#ripwire-on` no prompt, e global com
  `RIPWIRE_BROKER_HOOKS=off`;
- visualização: um `systemMessage` por injeção e `hook show` com referências e contagens,
  sem código.

## D-032 — Estado de sessão em disco (proposta)

- Arquivo por sessão, nomeado pelo sha256 do `session_id`, em `--state-dir`. O padrão é
  `$XDG_STATE_HOME/ripwire-broker`, fora do workspace (CA-09).
- Diretório `0700`, arquivo `0600` e gravação atômica (temporário + rename). Hooks
  concorrentes: o último a gravar vence, e isso fica documentado.
- Guarda só fingerprints, flags e referências `path#symbol`. Nunca guarda prompt nem código.

## D-033 — Instalação e diagnóstico (proposta)

- `doctor` verifica:
  - ripwire no PATH e versão mínima;
  - verbos obrigatórios;
  - workspace canonicalizável e git com commit (D-019);
  - state dir gravável e summarizer;
  - uma chamada de fumaça.

  Exit 0 se tudo passa, 1 se uma verificação obrigatória falha e 2 para uso inválido.
- `install` é dry-run por padrão. Com `--write`, faz merge JSON idempotente, preserva as
  chaves alheias e cria um `.bak`. O `config.toml` do Codex é só impresso, para não editar
  TOML sem parser.

## D-034 — Modelo local por subprocesso (proposta)

- `--summarizer-cmd "ollama run phi4"`: o comando é dividido em argv, sem shell; o texto
  entra pelo stdin e sai pelo stdout.
- Motivo: o CA-10 e o teste `the_build_has_no_network_stack` proíbem crates HTTP, e um
  subprocesso segue o mesmo modelo de execução do ripwire.
- Medição nesta máquina (ollama 0.34.4, phi4): 19 s na primeira chamada e 1 s aquecido.
  Isso exige cache e espera limitada (D-036).
- Um modelo remoto continua fora do escopo (§4.2).

## D-035 — Notas por módulo e cache (proposta)

- As notas são geradas só em `context_for_task`, a partir dos itens **incluídos** no
  envelope, agrupados pelos dois primeiros componentes do caminho (até 3 grupos).
- Cada nota é marcada com `generated: true`, `model`, `derived_from` e
  `basis: local_model`, e o texto vai dentro de `untrusted_repository_data`.
- Cache próprio, endereçado por conteúdo: `sha256("notes/v1" ‖ model_id ‖ evidência)`.
  Mudar o código, o modelo ou a versão do prompt invalida a entrada (§10.3).
- Os arquivos ficam em `--cache-dir`, fora do workspace, com `0600`.
- Interpretação registrada: "cache semântico" é o cache de conteúdo gerado, não uma busca
  por similaridade.
- Dependência nova: `sha2`, que não é crate de rede.

## D-036 — Espera limitada e fallback (proposta)

- A resposta espera a nota no máximo `--summarizer-wait-ms` (1.500 ms por padrão). Sem a
  nota, sai a limitação `note_pending` com o resumo determinístico.
- A geração continua em segundo plano, com limite rígido de `--summarizer-timeout-ms`
  (60 s por padrão), e grava no cache. Há no máximo uma geração em voo.
- Uma falha vira `summarizer_unavailable`, e os itens e o status não mudam.
- Nos hooks (processo curto) só entram notas do cache.

## D-037 — Extensões aditivas ao envelope v1 (proposta)

- `schema_version` continua `ripwire-broker.context/v1`.
- Campos novos, omitidos quando vazios: `notes[]` e `budget.already_delivered`.
- O enum `Basis` ganha `local_model`.
- Os três requests ganham o parâmetro opcional `include_seen`, e os schemas continuam
  estritos.

## D-038 — Revisão do plano das Fases 2 e 3

Um revisor independente conferiu o plano contra o código e os testes. Ele confirmou o
§5: nenhum dos 48 testes quebra, todos os literais de request usam `..Default`, o `sha2`
já está no lockfile via rust-mcp-sdk, e `io-util` e `fs` do tokio já estão disponíveis.
O que mudou no plano:

- **Bloqueador corrigido (CA-05).** A supressão por sessão poderia esconder o risco que
  justifica um `attention_required`. Agora `context_before_finish` não passa pelo filtro,
  e em `context_after_edit` os riscos que decidem o status também não. Novo teste: S2.3a.
- **D-029 revista.** A sessão fica desligada no `serve` (opt-in com `--incremental`),
  porque subagentes do Claude Code compartilham o processo MCP do pai. Ela fica ligada
  nos hooks. A mecânica em `envelope()` foi detalhada. O `Broker` ganha
  `session_snapshot`/`restore_session` (S2.8a).
- **D-031 revista.** Sai a variável `RIPWIRE_BROKER_HOOKS`, para manter D-022. O
  `hook show` vira `hook-log` e registra só contagens; `path#symbol` só com `--log-refs`
  (PRD §16.1).
- **Contratos dos hosts.** Não achei payloads reais de `apply_patch` gravados nesta
  máquina. A nova fatia S2.11a grava fixtures reais dos dois hosts e mede o limite de
  saída do hook. Os orçamentos dos hooks caem para 1.500 e 800, com um teto de 9.000
  caracteres (S2.12a).
- **Notas × sessão.** A evidência é montada com o item completo, antes do filtro
  (S3.6a).
- **Versão do modelo (§10.3).** Entra o `--summarizer-version-cmd`. Sem ele, o requisito
  fica parcialmente atendido, e o `doctor` avisa.
- **§21.3.** Depois do ponto de parada 3, o uso real de `session_hits` decide se o cache
  em disco (S3.15) é necessário.
- **Detalhes.**
  - As esperas dos testes são controladas por `Notify`, sem tempo real.
  - `wait_background()` usa clones `Arc`.
  - `check_version` e `ripwire_version` ficam públicas para o `doctor`.
  - `BrokerConfig` ganha um `Debug` manual.
  - `tokio` declara `io-util` explicitamente.
  - `install --write` é a única escrita dentro do workspace, e fica documentada.
  - A citação de CA-09 foi trocada pelo §15.1.
  - Os testes-guarda foram marcados como tais.

## D-039 — Aprovação do plano

- O usuário aprovou o plano revisado e confirmou todas as decisões propostas para as
  questões em aberto do PRD:
  - §21.1: hooks no Claude Code e no Codex;
  - §21.2: injeção no primeiro prompt e depois de edições, com o gate de conclusão
    opt-in;
  - sessão desligada no `serve` e ligada nos hooks;
  - modelo local por subprocesso.
- D-029 a D-037, com as revisões de D-038, deixam de ser propostas e viram decisões.
- Escopo autorizado agora: até o **ponto de parada 1** (S2.1–S2.8a, contexto incremental
  no núcleo).

## D-040 — Ponto de parada 1: contexto incremental no núcleo

Fatias S2.1–S2.8a implementadas por TDD no seam 1 (`tests/broker.rs`): 10 testes novos.

**Código**
- `src/session.rs` (novo):
  - `SessionMemory` guarda um conjunto de fingerprints sha256 e é serializável;
  - fingerprints:
    - item: caminho, linha, símbolo, assinatura e corpo;
    - teste: caminho e comando;
    - risco: tipo, caminho, símbolo e mensagem;
  - `SEEN_REFERENCE` é o `why_included` da referência enxuta e aponta para `include_seen`.
- `Broker`:
  - `BrokerConfig.incremental` vem desligado por padrão;
  - `session_snapshot()` e `restore_session()`;
  - o filtro fica em `envelope()`, depois de `cap_items` e antes de `budget::fill`;
  - só é memorizado o que ficou em `env` depois do orçamento, e referências não são
    memorizadas.
- `GATE_RISKS` (`cochange_missing`, `contract_change`) substitui os dois `matches!`
  duplicados. Esses riscos nunca são suprimidos, e `context_before_finish` não passa pelo
  filtro (CA-05).
- Envelope: `budget.already_delivered`, omitido quando 0.
- Status: `metrics.session_hits` (o "cache hit lógico" do §16.1) e
  `metrics.session.remembered`, só contagens.
- Helper de teste: `FakeUpstream::answer_seq`.

**Desvios do plano**
- `include_seen` existe só em `TaskRequest` e `EditRequest`, e não em `FinishRequest`,
  porque o gate nunca é filtrado e o parâmetro não teria efeito. Isso corrige D-037.
- `envelope()` passou a ter 8 parâmetros. Ficou um `#[allow(clippy::too_many_arguments)]`
  justificado, e a struct de parâmetros fica para a revisão.
- O teste S2.5 tinha uma premissa errada: com 400 tokens o primeiro item mantém o corpo.
  Ele foi reescrito para verificar que os itens cortados voltam completos e os entregues
  voltam como referência.

**Validação**
- S2.2, S2.3, S2.5 e S2.7 passaram na primeira execução: são guardas, ou o design já vinha
  de S2.1. Cada um foi validado por **mutação**: o corpo fora do fingerprint, o gate
  filtrado, a isenção removida e a memorização antes do orçamento. Todos os mutantes
  morreram.
- Suíte: 58 verdes (46 broker, 5 mcp_surface, 7 upstream_ripwire), clippy sem avisos,
  fmt ok. Nenhum teste antigo foi editado.

**Próximo:** S2.10–S2.11 (CLI) e depois S2.9. `--incremental` e `include_seen` ainda não
estão expostos na superfície MCP.

## D-041 — Contratos reais dos hooks e limite de saída

Fatia S2.11a. Os payloads foram capturados com um hook de captura registrado só num
diretório temporário: `--settings` no Claude Code, `.codex/hooks.json` do projeto no
Codex (com `--dangerously-bypass-hook-trust`) e `--ephemeral`. Nenhuma configuração
global foi alterada, e os artefatos das sessões foram apagados.

**Contratos**
- Os dois hosts mandam `session_id`, `cwd` e `hook_event_name`. Os eventos trazem:
  - `UserPromptSubmit`: `prompt`;
  - `PostToolUse`: `tool_name` e `tool_input`;
  - `Stop`: `stop_hook_active`.
- A edição chega de formas diferentes:
  - Claude Code: `Edit` com `tool_input.file_path` absoluto;
  - Codex: `apply_patch` com `tool_input.command`, com cabeçalhos
    `*** Add/Update/Delete File:` e `*** Move to:` e caminhos relativos ao `cwd`.
- O mesmo diretório chega como `/var/...` no Codex e `/private/var/...` no Claude Code.
  O guard de workspace já canonicaliza, então isso não é problema.
- O binário do Codex tem `systemMessage`, `decision` e `stop_hook_active`, com o mesmo
  formato de saída do Claude Code.
- Os fixtures ficam em `tests/fixtures/hooks/`, com os caminhos trocados por
  `__WORKSPACE__`.

**Limite de saída, medido**
- Claude Code: 9.411 caracteres chegam inteiros. Com 19.631, a saída vai para um arquivo
  e o modelo vê só uma prévia de cerca de 2.000 caracteres.
- Codex: mantém cerca de 10.000 caracteres e grava o resto num arquivo.
- Decisão: `MAX_CONTEXT_CHARS = 9.000`. O orçamento pedido ao broker é limitado a
  (9.000 − 200) / 4 = 2.200 tokens, e o JSON do envelope ocupa no máximo 4 bytes por
  token.

## D-042 — Ponto de parada 2: CLI e hooks

**Código**
- `src/cli.rs`: parse puro. A forma antiga sem subcomando continua equivalente a
  `serve`. Subcomandos: `serve`, `hook`, `hook-log`, `prompt`, `doctor` e `install`.
  Os três últimos ainda respondem "not implemented" e ficam para o ponto de parada 3.
  `--help` e `--version` agora saem com código 0 no stdout; antes saíam com código 2 no
  stderr.
- `ripwire_version` foi para `upstream.rs`, como função pública, para o `doctor` usar.
- MCP: `include_seen` em `context_for_task` e `context_after_edit`, e `--incremental` no
  `serve`. A sessão vem desligada por padrão.
- `src/hook.rs`:
  - `handle()` é o seam 4, e `run()` é o comando.
  - Primeiro prompt: `context_for_task`. Os seguintes só com `--every-prompt`.
  - Edição: `context_after_edit` só com os arquivos dentro do workspace. Não injeta
    nada se não houver novidade: todos os itens são referências, não há testes, e os
    riscos já foram entregues (`SessionMemory::knows_risk`).
  - `Stop`: `decision: block` só com `--gate`, status `attention_required` e sem
    `stop_hook_active`. `unknown` e o gate desligado viram `systemMessage`. `ready`
    fica em silêncio.
  - `#ripwire-off` silencia a sessão inteira (prompt, edição e `Stop`), e `#ripwire-on`
    religa.
  - Uma falha do broker vira `systemMessage`. O exit code é sempre 0, e o hook nunca
    bloqueia por falha própria.
- `src/state.rs`: um arquivo por sessão, com nome sha256, diretório `0700`, arquivo
  `0600` e gravação atômica. Um arquivo ilegível é tratado como sessão nova.
- `hook-log`: as últimas 5 injeções com contagens. `path#symbol` só com `--log-refs`.

**Testes**
- 21 novos:
  - `tests/cli.rs`: 3 de parse e 3 e2e com o binário (dois com o ripwire real);
  - `tests/hooks.rs`: 14 com os fixtures reais;
  - `tests/mcp_surface.rs`: 1 e2e.
- Mutações confirmaram os testes que nasceram verdes: o teto de tamanho, a regra do
  primeiro prompt e o filtro de workspace.
- O S2.17 original passava mesmo sem o filtro do hook, porque o guard do broker já
  recusa. Foi reforçado com um patch misto (dentro e fora do workspace).
- O S2.14 foi estendido: o opt-out não silenciava edições e `Stop`. Foi corrigido depois
  de confirmado o vermelho.

**Teste manual nos hosts reais**
- Os hooks do binário foram registrados num projeto temporário, num repositório Python
  pequeno.
- Claude Code: o `additionalContext` chegou no prompt (4 itens, ~394 tokens) e depois do
  `Edit` (1 item, 1 teste, 1 risco). O `systemMessage` apareceu, e o `Stop` rodou em
  422 ms.
- Codex: injetou depois do `apply_patch`. O primeiro prompt falhou, como descrito em
  D-043.
- Ajuste feito: uma ref sem símbolo agora sai como `path`, e não `path#`.
- Observação: `request_id` é sempre 1 nos hooks, porque cada evento é um processo novo.
- Os artefatos das sessões de teste foram apagados.

**Suíte:** 79 verdes (46 broker, 6 cli, 14 hooks, 6 mcp_surface, 7 upstream_ripwire),
clippy sem avisos, fmt ok.

## D-043 — Achado: roteador escolhe o identificador errado

- **Sintoma:** o prompt do Codex "Using apply_patch, rename the parameter 'token' of
  validate_token…" roteia para `change` com o símbolo `apply_patch`, o primeiro
  identificador com cara de código. `find_symbol` recusa, e o prompt recebe só o aviso
  "no context (upstream_refused)".
- **Causa:** comportamento da Fase 1 (`router::named_symbol`, sem fallback quando o
  símbolo não existe).
- **Opções:**
  1. quando `find_symbol` não encontrar o símbolo, cair para `explore` com uma
     limitação;
  2. tentar os identificadores seguintes do texto.

  As duas mudam o comportamento verificado por dois testes existentes
  (`status_reports_operations_without_sensitive_content` e
  `each_answer_can_be_correlated_with_its_upstream_calls`).
- **Estado:** pendente de decisão do usuário. Nada foi alterado.

## D-044 — Símbolo inexistente cai para `explore`

Decisão do usuário sobre D-043: fallback para `explore`.

- Quando o roteador escolhe um símbolo (intenção `symbol` ou `change`) e `find_symbol`
  **recusa**, o broker chama `explore` com o orçamento inteiro e acrescenta a limitação
  `symbol_not_found` (`basis: broker_inference`), que cita o nome e sugere crases.
  `impact` não é chamado. A intenção relatada continua a do roteador. Isso atende o risco
  "Roteador escolher verbo errado" do PRD §20 (fallback para `explore`).
- Timeout e indisponibilidade continuam sendo erro: só a recusa significa "não está no
  repositório". O teste-guarda é `a_find_symbol_timeout_is_still_an_error`.
- O final de `context_for_task` foi extraído para `finish_task`, para as duas rotas
  compartilharem os filtros de docs e corpos e o envelope. `symbol_context` recebe a
  resposta de `find_symbol` já obtida.
- Dois testes antigos foram ajustados, como anunciado em D-043:
  - `each_answer_can_be_correlated_with_its_upstream_calls`: a requisição com erro passou
    a usar timeout, e o propósito do teste (correlacionar uma falha) continua;
  - `status_reports_operations_without_sensitive_content`: `errors` passou de 1 para 0 e
    `upstream_calls` de 2 para 3. `last_error` continua `upstream_refused`, e a
    verificação de vazamento continua.
- Verificação com o ripwire real: o prompt do Codex de D-043 agora recebe 2 itens
  (`login`, `validate_token`) e a limitação `symbol_not_found`.
- Suíte: 81 verdes.

## D-045 — Ponto de parada 3: wrapper, diagnóstico e instalação

Fatias S2.25–S2.30 por TDD no seam 5 (binário), com 6 testes e2e novos. Com isso a
**Fase 2 está concluída**.

**Código**
- `src/local.rs`: `launch()` sobe o ripwire e o broker no próprio processo, para os
  comandos de uso único. O hook passou a usá-la, e o `connect` privado dele saiu.
- `prompt`: imprime a tarefa, uma linha em branco e o contexto dentro de
  `<ripwire-broker-context untrusted="true">`. Numa falha, imprime só a tarefa, com o
  motivo no stderr e exit 0. Nunca inventa contexto (RF-12).
- `src/doctor.rs`: sete verificações.
  - As que decidem o exit code (1 se alguma falhar): `ripwire_binary`,
    `ripwire_version`, `workspace`, `required_verbs` e `smoke_call`.
  - As que só avisam (`warn`): `git_history` (sem commit, o gate responde `unknown`,
    D-019) e `state_dir`.
  - Uma verificação que depende de outra que falhou é pulada (`skip`).
  - Saída em texto ou `--json`. O `doctor` também aceita `--state-dir`, para não gravar
    no diretório real durante os testes. `check_version` ficou pública, e
    `StateStore::remove` foi criado para a sonda de gravação.
- `src/install.rs`: calcula um plano de mudanças e só aplica com `--write`.
  - Claude Code: `.mcp.json` e, com `--hooks`, `.claude/settings.json` do workspace, com
    `--workspace` fixo.
  - Codex: `hooks.json` em `--codex-home` (padrão `~/.codex`), sem `--workspace`, porque
    os hooks do Codex são globais e seguem o `cwd` de cada sessão. O `config.toml` é só
    impresso, incluindo `[features] hooks = true`.
  - Os hooks do broker são reconhecidos pelo comando (`ripwire-broker` + ` hook `) e
    substituídos, nunca duplicados. As chaves e os hooks alheios são mantidos.
  - O backup `.bak` é criado uma vez e guarda o original do usuário. A ordem das chaves
    JSON é normalizada, porque o serde_json não preserva a ordem sem a feature
    `preserve_order`, e ela não foi adicionada.
- `main.rs`: todos os subcomandos estão implementados, e não sobrou nenhum "not
  implemented".

**Documentação**
- README: tabela de comandos, "Incremental context", "Automatic mode (hooks)", `install`
  e `doctor`, e o wrapper `prompt`.
- Novos `integrations/claude-code/settings.json` e `integrations/codex/hooks.json`. O
  `integrations/codex/config.toml` ganhou o comentário sobre `hooks = true`.
- A skill e o `AGENTS.md` cobrem o contexto já injetado, as referências "already
  delivered", `include_seen` e `symbol_not_found`.
- PRD:
  - §8.4, com o estado da Fase 2;
  - §9.1 e §9.2, com `include_seen`, o fallback `symbol_not_found` e a nota sobre
    `already_delivered`;
  - §16.1, com `session_hits`;
  - §19, com a Fase 2 implementada;
  - §21.1 e §21.2 resolvidos;
  - §21.3, com o estado.
- A pendência de D-021 "a deduplicação vale só dentro de uma resposta" foi resolvida
  (D-040).

**Verificação manual**
- `doctor` neste repositório: todas as verificações ok e um `warn` real, porque o
  repositório ainda não tem commit, então o gate responderia `unknown`. A chamada de
  fumaça levou 47 ms, com 9 itens.
- `install codex --hooks` em dry-run não escreveu nada.

**§21.3:** ainda não há medição de `session_hits` em uso real. Isso exige usar os hooks
por alguns dias. A decisão sobre o cache em disco da Fase 3 (S3.15) fica com o usuário.

**Suíte:** 87 verdes (48 broker, 12 cli, 14 hooks, 6 mcp_surface, 7 upstream_ripwire),
clippy sem avisos, fmt ok e nenhum link quebrado no changelog.

## D-046 — Fase 3 com cache em memória

- O usuário mandou seguir para a Fase 3. Vale a recomendação do ponto de parada 3: cache
  de notas **só em memória** (S3.5). O cache em disco (S3.15) fica adiado até haver uma
  medição de `session_hits` em uso real (§21.3).
- Consequência: só o servidor MCP (`serve`), que vive a sessão inteira, gera e reaproveita
  notas. Hooks, `prompt` e `doctor` são processos curtos e não geram notas. A fatia S3.17
  ("hooks usam só notas do cache") perde o objeto e sai do plano: sem cache persistente,
  os hooks nunca recebem notas nem a limitação `note_pending`.
- Fatia nova S3.19: o `doctor` verifica o summarizer configurado (se o programa existe) e
  avisa quando falta `--summarizer-version-cmd`, porque sem ele o §10.3 fica parcialmente
  atendido (D-035). O `doctor` não roda o modelo, porque a primeira chamada leva cerca de
  19 s.

## D-047 — Códigos de terminal na saída do modelo

- **Achado:** o teste opt-in com o modelo real (`ollama run phi4`, ollama 0.34.4) revelou
  que o ollama emite códigos de terminal (`ESC[4D ESC[K`, a quebra de linha
  redesenhada) mesmo com o stdout numa pipe. O saneamento tirava só o caractere `ESC` e
  deixava `evid[4D[Kevidenced` na nota. Os dublês não mostravam isso.
- **Correções:**
  - `notes::sanitize` remove sequências CSI (`ESC [ … final`) e OSC (`ESC ] … BEL|ST`)
    inteiras. Teste: `terminal_redraw_codes_from_a_model_cli_are_removed`, com o texto
    gravado do phi4. O S3.10 ficou mais rígido (`Ignore previous…` sem o resto de
    `[31m`).
  - Mesmo sem os códigos, o redesenho duplica pedaços de palavras (`functio` +
    `functionalities`). Emular um terminal seria exagero. O `doctor` avisa quando o
    comando é `ollama run` sem `--nowordwrap`, e o README recomenda a flag.
- **Verificação:** com `ollama run --nowordwrap phi4`, a nota sai limpa e coerente, em
  4,6 s aquecido.

## D-048 — Ponto de parada 4: Fase 3

**Código**
- `src/summarizer.rs`:
  - `trait Summarizer` (`summarize`, `model_id`, `program`);
  - `CommandSummarizer`: argv sem shell, prompt no stdin e nota no stdout;
    `kill_on_drop` e um limite rígido (`--summarizer-timeout-ms`);
  - `--summarizer-version-cmd` roda uma vez, e o sha256 da saída entra no `model_id`.
- `src/notes.rs`:
  - agrupamento por módulo (até 2 níveis de diretório, até 3 grupos, em ordem de
    ranking), evidência de até 2.000 caracteres, prompt `notes/v1`;
  - chave sha256 com as partes prefixadas pelo tamanho;
  - saneamento: remove ANSI e caracteres de controle, e corta em 600 caracteres;
  - `NoteEngine`: cache em memória, no máximo uma geração em voo, espera limitada por
    polling a cada 5 ms, segundo plano e `wait_background()`;
  - limitações `note_pending`, `summarizer_unavailable` e `notes_omitted`.
- `Broker`:
  - `BrokerConfig.summarizer` / `summarizer_wait`, com `Debug` via bound no trait;
  - `envelope_full()` devolve também a versão completa de cada item incluído, resolvendo
    as referências de sessão;
  - `complete_task` e `attach_notes` rodam só em `context_for_task`, pelas duas rotas
    (normal e fallback `symbol_not_found`);
  - as notas passam pelo filtro de sessão e entram em `budget.already_delivered`;
  - status `summarizer` só com contagens.
- `budget::add_notes`: as notas entram depois dos itens. Acima do orçamento, saem
  primeiro as notas, depois os itens de menor ranking, contados como omitidos, e a
  limitação `notes_omitted` é atualizada.
- `Envelope.notes` e `Basis::LocalModel` são aditivos ao v1 (D-037).
- CLI:
  - `serve`: `--summarizer-cmd`, `--summarizer-version-cmd`, `--summarizer-wait-ms` e
    `--summarizer-timeout-ms` (as três últimas exigem `--summarizer-cmd`);
  - `doctor`: `--summarizer-cmd` e `--summarizer-version-cmd`.
  - No `main`, uma configuração de modelo quebrada avisa no stderr e sobe sem notas.
- `tokio` declara `io-util` explicitamente (D-038).

**Testes:** 23 novos.
- `tests/notes.rs`: 15, no seam 1, com `FakeSummarizer` controlado por `Notify`.
- `tests/summarizer.rs`: 5, mais 1 opt-in com o modelo real, no seam 6, com scripts `sh`.
- `tests/cli.rs`: 2.
- `tests/mcp_surface.rs`: 1 e2e com um script como modelo sobre o ripwire real.

**Desvios de TDD**
- A implementação de S3.2 foi além do mínimo, porque escrevi o motor inteiro de uma vez.
  Por isso S3.3–S3.11 nasceram verdes. Todos foram validados por mutação:
  - morreram os mutantes de grupos=4, cache desligado, chave sem evidência, gerações em
    paralelo, saneamento desligado e falha ignorada;
  - o mutante "sem checagem de encaixe" sobreviveu: o reajuste seguinte já garantia o
    orçamento. O código redundante foi **removido**.
- A mutação de gerações em paralelo **travou** o S3.8 em vez de falhar. Os testes agora
  esperam o segundo plano com `settle()` (timeout de 10 s).
- O S3.2 original ("item cortado nunca chega ao modelo") comparava substrings. `login`
  aparece legitimamente no corpo de `home_route`, então passou a verificar as linhas de
  evidência (`- path#symbol |`).
- O S3.8 precisou de `yield_now()`, porque o runtime de teste tem uma thread só.
  Continua determinístico.
- Fora do plano: S3.15 foi adiada e S3.17 removida (D-046). S3.19, o `doctor` do
  summarizer, entrou.

**Validação com o modelo real:** o teste opt-in com o phi4 levou 18,6 s a frio e
4,6–7 s aquecido. A nota sobre `src` foi coerente com a evidência (D-047).

**Documentação:** README ("Architectural notes (local model)", flags, teste opt-in), skill
e `AGENTS.md` (notas são pistas geradas; verificar `derived_from`), PRD §9.1 (`notes[]`),
§10.3 (estado), §16.1 (métricas de notas), §19 (Fase 3) e §21.3.

**Suíte:** 110 verdes, mais 1 ignorado (opt-in), clippy sem avisos e fmt ok.

**Pendências:**
- o cache de notas em disco (S3.15) depende da medição de `session_hits` em uso real
  (§21.3);
- o `request_id` dos hooks é sempre 1 (D-042);
- o cancelamento continua pendente, como estava (D-021).

## D-049 — Cancelamento pelo cliente e status que não trava

- **Correção de D-019.** D-019 dizia que, quando o cliente cancela, o future da tool é
  descartado. Não era verdade. No `rust-mcp-sdk` 2.0.0, o `handle_cancelled_notification`
  padrão não faz nada, e o handler de cada requisição roda numa tarefa própria até o fim.
  O broker não respeitava o cancelamento (RF-14).
- **Como foi resolvido.** O SDK não entrega o id JSON-RPC ao `ServerHandler`. Trocar
  para `ServerHandlerCore` obrigaria reimplementar o despacho e as checagens de
  capability. Em vez disso:
  - o `McpObserver` vê cada requisição bruta e enfileira o id de cada `tools/call` sob a
    chave (tool, argumentos);
  - o handler pega o id de volta e registra um `Notify`;
  - `handle_cancelled_notification` dispara o sinal, e um `tokio::select!` descarta o
    `dispatch`;
  - um cancelamento que chega antes do handler fica guardado ("early").
  - Duas chamadas idênticas e simultâneas podem trocar de id. Cancelar qualquer uma
    interrompe trabalho idêntico, o que é aceitável.
  - O §7.5 fica valendo: as métricas continuam no núcleo, e o observador só correlaciona
    ids.
- **Núcleo:** `traced()` ganhou a guarda `Unfinished`. Uma tool call descartada antes
  de terminar é registrada com `outcome: "cancelled"` e as chamadas upstream já
  concluídas, e soma em `metrics.tools.*.cancelled`. Os spans agora ficam num
  `Arc<Mutex>` compartilhado com a guarda.
- **Achado no mesmo teste:** o status sondava o ripwire com `list_tools` sem limite de
  tempo. Com o ripwire ocupado numa chamada longa, o status travava até o timeout
  (60 s). A sonda agora espera no máximo `STATUS_PROBE` = 1 s, e o novo campo
  `upstream.busy` indica que o ripwire não respondeu a tempo (RF-13).
- **Testes:**
  - `a_cancelled_call_stops_its_upstream_work_and_is_recorded`, no seam 1, com
    `FakeUpstream::hold`;
  - `a_client_cancellation_stops_the_tool_call`, e2e com JSON-RPC bruto, porque o teste
    precisa controlar os ids, e um ripwire falso em Python cujo `explore` leva 30 s.
    Hoje a resposta chega em cerca de 1,8 s; sem a correção, só depois de 30 s.
  - A primeira versão do e2e tinha uma corrida: cancelava depois de 500 ms fixos, às
    vezes antes de a chamada começar. Agora espera um marcador gravado pelo ripwire
    falso.
  - Mutação sem o `handle_cancelled_notification`: o teste falha. Rodado 3 vezes seguidas,
    passou em todas.
- **Limite:** a chamada que já está no ripwire não é interrompida, porque o ripwire não
  expõe cancelamento.

## D-050 — Limite de memória do ripwire por supervisor

- **Problema:** quem lança o ripwire é o SDK (`create_with_server_launch`), e ele não
  expõe o PID nem aceita `pre_exec`. Além disso, o macOS não impõe `RLIMIT_AS`.
- **Solução:** com `--ripwire-max-rss-mb N`, o broker lança o próprio binário como
  supervisor:
  `ripwire-broker __supervise --max-rss-mb N -- ripwire <ws> --mcp`.
  - O supervisor herda o stdio, mede o RSS do filho com `ps -o rss=` a cada 200 ms e, se
    passar do limite, mata o processo e sai com 137.
  - O SDK vê a conexão cair, e o reinício controlado de D-005 assume.
  - Não usa shell e funciona igual no macOS e no Linux. Sem a flag, nada muda.
  - Tudo depois de `--` é argv literal, e `--help`/`--version` só valem antes dele.
- **Limites:**
  - O intervalo de 200 ms significa que uma chamada rápida pode terminar antes da
    medição. O limite serve para processos que crescem ao longo do tempo.
  - Se o supervisor levar `SIGKILL`, o ripwire fica órfão até ler EOF no stdin.
- **Testes:**
  - seam 5: o supervisor repassa o stdio abaixo do limite, e um `python3` que aloca
    300 MiB é morto em segundos;
  - parse de `--ripwire-max-rss-mb`;
  - seam 3, com o ripwire real: ele funciona sob o supervisor, e um limite de 1 MiB
    provoca a morte e o reinício (`restarts ≥ 1`).
  - A primeira versão do teste de seam 3 supunha a morte antes da primeira resposta, o
    que contradiz a medição a cada 200 ms. Foi reescrita.
  - Mutação "supervisor nunca mata": os dois testes falham.

## D-051 — Pendências técnicas fechadas

- **`request_id` nos hooks.** `SessionState.next_request` guarda o próximo id, e o
  `Broker` ganhou `next_request_id()` e `resume_request_ids()`, que nunca volta para
  trás. Teste: `request_ids_keep_counting_across_hook_processes`. Resolve a observação
  de D-042.
- **Refatoração.** A struct privada `Shape` (tool, intent, status, verbos, orçamento,
  `suppress_seen`) substitui os 7 parâmetros de `envelope()` e `envelope_full()`, e os
  dois `#[allow(clippy::too_many_arguments)]` saíram (D-040). Não entrou teste novo: a
  suíte inteira protege o comportamento.
- **D-021 fechada:**
  - cancelamento, em D-049;
  - limite de memória, em D-050;
  - deduplicação por sessão, em D-040.

  Continua aberta a reconexão do upstream fora de chamadas de tool, que não foi pedida.
- **PRD:** RF-14, §15.3, §9.4 (`busy`) e a Fase 0 foram atualizados. O README ganhou a
  flag, o `busy` e o cancelamento.
- **Suíte:** 118 verdes, mais 1 opt-in, clippy sem avisos e fmt ok.
- **Fora do escopo:** nenhum commit novo; o último é o `3d1b85a`.

## D-052 — Code review das Fases 2 e 3

Revisão independente de `session`, `hook`, `state`, `cli`, `local`, `doctor`, `install`,
`notes`, `summarizer`, `supervise` e das mudanças no núcleo. Os 9 achados foram conferidos
contra o código. O nº 1 também foi conferido no SDK: `rust-mcp-transport` 2.0.0
`stdio.rs:185-191` usa `kill_on_drop` e `process_group(0)` e só mata o PID lançado.

| # | Grav. | Achado |
| --- | --- | --- |
| 1 | alta | O reinício mata só o supervisor; o ripwire fica órfão e sem limite de memória |
| 2 | alta | `install` só põe aspas em caminhos com espaço: injeção de shell nos hooks |
| 3 | média | `</ripwire-broker-context>` no conteúdo fecha o bloco não confiável do `prompt` |
| 4 | média | Itens marcados como entregues antes de `add_notes`, que pode removê-los |
| 5 | média | Uma chamada cancelada durante a espera da nota já marcou itens como entregues |
| 6 | média | O `Stop` do hook marca testes e riscos como entregues sem enviá-los ao modelo |
| 7 | baixa | Hooks paralelos repetem `request_id` e perdem memória (última gravação vence) |
| 8 | baixa | A reconexão segura a trava do broker e bloqueia o status por até 60 s |
| 9 | baixa | `Inflight` nunca libera entradas e guarda o texto das tarefas |

Decisão do usuário: corrigir os 9 por TDD, com um teste vermelho por achado.

## D-053 — Correções do code review

Cada achado de D-052 ganhou um teste vermelho, confirmado antes da correção.

1. **Órfão do supervisor.** O monitor da memória virou um processo separado, `__watch`,
   lançado pelo `__supervise`. Ele mata o ripwire acima do limite e também assim que o
   supervisor some: `parent_id()` muda depois do `SIGKILL` que o SDK envia no reinício. O
   supervisor só espera o filho e devolve 128+sinal. Se o vigia não puder ser lançado, o
   supervisor se recusa a rodar. Teste: `killing_the_supervisor_does_not_orphan_ripwire`.
2. **Injeção de shell no `install`.** `quote()` agora põe aspas simples sempre e escapa `'`
   como `'\''`. Teste: um workspace chamado `it's;touch X;$(touch X)` é instalado, e o
   comando do hook é executado por `sh`. `X` não aparece.
3. **Delimitador do `prompt`.** `<` e `>` do JSON viram `\u003c`/`\u003e`, e o JSON
   continua válido e com o mesmo conteúdo. Teste com o ripwire real: um arquivo com
   `</ripwire-broker-context>` e "SYSTEM: …" deixa exatamente uma tag de fechamento, a
   última.
4. e 5. **Memória só do que chegou.** A gravação saiu de `envelope_full` e virou
   `remember()`, o último passo de cada tool, sobre o envelope final (itens, testes,
   riscos e notas). Um corte do `add_notes` e um cancelamento durante a espera da nota não
   marcam mais nada. Testes: uma varredura de orçamentos de 300 a 1.500 exige que toda
   referência tenha sido entregue antes; e um cancelamento durante a espera não deixa
   referências.
6. **Hooks só gravam o que o modelo viu.** A memória da sessão só é salva quando a saída
   tem `hookSpecificOutput` ou `decision`. Um `systemMessage` sozinho (aviso ao usuário,
   falha, `Stop` sem bloqueio) não conta. Teste: um `Stop` só com aviso, seguido de uma
   edição, ainda mostra o teste.
7. **Hooks paralelos.** `StateStore::lock` aplica `File::lock` exclusivo (Rust ≥ 1.89) num
   arquivo `<sha>.lock` por sessão, e o `hook::run` o segura de carregar até salvar. Se a
   trava falhar, o hook roda mesmo assim. Teste e2e com o ripwire real: dois hooks em
   paralelo na mesma sessão deixam ids 1 e 2 no `hook-log`. Passou 3 vezes seguidas.
8. **Status durante a reconexão.** `status_json` usa `try_lock`. Se a trava estiver
   ocupada por uma reconexão, responde como degradado com `upstream.reconnecting: true`.
   Teste: um ripwire que falha na primeira execução e trava nas seguintes. O status
   responde em até 3 s enquanto a tool call reconecta.
9. **Registro de cancelamento.** Uma chave some quando sua fila esvazia, porque as chaves
   guardam o texto da tarefa. Um cancelamento só fica guardado se a chamada ainda espera o
   handler. O status ganhou `inflight` (`tracked_calls`, `early_cancels`), só com
   contagens. Teste: duas chamadas concluídas e um cancelamento atrasado deixam `0/0`.

**Validação**
- As mutações do nº 8 (trava bloqueante) e dos anteriores morreram.
- Testes com corrida foram estabilizados com marcadores gravados pelos processos falsos,
  nunca com espera fixa:
  - o nº 8, que a princípio passou pelo motivo errado;
  - o nº 5, que esperava 30 s por nada e agora espera 2 s.
- Suíte: 127 verdes, mais 1 opt-in, clippy sem avisos e fmt ok.

## D-054 — Publicação no CeciApp

- Pedido do usuário: criar o projeto no CeciApp, que pode ser público.
- Criado https://github.com/CeciApp/ripwire-broker (público) com `gh repo create --source .
  --push`. O branch padrão é `master`, com os commits `3d1b85a`, `a352a65` e `2755d57`, e
  o remoto é `origin` (SSH).
- Verificação antes de publicar:
  - não há segredos, tokens nem caminhos pessoais nos arquivos versionados;
  - os caminhos `/private/var/folders/...` nos fixtures do ripwire são diretórios
    temporários aleatórios do macOS;
  - o único dado pessoal público é o autor dos commits (nome e e-mail do git).
- O repositório não tem arquivo de licença. Sem licença, o código fica visível, mas não
  há permissão explícita de uso. A escolha da licença fica com o usuário.

## D-055 — Licença MIT

- Pedido do usuário: licenciar o projeto como MIT.
- `LICENSE` com o texto MIT padrão e "Copyright (c) 2026 Antonio Quental", o autor dos
  commits. O repositório fica na organização CeciApp, mas a titularidade segue o autor.
  Se o titular dever ser a organização, basta trocar a linha de copyright.
- `Cargo.toml` ganhou `license = "MIT"` (SPDX), e o README ganhou uma seção "License".
- Isso resolve a pendência de D-054.

## D-056 — Fusão do adaptador `--online` no PRD

- Pedido do usuário: fundir a spec do adaptador `--online` no PRD pai, em
  `spec/ripwire-broker-mcp.md`, versão 0.3, sem apagar os arquivos Jev.
- A fonte pinada era a spec Jev **v0.2.1**, mas `spec/jev-integration-prd.md` é a
  **v0.1**, e a única outra cópia no disco é idêntica. Vários itens do roadmap pedido
  não existem na v0.1 (`RankedPath`, `prompts/v1`, Sprint 0, `doctor --jev-probe`,
  rescore, 24 requests). O trabalho parou e o usuário escolheu a opção (b): fundir com
  a v0.1 e marcar esses itens como *sem fonte na v0.1*, como lacunas e decisões em
  aberto, não requisitos.
- Os blocos `### Fase 0` a `### Fase 3` ficaram byte a byte iguais ao pai. `Fase 4 —
  Times e CI` virou `Fase 6 — Times e CI`, com o mesmo conteúdo, e o §21.4 passou a
  apontar para a Fase 6. As novas Fases 4 e 5 anotam a fonte de cada item.
- Fora do Roadmap mudaram só o cabeçalho, o sumário, §6.4–6.6 (sem a comparação
  longa e sem números SWE-bench de terceiro), a caixa "somente --online" no §7.1, a
  nota aditiva do envelope no §9.1, o RF-15 ponte, dois riscos, o §21.5 e as
  referências (sai jevgrep, entra TypeSafe). "jevgrep" aparece uma vez, como
  inspiração.
- O §23 transporta invariantes, protocolo, tipos, prompt, composição, tetos, CLI,
  envelope, cache, segurança, falhas, métricas, RF-ONLINE-01..15, RNF,
  CA-ONLINE-01..15, testes, avaliação, riscos e decisões em aberto da v0.1.
- Não transportados, por instrução da fusão: engenharia reversa, comparação longa,
  SWE-bench de terceiro, pipeline de 8 estágios, 32 em voo, hard stop de 50.000,
  papéis, passagem relacional e fronteira remota de diretórios. Com isso,
  RF-ONLINE-03 foi substituído, RF-ONLINE-06 revogado e CA-ONLINE-04
  parametrizado. O §23.18 lista cada item e onde aparecia.
- Regras da fusão: nenhuma tool MCP nova; nada de classificador em
  `context_after_edit` nem `context_before_finish`; CA-10 e offline padrão
  inalterados; `jev-latest` nunca é padrão; probabilidade nunca vira caller, teste
  ou impacto.
- O avulso Jev já pode ser removido sem perder norma da v0.1. Se a v0.2.1 aparecer,
  ela precisa ser reconciliada com o §23.17 antes da remoção.

## D-057 — Reorganização das specs

- O usuário moveu `ripwire-broker-prd.md` (v0.2), `jev-integration-prd.md` (v0.1) e
  `merge.md` (o pedido de fusão de D-056) para `spec/old/`, sem alterar o conteúdo.
- `spec/ripwire-broker-mcp.md` (v0.3) passa a ser o PRD vigente. O README aponta para
  ele, e o §23 cita a fonte Jev em `spec/old/`.
- O D-056 continua citando os caminhos da época.
- `.DS_Store` entrou no `.gitignore`.

## D-058 — Plano das Fases 4 e 5

Plano completo em [plan-fases-4-5.md](plan-fases-4-5.md). Estado: **proposta, aguardando
aprovação do usuário**. As decisões D-059 a D-064 também são propostas até essa aprovação.

- Fonte: PRD §19 (Fases 4 e 5) e §23. Cada lacuna *sem fonte na v0.1* do §23.17 recebe
  uma proposta explícita (tabela do §1 do plano).
- Sprint 0 (S4.0a–S4.0d): `RankedPath` sem mudança de comportamento, registro live contra
  `jev-1.13.0` com corpus sintético, golden de `prompts/v1` e spike das unidades do Ripwire.
- Fase 4: S4.1–S4.31. Fase 5: S5.1–S5.14, mais a avaliação A/B como entregável de medição.
- Sete seams: quatro novos (`tests/online.rs`, `online_units.rs`, `online_scheduler.rs`,
  `online_protocol.rs`), a CLI e a superfície MCP existentes, e um teste live ignorado.
- A lógica do adaptador roda no build padrão com `FakeClassifier`; só o `JevClient` exige
  `--features online`.
- Linha de base: 127 verdes, 1 ignorado. A única edição de teste existente é a do CA-10
  (D-059).
- Cinco pontos de parada; o 2 é a barra de merge da Fase 4 (§23.15).

## D-059 — Feature `online` e CA-10 (proposta)

- O cliente HTTP (`reqwest` com rustls, `secrecy`) só compila com a feature Cargo `online`,
  desligada por padrão. A ativação continua pela flag `--online` no startup (v0.1 §3.1).
- O `env` do servidor MCP carrega só `RIPWIRE_BROKER_JEV_API_KEY`. Não há variável que
  ative o modo online, o que mantém D-022 (configuração por argumentos, `env` só para
  segredos). Isso resolve a lacuna 11 do §23.17.
- Um binário sem a feature recusa `--online` com erro de uso.
- Achado: `the_build_has_no_network_stack` procura crates de rede no `Cargo.lock`, que
  lista também dependências opcionais. Com a feature declarada, ele falharia mesmo no build
  padrão. Proposta: verificar o grafo resolvido do build padrão (`cargo tree -e normal`),
  que é o que o CA-10 afirma. É a única edição de teste existente do plano.

## D-060 — Gate por rota (proposta)

- O classificador roda só nas rotas que terminam em `explore`: `orient` (inclusive o caso
  incerto), `change` sem símbolo e o fallback `symbol_not_found`.
- Ficam de fora `debug`, `symbol`/`change` com símbolo encontrado, `review` e `docs`: os
  casos de ganho pequeno da v0.1 §5.3.
- A v0.1 §3.3 exige a etapa semântica em toda chamada. A proposta a contraria de forma
  explícita: nas rotas puladas, `provenance.online.discovery = "skipped"` e a limitação
  `semantic_skipped`. O broker nunca declara uso online sem avaliação, que é o objetivo
  daquela regra.

## D-061 — Candidatos e unidades (proposta)

- `RankedPath { path, rank, priority, origin, lines }`, construído por função pura depois
  da rota e antes de `complete_task`, portanto antes do budgeter (lacuna 9).
- Rescore (Fase 4, lacuna 7): os paths distintos dos itens estruturais, exceto docs, na
  ordem do Ripwire, até 16 (`--jev-max-candidates`). `file_admission` sobre preview de
  16 KiB; `source_selection` sobre as unidades dos admitidos. O rescore só promove; nada
  estrutural é rebaixado nem removido.
- Lookahead de um nível (Fase 5, lacuna 8): arquivos elegíveis diretamente nos diretórios
  dos paths admitidos, sem descer, até 32 no total (`--jev-lookahead-max`).
- Unidades (RF-ONLINE-07): chunks de ~3 KiB alinhados a linhas, ligados ao símbolo do
  Ripwire cuja linha contêm. O spike S4.0d decide se `analyze`/`for` entram como fonte de
  ranges, como verbo opcional fora de `REQUIRED_VERBS`.

## D-062 — `prompts/v1` e tetos (proposta)

- Guidance literal da v0.1 §16.4. Rascunho das duas perguntas em inglês no §3.2 do plano,
  com a regra "coincidência temática não basta". O texto só é congelado por golden depois
  do registro live (S4.0b), que confirma como uma pergunta referencia um item.
- Tetos da fusão aceitos como padrões configuráveis: 4 requests em voo e 24 por chamada
  (lacuna 13).

## D-063 — Envelope online e `interrupted` (proposta)

- `provenance.online = {enabled, provider, model, requests, cache_hits, incomplete,
  discovery}`, com `discovery ∈ complete | incomplete | interrupted | skipped`. O booleano
  `incomplete` da v0.1 continua (lacuna 14).
- Item só-semântico: `source = {verb: "jev", basis: "remote_classifier"}`. Item estrutural
  confirmado mantém `source` e ganha `semantic` (estágio, probabilidade, threshold, modelo,
  digest, hash da fonte, cache hit). Equivale à proveniência dupla do §23.7 sem mudar a
  forma v1 de `source`.
- Cancelamento do cliente MCP mantém o RF-14 (`cancelled`), agora propagado até as
  requests HTTP. `interrupted` fica para o prazo de descoberta (`--jev-deadline-ms`, 8.000
  por padrão, sem fonte): o estrutural sai com a evidência fresca já validada (lacuna 15).

## D-064 — Cache, diagnóstico e integração (proposta)

- Cache em memória com chave por pergunta (provider, endpoint, modelo, versões de prompt e
  política, estágio, query, hash da fonte, range). A chave por request da v0.1 quase nunca
  acertaria, porque os lotes mudam a cada chamada. O valor guarda só a probabilidade
  validada e o timestamp.
- `doctor --jev-probe`: uma request com texto sintético embutido no binário, nenhum byte do
  workspace. `doctor` sem a opção nunca usa a rede.
- `install --online`: acrescenta `--online` aos args e um bloco `env` que referencia a
  variável, nunca o valor da credencial.
- `hook` e `prompt` continuam offline nas duas fases e recusam `--online`: processo curto,
  timeout do host e consentimento por processo não combinam com envio a cada prompt.

## D-065 — Aprovação das propostas das Fases 4 e 5

- O usuário aprovou D-059 a D-064 sem alterações e pediu o início do Sprint 0.
- D-058 continua sendo o plano de referência. As propostas deixam de ser propostas.

## D-066 — Sprint 0 parcial

- **S4.0a:** `online::ranked_paths` foi implementado como função pura sobre `(prioridade, &Item)`,
  porque `Entry` é privado e o seam 2 só usa API pública. A ordem é: melhor prioridade, depois
  a primeira aparição na saída do Ripwire; docs ficam de fora e as linhas são ordenadas e
  distintas.
  - Desvio do plano: a chamada dentro de `context_for_task_inner` não entrou. Sem um motor
    online ela seria código morto. Entra em S4.22, junto com `BrokerConfig.online`, e o teste
    de guarda do envelope offline vai junto.
- **S4.0c:** `online::prompt` (versão `v1`, guidance literal e as duas perguntas do rascunho de
  D-062) e `online::request::build`, puro. O golden foi escrito à mão em
  `tests/online_units.rs`. Ele é **provisório** até o S4.0b confirmar a forma real de
  `state.items` e como uma pergunta referencia um item.
  - Desvio do plano: sem `indexmap`. Um `Serialize` de mapa sobre `Vec` mantém `q0..qn` na
    ordem de inserção, e um teste cobre mais de dez perguntas. Assim o build não ganha
    dependência nova.
- **S4.0d (spike, Ripwire 0.6.4):**
  - `find_symbol` dá só a linha inicial.
  - `analyze` exige um diretório e lista nomes sem linhas.
  - `fetch_body` dá `line` e `total_lines`, isto é, o intervalo completo, mas custa uma chamada
    por símbolo.
  - Decisão (dentro de D-061): na Fase 4, as unidades são chunks ligados à linha do item. Só o
    corpo que o broker já busca (o símbolo central) usa o intervalo de `fetch_body`. Nenhum
    verbo entra na allowlist.
- **S4.0b:** preparado, não executado. `examples/jev_record.rs` imprime os dois requests de um
  corpus sintético, inventado para isso e sem ler o workspace, para que o `curl` os envie. Assim
  o Sprint 0 não põe crate de rede no build. Falta a credencial
  `RIPWIRE_BROKER_JEV_API_KEY`, que não está no ambiente.
- Suíte: 131 verdes, 1 ignorado; clippy e fmt limpos.

## D-067 — Gravação live e ponto de parada 0

- **S4.0b.** Com a credencial em `~/.config/ripwire-broker/jev.key` (`0600`) e a autorização
  do usuário, os dois requests de `examples/jev_record.rs` foram enviados por `curl` a
  `https://api.typesafe.ai/v1/systemone`, com HTTPS obrigatório e sem redirect. A chave foi
  lida do arquivo no próprio comando e nunca impressa.
- **Contrato confirmado** (v0.1 §10):
  - `200`, `Content-Type: application/json`, HTTP/2;
  - a resposta é `{model, answers: {qN: {type: "noul", noul: p}}, usage: {input_tokens,
    output_tokens}}`, com uma resposta por pergunta e os mesmos ids;
  - latência de 300–350 ms para 2–3 perguntas;
  - o header `x-typesafe-request-id` existe, mas não é gravado.
- **`prompts/v1` validado.** Uma pergunta referencia seu item por id (`item i0`, `code block
  i1`) dentro de `state.items = [{id, path, text}]`, e o modelo discrimina:
  - admissão: `auth.py` 0,81, o teste 0,83 e um CSV sem relação 0,03;
  - seleção: `validate_token` 0,81 e `login` 0,64.
  O rascunho de D-062 fica como texto definitivo da `v1`, e o golden deixa de ser provisório.
- **Fixture** `tests/fixtures/jev/live_v1.json`: só digest sha256 e tamanho de cada request,
  ids das perguntas, status, content type, latência e a resposta (probabilidades e uso). Não
  tem fonte, query, credencial nem id do provider.
- O corpus sintético saiu do exemplo para `tests/common/jev_corpus.rs`, compartilhado com o
  teste `the_live_recording_still_matches_prompts_v1`. Esse teste recalcula os digests; se
  `prompts/v1` mudar, ele pede uma nova gravação.
- Não registrado ainda: as formas de erro do provider (`401`, `429`, `5xx`). O S4.11 usa o
  servidor fixture com as formas da v0.1; o teste live S5.14 confere.
- **Ponto de parada 0:** Sprint 0 concluído (S4.0a–S4.0d). D-061 e D-062 confirmados pela
  evidência. Suíte: 132 verdes, 1 ignorado; clippy e fmt limpos.

## D-068 — Configuração, credencial e garantia offline

Fatias S4.1 a S4.6 do plano, no branch `fase-4-online`.

- **S4.1.** `cli::OnlineArgs` chega em `ServeArgs.online`. Os padrões são os de D-059 a D-063:
  `typesafe`, `jev-1.13.0`, 4 em voo, 24 requests, 15.000 ms por tentativa, 16 candidatos e
  prazo de 8.000 ms.
  - Viram erro de uso: uma opção `--jev-*` sem `--online`, provider diferente de `typesafe`,
    zero em teto, e qualquer tentativa de passar a chave pela linha de comando. Nenhum erro
    ecoa valores.
  - `hook`, `prompt` e `doctor` recusam `--online` com mensagem própria (D-064).
- **S4.2.** Um binário sem a feature recusa `--online` com código 2 antes de consultar o
  ripwire e de publicar o MCP, e explica como recompilar. Até aqui, o servidor subia offline em
  silêncio, o que violava a invariante 3 do §23.1.
- **S4.3 e S4.4.** A feature `online` traz `secrecy` como dependência opcional.
  - `online::credential::Credential` só lê `RIPWIRE_BROKER_JEV_API_KEY`. Tira o whitespace
    externo, recusa whitespace interno e conta vazio como ausente.
  - O `Debug` é redigido; não há `Serialize` nem `Display`.
  - Sem credencial válida, o `serve --online` falha com código 2 antes de publicar o MCP
    (CA-ONLINE-02).
  - Transitório: com credencial válida, o build `online` ainda **recusa** `--online`, porque o
    adaptador só é ligado em S4.20–S4.31. Assim nenhum commit intermediário roda offline
    fingindo estar online.
- **S4.5.** `the_build_has_no_network_stack` agora lê o grafo resolvido do build padrão
  (`cargo tree -e normal --offline`) em vez do `Cargo.lock`, como D-059 previa. A lista
  proibida ganhou `h2` e `secrecy`.
  - Desvio do plano: a outra metade do S4.5, `without_online_the_classifier_is_never_called`,
    depende da `trait Classifier` e do `FakeClassifier`. Ela vai para S4.21, junto com o
    teste-guarda das outras duas tools.
- **S4.6.** O texto de consentimento do §23.6 foi para o `USAGE` já no S4.1. O teste nasceu
  verde e guarda o texto literal.
- **CI.** O workflow do GitHub Actions também compila e testa com `--features online`.
- Suítes: 137 verdes no build padrão e 138 com `online`, 1 ignorado em cada; clippy e fmt
  limpos nas duas.

## D-069 — Protocolo, validação e cliente HTTP

Fatias S4.7 a S4.11 do plano.

- **S4.7.** Já estava coberto pelos testes do S4.0c: ordem `q0..qn` além de dez perguntas, tipo
  `noul`, guidance literal e golden. Nenhum teste novo. A parte do wire veio em S4.11.
- **S4.8.** `online::response::parse_answers` devolve uma probabilidade por pergunta, na ordem
  do request e casada por id, não por posição (CA-ONLINE-10).
  - Viram desconhecido (`None`) uma pergunta sem resposta, tipo diferente de `noul`, valor
    ausente, que não seja número, negativo ou maior que 1. Os valores 0 e 1 são válidos.
  - Rejeitam a resposta inteira: JSON inválido (inclusive `NaN`, que não é JSON), falta de
    `model` ou `answers`, modelo diferente do pinado e resposta a pergunta não feita. Neste
    último caso a correspondência por id está quebrada.
- **S4.9.** `online::decision` aplica thresholds estritos: admissão `> 0,25`, seleção `> 0,50`,
  lead em `(0,25; 0,50]` (CA-ONLINE-06). Um arquivo fica com o maior score entre os fragmentos.
  - Interpretação: se algum fragmento ficou sem avaliação e o melhor conhecido não passa, o
    arquivo é `Unknown`, não `Rejected`, porque o fragmento faltante pode conter a evidência.
- **S4.10.** `online::request::batches` mantém a ordem e fecha o lote antes de 128 perguntas
  ou 38.000 bytes. Lotes de evidência têm até 8 unidades e ~14 KiB; uma unidade sozinha pode
  passar dos 14 KiB, desde que caiba em 38.000. O que não cabe nem sozinho volta à parte, para
  a limitação `request_too_large`.
  - O tamanho é calculado de forma incremental e exata (`request_bytes`), e um teste o confere
    contra o JSON real. A primeira versão reserializava o lote a cada item e era quadrática, o
    que ameaçava a meta de p95 < 75 ms.
- **S4.11.** `online::classifier::Classifier` é a trait interna do provider e compila no build
  padrão. `ClassifyError` só carrega categoria e status HTTP.
  - Mapeamento: `401`/`403` → `auth`; `408` e timeout → `timeout`; `429` → `rate_limited`,
    com `Retry-After` bruto limitado a 64 caracteres; `5xx` → `server`; demais status e
    redirects → `rejected`; body malformado ou `Content-Type` diferente de JSON →
    `invalid_response`; body acima de 256 KiB → `response_too_large`; falha de conexão →
    `network`.
  - `online::jev::JevClient`, só com a feature: `reqwest` 0.12 com `rustls-tls` (raízes webpki,
    ring, sem compilar C) e `http2`. Um cliente por processo, HTTPS obrigatório, sem redirect,
    sem proxy, timeout por tentativa, header de autorização marcado como sensível e nenhuma
    retentativa própria.
  - `JevClient::loopback` (`#[doc(hidden)]`) aceita só `http://127.0.0.1` e existe para os
    testes; nenhuma opção da CLI chega a ele.
  - O servidor fixture do seam 4 é um HTTP/1.1 mínimo sobre `TcpListener`, sem crate de
    servidor. Os 6 testes cobrem body exato com bearer em `/v1/systemone`, os status da v0.1
    §22, redirect recusado sem contatar o destino, ausência de retry, timeout, conexão fechada
    e endpoint allowlisted. Nenhum erro mostra a credencial nem o texto remoto.
- **D-059 confirmado:** com o `reqwest` no `Cargo.lock`, o teste antigo de CA-10 falharia. O
  teste sobre o grafo resolvido continua verde no build padrão.
- Suítes: 146 verdes no build padrão e 153 com `online`, 1 ignorado; clippy e fmt limpos nas
  duas.

## D-070 — Leitura do workspace e ponto de parada 1

Fatias S4.12 a S4.14 do plano.

- **S4.12.** `online::reader::WorkspaceReader::snapshot` é o único caminho entre o workspace e o
  classificador. Checa nesta ordem e devolve o motivo sem path nem conteúdo:
  1. absoluto ou com `..` → `outside`;
  2. nome sensível → `sensitive_name`. Vem antes de "oculto" para o `.env` ser classificado
     como sensível. A lista cobre `.env*`, `.netrc`, `.npmrc`, `.pypirc`, `.pgpass`, chaves SSH,
     `credentials*`, `secrets.*`, além de `*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.jks`,
     `*.keystore`, `*.kdbx` e `*.gpg`;
  3. componente oculto (inclui `.git`) → `hidden`;
  4. diretório de dependência ou build (`node_modules`, `target`, `vendor`, `dist`, `build`,
     `__pycache__`, `bower_components`, `venv`) → `dependency_or_build`;
  5. qualquer symlink abaixo da raiz → `symlink`, nunca seguido;
  6. diretório, FIFO, socket ou device → `not_regular`, detectado pelos metadados sem abrir o
     arquivo, o que evita travar num FIFO;
  7. ignorado por `.gitignore`, `.ignore` ou os excludes do git → `ignored`;
  8. NUL nos primeiros 8.000 bytes → `binary`; UTF-8 inválido → `not_utf8`; marcador
     `-----BEGIN … PRIVATE KEY-----` → `private_key`.
  - O ignore usa a crate `ignore` (mesma semântica do git, sem rede). É conferido componente a
    componente, porque um diretório ignorado precisa esconder também o que está dentro dele.
    Como no git, os excludes globais do usuário também valem; isso só exclui mais.
  - Acréscimo ao plano: arquivos acima de 8 MiB nem são lidos (`too_large`). Antes disso,
    hashear um arquivo gigante seria o único trabalho sem limite do leitor.
- **S4.13.** `Snapshot { path relativo, content_hash "sha256:…", texto }`. O preview tem no
  máximo 16 KiB e termina numa quebra de linha, ou numa fronteira de caractere se não houver
  quebra. `is_fresh` relê o arquivo pela mesma política e compara o hash; a revalidação da
  Fase 5 (S5.1) vai usá-lo.
- **S4.14.** `units(snapshot, linhas_do_ripwire)`:
  - uma unidade nova começa em cada linha de símbolo do Ripwire e fica ligada a ele
    (`symbol_line`);
  - fora disso, a unidade fecha ao chegar a ~3 KiB de linhas inteiras;
  - nenhuma passa de 24 KiB, e uma linha maior que isso é cortada em pedaços;
  - arquivo acima de 1 MiB não tem unidades (só localização), mas mantém o preview para a
    admissão;
  - as linhas são one-based e inclusivas por fora, e os bytes half-open por dentro (§23.2).
- **Ponto de parada 1:** o build padrão tem 152 verdes e o `online` 159, 1 ignorado em cada.
  Clippy e fmt limpos nas duas configurações.
  - A crate `ignore` trouxe só dependências locais (`globset`, `walkdir`, `crossbeam`,
    `regex-automata`, `bstr`), e a guarda de CA-10 continua verde.

## D-071 — Scheduler mínimo

Fatias S4.15 a S4.19 do plano, sem retry nem cooldown (Fase 5).

- `online::scheduler::Scheduler::run` recebe jobs de uma fila `mpsc` limitada
  (`SchedulerConfig.queue`), cada um com o id do produtor. Cada request vira uma task num
  `JoinSet` que pertence ao scheduler. O `Report` traz os resultados na ordem de término, os
  jobs admitidos e não respondidos (`unfinished`), os requests enviados e o motivo da parada.
- **Teto em voo.** No máximo `max_in_flight` tasks vivas. A contagem é feita pelas tasks do
  próprio `JoinSet`, não por um `Semaphore`: o limite é o mesmo e fica determinístico. Na
  Fase 5, a espera de cooldown acontece no laço do scheduler, fora das tasks, por isso nenhuma
  vaga fica ocupada só esperando (v0.1 §11.8).
- **Backpressure.** Com o provider lento, o produtor fica parado no `send` da fila cheia, e só
  L requests começam (CA-ONLINE-05).
- **Associação por id.** Respostas que terminam fora de ordem continuam ligadas ao job certo.
- **Auth.** Um `401`/`403` aborta as irmãs em voo (o drop da future cancela o request HTTP),
  nenhum request novo começa, e a fila é fechada, o que faz o produtor parar.
- **Limite de requests.** No 25º job, com limite 24, a admissão para, a fila é fechada, e o
  que estava na fila entra em `unfinished`. `Report::incomplete()` fica verdadeiro.
- **Cancelamento.** Um `CancellationToken` aborta as tasks em voo e fecha a fila. O
  cancelamento vindo do MCP e o aborto do request HTTP real são o S5.4.
- **Achado:** a primeira versão travou. O ramo de cancelamento do `select!` nunca fica
  inativo, então o `else => break` nunca disparava quando o trabalho acabava. A saída agora é
  explícita no topo do laço (sem admissão e sem tasks).
- **Correção de teste:** o teste de limite usava 30 jobs, e com uma fila de 8 todos cabiam
  antes do fechamento. Passou a usar 100 e a conferir que todo job admitido foi respondido ou
  reportado.
- Dublê `tests/common/classifier.rs`: responde por id de item, pode falhar, atrasar ou segurar
  requests, e conta chamadas, concorrência e requests abortados. Uma falha roteirizada
  responde sem esperar o gate.
- Dependências: `tokio-util` 0.7 no build padrão (`CancellationToken`, sem rede) e a feature
  `test-util` do tokio só para testes (relógio pausado).
- Os testes do scheduler rodaram 30 vezes seguidas sem falha. Suítes: 158 verdes no build
  padrão e 165 com `online`, 1 ignorado; clippy e fmt limpos nas duas.

## D-072 — Composição em `context_for_task` e ponto de parada 2

Fatias S4.20 a S4.31 do plano.

- **Fiação.** `BrokerConfig.online: Option<OnlineConfig>` (`None` em `BrokerConfig::new`)
  vira um `OnlineEngine` no `connect`. O passo `semantic_step` fica entre a rota e
  `finish_task`, portanto antes do budgeter, que conta os bytes de `provenance.online`.
  - O `main` monta o `JevClient` real com a credencial do ambiente, e a recusa transitória de
    D-068 saiu.
  - O status degradado (sem Ripwire) agora publica o modo online e `offline: false`. Antes
    dizia `offline: true` mesmo com `--online`.
- **S4.20/S4.21, gate por rota (D-060).** O classificador roda se a rota chamou `explore`
  (`orient`, `change` sem símbolo e fallback de símbolo inexistente). Nas outras rotas, e
  sempre em `context_after_edit` e `context_before_finish`, nenhuma chamada. Rota pulada
  traz `discovery: "skipped"` e a limitação `semantic_skipped`.
- **S4.22, coordenador.** Até 16 paths do planner passam pelo `WorkspaceReader`; os
  inelegíveis viram a limitação `semantic_not_sent` (contagem por motivo, sem path) e deixam a
  descoberta `incomplete`. A admissão usa o preview; a seleção usa as unidades dos admitidos.
  As duas etapas dividem o limite de 24 requests por chamada.
- **S4.23–S4.25, merge.** Decisões desta fatia:
  - item estrutural: mantém `source` (Ripwire) e ganha `semantic`. É a evidência do bloco que
    começa na linha do símbolo, se houver; senão, a da admissão do arquivo. O estado vale
    `admitted`, `rejected`, `selected_source`, `reading_lead` ou `excluded`. A rejeição
    aparece, não some (risco "merge esconder discordância", §23.16);
  - bloco selecionado de um símbolo do Ripwire: o item sobe para a faixa `CENTRAL`. Nada é
    rebaixado nem removido;
  - evidência sem símbolo correspondente: item `semantic_location`, com papel novo
    `Role::Semantic` (`"semantic"`, só aparece com `--online`) e `source = {verb: "jev",
    basis: "remote_classifier"}`. O estado fica em `semantic.state`; conteúdo só em
    `selected_source`; nunca vem com caller, teste ou risco;
  - `semantic.lines` traz o intervalo avaliado (one-based, inclusivo), e
    `semantic.request_digest` o sha256 do request;
  - `SemanticEvidence` fica num `Box` no `Item`, por causa do aviso de tamanho de variante do
    clippy. A serialização não muda.
- **S4.26, falha parcial.** Falhas viram `semantic_incomplete` e `discovery: "incomplete"`, e
  a resposta estrutural sai igual à offline (CA-ONLINE-15).
- **S4.27, orçamento.** Achado: o esqueleto do envelope, que nunca é cortado, já ocupa ~224
  dos 256 tokens mínimos; com `provenance.online` e as limitações online, 256 é impossível de
  garantir.
  - Decisão: num processo `--online`, `context_for_task` exige **512** tokens
    (`MIN_ONLINE_BUDGET_TOKENS`), e o schema publicado mostra esse mínimo. As outras duas
    tools e o modo offline continuam com 256.
  - Um teste confere o pior caso (todas as limitações online) em 512, e a varredura de 512 a
    4.000 nunca passa do orçamento (CA-ONLINE-14).
  - Desvio do plano: `the_package_leads_with_status_summary_and_limitations` foi retirado.
    Reordenar os campos do envelope mudaria a saída offline byte a byte (princípio 2 do
    plano). O `summary` já abre o envelope.
- **S4.28, envelope.** Sem `--online`, nenhum campo novo aparece: nem `online`, nem
  `semantic`, nem `semantic_location`, nem as limitações novas.
- **S4.29, cache.** O valor guarda também o digest do request que trouxe a resposta
  (metadado não sensível, §23.8). A chave muda com modelo, query, versão da fonte, range e
  etapa. A inspeção não encontra query, path, fonte nem credencial (CA-ONLINE-13).
- **S4.30, status.** Bloco `online` com provider, modelo, host do endpoint, tetos, requests,
  cache hits, decisões em cache e a categoria do último erro, sem conteúdo.
- **S4.31.** O broker de ponta a ponta com o `JevClient` real contra o fixture local: bearer
  em todo request, e o root absoluto nunca aparece no body. Um e2e do binário com
  `--online` confere o status e o piso de 512 no schema.
- **Medição do ponto de parada 2.** Overhead local de batching e merge, sem Ripwire e sem
  rede, em release: 16 arquivos de ~16 KiB e 24 requests por chamada, p50 7,9 ms e **p95
  8,3 ms** (meta < 75 ms). Fica no teste ignorado `overhead_of_batching_and_merge`.
- **Barra de merge da Fase 4 (§23.15): verde.** CA-10, CA-ONLINE-01 a 08, 10, 13, 14 e 15,
  com `L = 4`.
- **Pendências registradas:**
  - `--jev-max-source-bytes` e `--jev-deadline-ms` são aceitos, mas ainda não têm efeito. O
    prazo é o S5.5; o limite de fonte renderizada entra junto.
  - O teste manual num host real com `--online` depende de o usuário autorizar o envio de
    código do repositório.
  - Uma execução da suíte padrão teve uma falha que não se repetiu em outras 21 execuções,
    8 delas com as duas suítes em paralelo. O teste não foi identificado; se voltar, vira
    investigação.
- Suítes: 182 verdes no build padrão e 191 com `online`, 2 ignorados; clippy e fmt limpos nas
  duas.

## D-073 — Frescor das fontes

Fatia S5.1 do plano (RF-ONLINE-10, CA-ONLINE-11).

- **Decisão.** O §23.10 permite "replanejar uma vez **ou** marcar incompleto". Fica a
  segunda opção: sem replanejamento, a evidência afetada é descartada e a descoberta
  marcada `incomplete`. Replanejar exigiria reler, recortar e reenviar dentro do mesmo
  orçamento de requests, e o CA-ONLINE-11 só pede o descarte. Se a medição mostrar
  descartes frequentes, a opção de replanejar volta a ser avaliada.
- **Antes de cada tentativa.** `scheduler::Job` ganhou `fresh: Option<Freshness>`, uma
  checagem avaliada imediatamente antes de disparar o request. O coordenador monta a de cada
  lote com os snapshots de todos os seus itens: `WorkspaceReader::is_fresh` relê o arquivo
  pela política de elegibilidade e compara o hash. Um lote com fonte alterada não é enviado,
  não conta como request e vai para `Report::stale`. O S5.2 vai reavaliar a mesma checagem
  antes de cada retry.
- **Antes da saída.** Ao fim da descoberta, cada arquivo candidato é revalidado. A evidência
  de um arquivo alterado sai inteira de `Discovery::files`, e os fatos estruturais desse
  arquivo ficam sem anotação `semantic`.
- A limitação `semantic_incomplete` diz quantos lotes não foram enviados e quantos arquivos
  mudaram, sem paths.
- **Testes.**
  - O dublê ganhou `on_call`, que edita o arquivo enquanto o provider responde. Um teste muda
    o arquivo durante a admissão, e o lote de seleção nunca sai; outro muda durante a
    seleção, e a resposta chega mas é descartada. Um teste do scheduler cobre o job obsoleto.
  - A primeira rodada falhou por um erro no próprio dublê: a chamada do gancho não tinha
    entrado no `classify`, porque o script de edição procurava uma linha já reformatada pelo
    fmt.
  - Um teste de mutação (as duas revalidações desligadas) derrubou exatamente os dois testes
    novos.
- **Custo.** O overhead local de batching e merge subiu de p95 8,3 ms para **21,9 ms**
  (meta < 75 ms), porque cada lote relê seus arquivos e refaz a checagem de ignore por
  diretório. A leitura é síncrona dentro da task; se o custo crescer, ela vai para
  `spawn_blocking` e o resultado de ignore por diretório passa a ser guardado na chamada.
- Suítes: 185 verdes no build padrão e 194 com `online`, 2 ignorados; clippy e fmt limpos nas
  duas.

## D-074 — Retry e divisão de lotes

Fatia S5.2 do plano (v0.1 §11.9, §23.5).

- **Política, no scheduler.** O cliente continua sem retry próprio.

  | Lote | Falha transitória (timeout, 5xx, rede) | Não transitória |
  | --- | --- | --- |
  | seleção de fonte | até 2 tentativas; depois, se tiver mais de um item, divide ao meio | nem retry nem divisão |
  | admissão com vários itens | 1 tentativa; depois divide ao meio | idem |
  | lote com um item | até 2 tentativas | idem |

  - "Não transitória" inclui `409`, resposta inválida e resposta grande demais. `401`/`403`
    continuam parando tudo (D-071).
  - O `429` fica fora do retry até o S5.3, que traz o cooldown compartilhado. Repetir sem
    esperar o `Retry-After` só pioraria o rate limit.
- **Mecânica.**
  - `Job` ganhou `stage`, que decide a política e reconstrói as metades.
  - Tentativas e metades vão para uma fila interna com prioridade sobre a fila do produtor.
  - Cada tentativa conta no limite de requests e repete a checagem de frescor do S5.1; um
    retry sobre fonte alterada nunca é enviado.
  - As metades herdam o id e a checagem de frescor do lote original.
  - `JobResult` agora traz o request que de fato respondeu: uma metade tem seus próprios itens
    e ids de pergunta. O coordenador casa as respostas por ele, não mais por `requests[id]`.
  - `Report` ganhou `retries` e `splits`, que o S5.10 vai expor como `jev_retry_total` e
    `jev_split_total`.
- **Limite da busca.** Nenhuma busca é reiniciada inteira. Com falha persistente, um lote de
  admissão de 3 itens gera exatamente 8 requests: o lote (1); `[j0]` duas vezes (2);
  `[j1, j2]` uma vez (1); `[j1]` e `[j2]` duas vezes cada (4). No fim, cada item fica com uma
  falha final e resposta desconhecida.
- **Testes.** Seis testes novos no seam 3: a política por etapa, erros não transitórios, o
  limite com falha persistente, retries contando no limite de requests, retry sobre fonte
  alterada e ausência de deadlock (1 em voo, fila 1, 8 lotes de 8 que falham uma vez e se
  dividem). Os 13 testes do scheduler rodaram 30 vezes seguidas sem falha.
- Suítes: 191 verdes no build padrão e 200 com `online`, 2 ignorados; clippy e fmt limpos nas
  duas.

## D-075 — `429` e cooldown compartilhado

Fatia S5.3 do plano (v0.1 §11.8, CA-ONLINE-09).

- **`online::retry_after::parse`** é puro e não usa crate de data. Lê segundos inteiros ou a
  data HTTP no formato IMF-fixdate (`Sun, 06 Nov 1994 08:49:37 GMT`); uma data passada vale
  espera zero. Outros formatos, dia ou mês inválidos devolvem `None`. O cálculo de dias é o
  `days_from_civil` de H. Hinnant, e os testes usam o exemplo da RFC 9110 e 29/02/2024.
- **Cooldown.** Um prazo único no laço do scheduler, compartilhado por todos os lotes, que
  guarda o maior valor observado. Enquanto ele não vence, nada sai: nem retry, nem metade de
  lote, nem job novo. Como a espera acontece no laço e não nas tasks, ela não ocupa vaga de
  envio; o teste confere zero requests em voo durante a espera. O cancelamento interrompe a
  espera na hora.
- **Política.**
  - O lote que recebeu `429` ganha uma nova tentativa depois do cooldown, sem ser dividido,
    porque a culpa não é do lote.
  - Um segundo `429` do mesmo lote é final.
  - Sem `Retry-After` legível, a espera é de 1 s (`DEFAULT_COOLDOWN`). Acima de 30 s
    (`MAX_COOLDOWN`), o scheduler não espera: o lote fica com falha `rate_limited` e a
    descoberta, incompleta. Isso impede que um provider prenda a resposta ao agente; o prazo
    total entra no S5.5.
  - `Report.rate_limited` conta os `429` para a métrica `jev_rate_limit_total` do S5.10.
- **Testes.** Quatro novos no seam 3 e um no seam 2. Uma mutação que desliga o cooldown
  derruba exatamente os três testes de espera. Os 17 testes do scheduler rodaram 30 vezes
  seguidas sem falha.
- Suítes: 196 verdes no build padrão e 205 com `online`, 2 ignorados; clippy e fmt limpos nas
  duas.

## D-076 — Teste manual com `--online`

Pendência do ponto de parada 2 (D-072), autorizada pelo usuário: envio de trechos elegíveis
deste repositório ao TypeSafe.

- **Montagem.** Binário release com `--features online`, servidor MCP sobre este
  repositório com o Ripwire 0.6.4 real e a chave lida de `~/.config/ripwire-broker/jev.key`
  só para o ambiente do processo. Um cliente JSON-RPC cru no scratchpad, no formato do cliente
  dos testes. Os envelopes ficaram só no scratchpad.
- **Resultados.**

  | Chamada | Tempo | Requests | Cache hits | Descoberta | Tokens |
  | --- | --- | --- | --- | --- | --- |
  | orientação ("como o broker mantém a resposta no orçamento?") | 2,38 s | 7 | 0 | complete | 2.386/2.500 |
  | a mesma, repetida | 0,02 s | 0 | 28 | complete | 2.385/2.500 |
  | mudança ("pular a injeção do hook com prompt vazio") | 0,64 s | 4 | 0 | complete | 2.499/2.500 |
  | símbolo (`` `estimate_tokens` ``) | 0,00 s | 0 | 0 | skipped | 597/2.500 |

  - `context_after_edit` não teve `provenance.online`.
  - O status mostrou `offline: false`, 11 requests, 47 decisões em cache e nenhum erro.
  - A chave não apareceu em nenhuma saída gravada.
- **Qualidade observada.**
  - **Mudança:** o classificador selecionou exatamente `hook.rs::handle` (p = 0,74), a função
    que decide a injeção. Rejeitou ou excluiu os outros 10 itens do Ripwire, e todos
    continuaram no envelope, com a discordância visível.
  - **Orientação:** o `explore` não trouxe `src/budget.rs` entre os candidatos. Como o rescore
    só avalia o que o Ripwire já achou, o classificador selecionou trechos de `hook.rs` e
    `normalize.rs` (0,65 a 0,85), e os blocos do Ripwire ficaram como reading leads (0,44 a
    0,50). É a limitação prevista da Fase 4. O lookahead de um nível (S5.7) teria incluído
    `src/budget.rs`, vizinho em `src/`. Isso reforça que o ganho de recall depende do S5.7 e
    precisa ser medido no A/B.
  - Os trechos selecionados sem símbolo (`semantic_location`, prioridade `BODY`) ocuparam 4 dos
    8 itens mostrados em 2.500 tokens, como manda a ordem do §23.4 (fonte selecionada antes de
    caller estrutural). O A/B precisa conferir se essa ordem ajuda o agente.
- Com isso o ponto de parada 2 fica completo.

## D-077 — Cancelamento até o HTTP

Fatia S5.4 do plano (RF-ONLINE-14, CA-ONLINE-12).

- **Achado.** O caminho já existia por estrutura. No RF-14 (D-049), o `mcp.rs` descarta a
  future da tool quando chega `notifications/cancelled`. O drop então desce a cadeia:
  `context_for_task` → `OnlineEngine::discover` → `Scheduler::run` → `JoinSet`, cujo drop
  aborta todas as tasks → future do `reqwest`, cujo drop fecha a conexão. Filas, retries
  pendentes, esperas de cooldown e requests em voo terminam juntos, e a tool responde
  `cancelled`, como hoje.
- **Decisão.** Manter o cancelamento pelo drop estruturado, sem ligar o `CancellationToken` do
  scheduler ao `Notify` do RF-14, como o plano previa. Um token duplicaria o que o drop já
  garante, e o cancelamento MCP continua respondendo `cancelled` (D-063). O token do scheduler
  fica para o prazo de descoberta (S5.5), que precisa devolver o envelope com `interrupted`.
- **Regra de código.** Nada no caminho do request pode rodar em task destacada
  (`tokio::spawn` sem dono): ela sobreviveria ao drop e manteria a conexão aberta.
- **Testes.**
  - Seam 3: descartar a future do `run` derruba os 4 requests em voo, nada começa depois, e o
    produtor para.
  - Seam 4: o broker com o `JevClient` real contra um provider local que nunca responde.
    Abortar a chamada fecha todas as conexões em até 250 ms (v0.1 §20.1), e nenhum request
    novo sai depois.
  - Os dois nasceram verdes, como a análise previa. Uma mutação plausível (enviar o request
    numa task destacada) derruba o teste do seam 4, o que prova que ele guarda a regra acima.
    O teste do seam 4 rodou 10 vezes seguidas sem falha.
- Suítes: 197 verdes no build padrão e 207 com `online`, 2 ignorados; clippy e fmt limpos nas
  duas.

## D-078 — Prazo de descoberta e limite de fonte

Fatia S5.5 do plano; fecha as duas pendências de flags de D-072.

- **Prazo (`--jev-deadline-ms`, padrão 8.000).** `OnlineConfig.deadline` vira um instante
  absoluto no início da descoberta, que vale para as duas etapas.
  - Ao vencer, o `CancellationToken` do scheduler é cancelado. O `run` não é descartado: ele
    aborta os requests em voo e devolve o relatório parcial, então as respostas que já
    voltaram continuam valendo (v0.1 §11.10).
  - Se o prazo já venceu antes de uma etapa começar, ela nem envia e os lotes contam como não
    respondidos.
  - O envelope sai com o estrutural intacto, a evidência validada (inclusive a revalidação de
    frescor do S5.1), `discovery: "interrupted"` e `incomplete: true`. A limitação
    `semantic_incomplete` diz "discovery deadline of N ms reached".
  - `interrupted` tem precedência sobre `incomplete` no campo `discovery`. O cancelamento pelo
    cliente MCP continua respondendo `cancelled` (D-063, D-077).
- **Limite de fonte (`--jev-max-source-bytes`, padrão: só o orçamento de tokens).**
  `OnlineConfig.max_source_bytes` limita o total de fonte semântica **renderizada** nos itens
  `semantic_location`, não a avaliação.
  - Um bloco selecionado que passaria do limite continua `selected_source`, mas sai sem
    `content`, com o motivo no `why_included`, e a limitação `semantic_source_capped` conta
    quantos ficaram assim.
  - O conteúdo dos itens do Ripwire não é afetado.
- O `main` passa as duas flags para o `OnlineConfig`, e nenhuma opção `--jev-*` fica mais sem
  efeito.
- **Testes.** Três novos no seam 1: o prazo com relógio pausado (a admissão responde, o lote
  de seleção demoraria 10 s e o prazo é de 200 ms), um prazo não atingido e o limite de fonte.
  Uma mutação que desliga o timer do prazo derruba o teste. Os testes do núcleo online rodaram
  20 vezes seguidas sem falha.
- Suítes: 200 verdes no build padrão e 210 com `online`, 2 ignorados; clippy e fmt limpos nas
  duas.

## D-079 — Redaction de texto remoto

Fatia S5.6 do plano (PRD §23.9; v0.1 §13.3 e §17).

- **Mapeamento.** Por construção, nenhum erro carrega o corpo da resposta remota: o
  `ClassifyError` só leva categoria e status HTTP, e os erros do `reqwest` viram categorias
  (D-069). O caminho online não escreve em stderr. O único texto remoto que sobrevivia era o
  `Retry-After` bruto do `429`, visível no `Debug` do erro.
- **`online::redact::remote_text(texto, segredo, máx)`**, puro: mantém só ASCII visível e
  espaço (o que remove controles, sequências ANSI, quebras de linha e não-ASCII), troca toda
  ocorrência do segredo por `[redacted]`, apara e corta em `máx` bytes. A troca vem depois da
  filtragem, então um segredo partido por um caractere de controle também é pego.
- O `JevClient` passa o `Retry-After` por ela, com a própria credencial e o limite de 64
  bytes. Um valor redigido não é lido como espera, e o scheduler usa o padrão de 1 s (D-075).
- **Testes.**
  - Seam 2: a função sobre ANSI, BEL, CR/LF, não-ASCII, tamanho e segredo partido.
  - Seam 4: uma varredura com respostas `429` (duas), `401`, `403`, `500`, `409` e `200` com
    corpo que ecoa a chave e mensagem remota. Nenhum `Display`, `Debug` ou categoria contém a
    chave, a mensagem remota ou caracteres fora do ASCII visível.
  - Achado no próprio teste: a primeira versão pôs um BEL no header, o hyper recusou a resposta
    inteira (`network`), e o caso passou sem tocar o `Retry-After`. Com a chave ecoada em texto
    limpo, o teste falhou como devia antes da implementação.
- O status, os limites de status e os e2e já cobriam a ausência de credencial (D-072).
- Suítes: 201 verdes no build padrão e 212 com `online`, 2 ignorados; clippy e fmt limpos nas
  duas.
- **Ponto de parada 3:** CA-ONLINE-09 (D-075), 11 (D-073) e 12 (D-077) verdes.

## D-080 — Lookahead de um nível

Fatias S5.7 a S5.9 do plano (D-061).

- **Mecânica.** Depois da admissão dos paths do planner, entram os arquivos regulares que
  estão diretamente nos diretórios dos paths **admitidos**:
  - sem descer em subdiretórios e sem seguir symlinks (`WorkspaceReader::files_in`, que também
    respeita `.gitignore`);
  - em ordem de path, sem repetir candidatos, até `--jev-lookahead-max` no total (padrão 32;
    0 desliga);
  - cada um passa pela política completa do `snapshot`. Irmãos inelegíveis são pulados em
    silêncio: são exploração, não candidatos que o Ripwire nomeou, e não marcam a descoberta
    como incompleta. O teto também não marca, como o de 16 candidatos (política de custo).
  - Os admitidos seguem para a seleção, com os mesmos lotes, cache, frescor, prazo e limite de
    requests. No merge viram `semantic_location`, e o `why_included` diz "found beside a
    ripwire candidate". Nunca ganham caller, teste ou risco.
- **S5.8.** `Discovery.semantic_only` conta os arquivos do lookahead admitidos, o ganho além
  do Ripwire, e o status expõe o acumulado em `online.semantic_only_candidates`.
- **S5.9 não se aplica como escrito.** Sem Ripwire não há candidatos do planner, e portanto
  não há lookahead. A linha "Ripwire falha → preservar evidência semântica" do §23.4 pressupõe
  a fronteira remota que D-056 removeu. No lugar dela, um teste-guarda: com o Ripwire fora,
  erro estruturado `upstream_unavailable` e nenhuma chamada ao classificador (CA-07).
- **Testes.** Quatro novos no seam 1: irmãos elegíveis admitidos e selecionados, sem descer em
  subdiretórios; elegibilidade e teto (ordem de path, teto 3); ganho no status (1 com o irmão
  admitido, 0 rejeitado); e a guarda sem Ripwire. O teste de flags da CLI ganhou
  `--jev-lookahead-max`.
- **Medições.** O overhead local, em release, ficou em p95 21,8 ms, sem mudança. Suítes: 201
  verdes no build padrão e 216 com `online`, 2 ignorados; clippy e fmt limpos nas duas.
- **Teste ao vivo** (autorizado em D-076), mesma tarefa de orientação do D-076:
  - com 100.000 tokens, 55 itens; o lookahead encontrou e **selecionou `src/budget.rs`** (p =
    0,90 e 0,82), o arquivo que faltara em D-076, além de `local.rs`, `online/request.rs` e
    outros quatro admitidos sem trecho;
  - **com os padrões (2.500 tokens, 24 requests), numa chamada fria, nenhum item do lookahead
    aparece.** A admissão de até 16 + 32 arquivos, com previews de 16 KiB (2 por request),
    consome sozinha os 24 requests. Com 8.000 tokens, a descoberta bateu no limite e saiu
    `incomplete`. O `budget.rs` só apareceu quando o cache das chamadas anteriores liberou
    requests para a seleção;
  - `semantic_only_candidates` chegou a 28 em duas chamadas, e a chave não apareceu em nenhuma
    saída.
- **Decisão pendente com o usuário: equilibrar o lookahead dentro dos 24 requests.** As opções
  estão na resposta desta sessão; nenhuma foi aplicada.
- **Falha intermitente, 2ª ocorrência.** Um teste do build padrão falhou uma vez, de novo na
  primeira execução depois de uma recompilação (a 1ª foi em D-072). Não reproduziu em 46
  execuções, 3 delas sob carga (compilação release e a suíte `online` em paralelo). O nome do
  teste não foi capturado. Próximo passo: rodar o CI com `--no-fail-fast` e guardar a saída.

## D-081 — Equilíbrio do lookahead

Decisão pendente de D-080. O usuário escolheu a opção recomendada: **preview menor no
lookahead e seleção pela probabilidade de admissão**. O limite de 24 requests e o teto de 32
vizinhos continuam.

- **Preview do lookahead: 4 KiB** (`LOOKAHEAD_PREVIEW_BYTES`, `Snapshot::preview_at`). Cabem
  ~8 arquivos por request, em vez de 2, e 32 vizinhos custam ~4 requests de admissão em vez
  de 16. Os paths do planner mantêm os 16 KiB da norma v0.1 §6.3. Como a chave do cache inclui
  o range do preview, as duas formas nunca se confundem.
- **Seleção pela probabilidade de admissão.** As unidades dos arquivos admitidos são enviadas
  do arquivo mais provável para o menos provável; empates mantêm a ordem do Ripwire. Com o
  limite apertado, quem perde é o menos provável.
- **Ordem dos `semantic_location` pela probabilidade**, dentro da própria faixa. No teste
  frio anterior, o `src/budget.rs` (0,88) ficava atrás de trechos do planner com 0,60, só
  porque vinha depois na lista de arquivos, e com 2.500 tokens era cortado. Agora os itens
  só-semânticos se ordenam entre si pela probabilidade. Os itens do Ripwire não mudam de lugar
  e os scores não se misturam com o ranking dele (§23.4). É a mesma regra da opção escolhida,
  aplicada à renderização.
- **Testes.** Quatro novos: o preview de 4 KiB no seam 2; no seam 1, a admissão com preview
  curto só no lookahead, a seleção começando pelo arquivo mais provável e a ordem dos
  `semantic_location` com os itens do Ripwire intactos.
- **Ao vivo, a frio, com os padrões** (mesma tarefa de D-076 e D-080, processo novo): 19
  requests (antes 24, no limite), descoberta `complete`, 2.394 de 2.500 tokens, e **o
  `src/budget.rs` aparece** como o primeiro trecho semântico (p = 0,89), junto com
  `online/request.rs` (0,85, também do lookahead).
- Suítes: 209 verdes no build padrão e 220 com `online`, 2 ignorados; clippy e fmt limpos nas
  duas.
- **Falha intermitente, 3ª ocorrência**, agora na suíte `online`, de novo na primeira execução
  depois de uma recompilação. Não reproduziu em 8 execuções seguidas.
  - Suspeito: `a_hung_upstream_times_out_and_is_restarted` (da Fase 0/1). O timeout global de
    500 ms vale também para a segunda chamada, ao Ripwire real recém-reiniciado.
  - Não se confirmou em 15 execuções sob carga de 24 processos em 12 CPUs.
  - Mudança de procedimento: toda verificação passa a guardar a saída completa no
    scratchpad, para registrar o nome do teste na próxima ocorrência.

