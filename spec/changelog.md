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
| 2026-09-27 17:55 | Proposta: rascunho de `prompts/v1` e tetos 4 em voo / 24 requests | [D-062](#d-062--promptsv1-e-tetos-proposta) |
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
| 2026-09-27 19:41 | S5.10–S5.11: as 15 métricas do §23.11 em `status.online.metrics` e as etapas online em `recent_requests[].stages`; só contagens e tempos; 213/224 verdes | [D-082](#d-082--métricas-e-etapas-online) |
| 2026-09-27 19:45 | S5.12: `doctor --jev-probe` envia uma pergunta sintética embutida no binário; ao vivo, `jev-1.13.0` respondeu em 304 ms; 218/229 verdes | [D-083](#d-083--doctor---jev-probe) |
| 2026-09-27 19:49 | S5.13: `install --online` acrescenta a flag e referencia a chave pelo nome (`${VAR}` no Claude Code, `env_vars` no Codex, validado pelo parser do Codex 0.157); hooks continuam offline; 219/230 verdes | [D-084](#d-084--install---online) |
| 2026-09-27 19:51 | S5.14: dois testes live ignorados (corpus sintético e caminho completo com Ripwire real); ao vivo verdes, digest idêntico ao S4.0b; 219/230 verdes, 4 ignorados no `online` | [D-085](#d-085--testes-live) |
| 2026-09-27 19:53 | Documentação da Fase 5 (README, skill, AGENTS.md, PRD); ponto de parada 4: Fases 4 e 5 completas, exceto o A/B; falha intermitente identificada (`a_hung_model_is_killed_at_the_hard_limit`) e corrigida | [D-086](#d-086--documentação-e-ponto-de-parada-4) |
| 2026-09-27 19:56 | Branch `fase-4-online` publicado e PR #1 aberto para o `master` | [D-087](#d-087--pr-das-fases-4-e-5) |
| 2026-09-27 20:27 | CI do PR #1: 1ª execução falhou por ETXTBSY (corrida pré-existente, Linux), 2ª passou; merge bloqueado pelo classificador de permissões; revisão `/tdd` com 17 achados, todos tratados no PR, incluindo um defeito de produto (resposta desconhecida não marcava `incomplete`) | [D-088](#d-088--ci-revisão-tdd-e-correções) |
| 2026-09-27 23:38 | `/security-review` do PR #1: nenhuma vulnerabilidade acima do limiar; das duas observações, `*.env` sem ponto passou a ser nome sensível | [D-089](#d-089--revisão-de-segurança) |
| 2026-09-28 00:12 | README: referências conferidas após o arquivamento em `spec/old/` (todas válidas); citações "PRD §x" viram links para as seções do PRD atual | [D-090](#d-090--links-do-readme-para-o-prd) |
| 2026-09-28 16:15 | Revisão do repositório: cinco defeitos de robustez corrigidos em TDD — dois panics do leitor tolerante, `dedup` sobre vetor não ordenado, bloco de `memory_recall` descartado em silêncio, espera de nota por polling e caminho não-UTF-8 no `install` | [D-091](#d-091--revisão-do-repositório-e-correções-de-robustez) |
| 2026-09-28 16:34 | As duas observações do D-091 corrigidas em TDD: o registro `Inflight` não guarda mais o id de um `tools/call` que o SDK rejeita antes do handler, e uma nota servida pela geração de outra chamada é marcada `cached` | [D-092](#d-092--vazamento-do-inflight-e-cache-hit-de-nota) |
| 2026-09-28 16:45 | As duas ressalvas do D-092 fechadas: o `install` valida o workspace antes de tocar o disco (testável em qualquer plataforma) e o registro `Inflight` ganhou teto com remoção do mais antigo | [D-093](#d-093--fechamento-das-ressalvas-do-install-e-do-inflight) |
| 2026-09-28 16:52 | Teto do `Inflight` revertido por decisão do usuário: a convenção de testar só por costuras públicas pesa mais que a defesa em profundidade sem defeito demonstrado | [D-094](#d-094--reversão-do-teto-do-inflight) |
| 2026-09-28 17:47 | `spec/prompt/ci-cd.md` preenchido com os fatos do código, traduzido para o português e auditado quanto a segurança e práticas de DevOps | [D-095](#d-095--prompt-de-testes-de-propriedade-e-cicd) |
| 2026-10-03 17:00 | Fase 2 do plano do `--memory` (controle Jev) feita em TDD (T2.1 a T2.13; T2.0 escrita, pendente de rodada com a chave): Choice e pedidos de estado, decisões tipadas, transporte comum, `memory-prompts/v1`, fila com leases por lock e quota de 24 h, worker com typing, candidatos e relações, commit por par, falhas do provider, teto único de requisições, worker no `serve --memory` e `memory drain --online`, métricas de custo; PRD principal §23.1/§23.2/§23.3/§23.5 autorizam o worker | [D-138](#d-138--fase-2-do---memory-controle-jev) |
| 2026-10-03 15:12 | Fase 1 do plano do `--memory` feita em TDD (T1.1 a T1.16, um commit por tarefa, mutação em cada uma): `--memory` e `--memory-*` no parse; registro `memory/v1`, identidade, admissão, relógio e sequência; store privado com spool, snapshot, tetos, lock, retenção, `forget` com tombstones e revogação; `memory status\|forget\|add\|resume`; coleta pelas tools e pelo `hook --memory`; `install`/`doctor`. O `serve --memory` ainda não liga a coleta (T2.11) | [D-137](#d-137--fase-1-do---memory-store-e-coleta) |
| 2026-10-03 13:05 | Fase 0 do plano do `--memory`: o PRD jev-mem passa à v0.3, reconciliado com o estudo `docs/jev-mem.md` e o PDF (mesmo hash; citações do paper e do broker conferem; seis divergências do estudo decididas a favor do PRD) e com as superfícies do D-135 no §4 (`memory add`, `memory resume`, `hook --memory`, `install --memory`, `memory drain --online`) | [D-136](#d-136--fase-0-do---memory-prd-jev-mem-v03) |
| 2026-10-03 12:50 | Aceitas as cinco decisões pendentes do plano do `--memory`: `memory add` entra na Fase 1; `--online` passa a valer também em `memory drain`; o hook liga a coleta com `hook --memory` (só spool local, sem HTTP, gravado por `install --memory`); `memory resume` reativa a coleta depois de `forget --all`; a Fase 2 pode começar antes do A/B do `--online`, como experimental | [D-135](#d-135--decisões-pd-1-a-pd-5-do---memory) |
| 2026-10-03 12:36 | Plano de implementação do `--memory` ([`spec/plan/jev-mem-plan.md`](plan/jev-mem-plan.md)) a partir do PRD `docs/jev-mem-prd.md`: seis fases, TDD obrigatório por tarefa (teste vermelho, código mínimo, mutação, documentação afetada, portões), 16 achados de validação contra o código e 5 decisões pendentes do mantenedor; nenhum código de produção; linha de base 462/476 testes verdes | [D-134](#d-134--plano-de-implementação-do---memory) |
| 2026-10-02 12:15 | Auditoria da documentação contra o código: README e PRD §8.4 ganham a coalescência de edições do D-106 (`--edit-interval-ms`) e os orçamentos dos hooks; o README ganha `prompt --budget`, `--jev-provider`, as flags de binário e prazo do `ripwire-eval` e a procedência atual das fixtures | [D-133](#d-133--auditoria-da-documentação-contra-o-código) |
| 2026-10-01 23:40 | Documentação alinhada ao D-131: o exemplo `integrations/claude-code/settings.json` ganha `Bash` no matcher do `PostToolUse` (como o `install` grava desde o D-129), o PRD deixa de dar a validação da barra como pendente, o `handoff.md` vai até o D-131, com o campo `agent` como pendência, e o diagrama ganha a barra de status e o `git` | [D-132](#d-132--documentação-alinhada-ao-d-131) |
| 2026-10-01 23:16 | Fecha a pendência `bashEditDiff` do D-129: a lista de arquivos que o próprio Claude Code manda no `PostToolUse` do Bash substitui a impressão do git; depois do primeiro payload com o campo, a sessão não chama mais o `git`, e a impressão fica para versões que não o mandam | [D-131](#d-131--a-lista-do-próprio-claude-code-substitui-a-impressão-do-git) |
| 2026-10-01 23:00 | Fecha as divergências 5 e 6 do D-128: no `--detail`, `último contexto` ganha a própria idade e a do snapshot vira `visto há`; no primeiro prompt, um envelope só com limitações não é injetado nem conta em `inj` | [D-130](#d-130--duas-idades-no-detalhe-e-o-primeiro-prompt-sem-conteúdo-não-é-injetado) |
| 2026-10-01 21:50 | Edições feitas pelo shell chegam ao hook de edição: o `PostToolUse` do Claude Code casa `Bash`, e uma impressão digital do `git status` decide, antes de subir o ripwire, se o comando mudou arquivos; só leitura não sobe o ripwire; uma árvore lenta ou suja demais desliga a detecção pela sessão; fecha a divergência 2 do D-128; 32 testes novos (457 padrão, 471 com `online`) | [D-129](#d-129--edições-pelo-shell-chegam-ao-hook-de-edição) |
| 2026-10-01 20:43 | Validação manual da barra (§24.10) numa sessão real do Claude Code 2.1.285: barra e snapshot batem em todos os passos, payload real confere com §24.2/§24.8; segunda rodada com o comando instalado igual; 6 divergências registradas (a 1 e a 3 eram erros do roteiro; a 2 é um ponto cego, edições por Bash não chegam ao hook; a 4 é conforme por desenho; a 5 é rótulo ambíguo; a 6 conta certo, mas injeta envelope só com limitações; decisões pendentes); fixture `tests/fixtures/statusline/claude_code.json` e 1 teste novo (420 padrão, 434 com `online`); fecha as pendências do D-123 | [D-128](#d-128--validação-manual-da-barra-e-fixture-de-payload-real) |
| 2026-10-01 21:00 | As pendências menores da barra de status fechadas (D-123): rótulo do modelo, caracteres invisíveis, `reuso` saturado, `ctx` por campo, leitura e escrita privadas sem seguir links, `hook-stats` sem sessões vazias, nota e propriedade do `install`; 19 testes novos, e 3 na Revisão (419 padrão, 433 com `online`) | [D-127](#d-127--pendências-menores-da-barra-de-status) |
| 2026-10-01 19:10 | O marcador `#ripwire-off`/`#ripwire-on` vale mesmo quando o ripwire não sobe: a pausa é confirmada e salva, a retomada é salva antes de a falha ser reportada; fecha o defeito registrado no D-123; 1 teste novo (397 padrão, 411 com `online`) | [D-126](#d-126--o-marcador-vale-mesmo-sem-ripwire) |
| 2026-10-01 18:40 | O marcador `#ripwire-off`/`#ripwire-on` só vale como palavra inteira no fim ou no começo do prompt; citado no meio do texto (um relatório de subagente que o mencionava pausou os hooks de uma sessão real) não altera nada; 1 teste novo (396 padrão, 410 com `online`) | [D-125](#d-125--o-marcador-de-opt-out-só-vale-na-borda-do-prompt) |
| 2026-10-01 18:10 | Com `--color always`, `hooks off` fica vermelho e `hooks on` azul claro (`38;5;117`); `hooks sem dados` segue sem cor; 1 teste novo (395 padrão, 409 com `online`) | [D-124](#d-124--cores-do-estado-dos-hooks-na-barra) |
| 2026-10-01 16:40 | A barra de status do Claude Code (`statusline`, projeção publicada pelos hooks, `install --statusline`) é implementada em 9 tarefas por subagentes; 61 testes novos (384 padrão, 398 com `online`); p95 de 2,8 ms medido em release; **validação manual e fixture de payload real pendentes** | [D-123](#d-123--a-barra-de-status-é-implementada) |
| 2026-10-01 15:20 | A barra de status entra no PRD como §24 (a spec `spec/status-bar.md`, fundida e removida), o plano vai para `spec/plan/`, e a barra entra no roadmap antes da Fase 6 | [D-122](#d-122--a-barra-de-status-entra-no-prd) |
| 2026-10-01 13:28 | Três mecanismos reproduzem o sintoma do D-117 num corpus de diagnóstico (a causa daquela rodada segue sem prova): a porta fixa do endpoint de teste do repositório A (`eaddrinuse`), a data (provada com o relógio congelado) e, fraca, a carga. O Postgres disputado sozinho não derrubou nada. Regra nova: cada tarefa do A fixa relógio e porta no `env` | [D-121](#d-121--três-mecanismos-que-reproduzem-as-falhas-de-validação-do-d-117) |
| 2026-10-01 08:46 | O cancelamento sob HTTP/2 ganha teste: um fixture `h2` (h2c) mostra que cada stream em voo recebe `RST_STREAM(CANCEL)`; `h2` entra como dev-dependency, já presente no grafo pelo `reqwest` | [D-120](#d-120--o-cancelamento-sob-http2-ganha-teste) |
| 2026-10-01 08:14 | Duas pendências do handoff fechadas: o `sha2` >= 0.11 vai para o `ignore` do dependabot, e os links dos planos no PRD, no changelog e nos próprios planos passam a apontar para `spec/plan/` | [D-119](#d-119--pendências-do-handoff-sha2-no-dependabot-e-links-dos-planos) |
| 2026-10-01 07:50 | Revisão do PR #29: a guarda de shell enxerga atribuições, invólucros, `sh -c` e aspas; `fix` validado como commit; edição do agente num arquivo que o `setup` tocou volta a contar. E o `handoff.md` | [D-118](#d-118--revisão-do-pr-29-e-handoff) |
| 2026-10-01 01:06 | Corpus real (32 tarefas em três repositórios, dois privados, fora deste repositório) e o que montá-lo ensinou: hooks e índice de outra ferramenta versionados num repositório, `setup`/`env`/`teardown`, `validate`, e um `check` que falha sem causa provada | [D-117](#d-117--o-corpus-real-três-repositórios-e-duas-falhas-de-isolamento) |
| 2026-09-30 23:36 | Plano e instrumentos dos itens 1 e 2: `ripwire-eval` (A/B do §16.2, §17 e §23.15) e `hook-stats` (§21.3); o `session_hits` dos hooks morria com o processo, e o clone do A/B vazava a resposta das tarefas tiradas do histórico | [D-116](#d-116--plano-da-avaliação-ab-e-de-session_hits-em-uso-real) |
| 2026-09-29 09:01 | Diagrama de arquitetura versionado em `spec/diagrams/`: fonte JSON do archify (a fonte da verdade) e HTML entregue, com fontes fixadas no commit `5daaf27` | [D-115](#d-115--diagrama-de-arquitetura-versionado) |
| 2026-09-29 00:29 | O `sha2` 0.11 é recusado: ele **não** muda o digest (premissa minha, errada), e subir o nosso direto apenas **duplica** o crate, porque o `rust-mcp-sdk` pinado traz o 0.10 | [D-114](#d-114--o-sha2-011-é-recusado-e-uma-premissa-minha-estava-errada) |
| 2026-09-29 00:08 | Fatia G: P1.3 (escalonador) — teto de voo, orçamento de requisições contra o oráculo do classificador, e resposta que nunca migra de pergunta; a proposta de PBT/CI está cumprida | [D-113](#d-113--fatia-g-o-escalonador-e-a-quarta-vez-que-o-instrumento-era-o-problema) |
| 2026-09-29 00:00 | Fatia F: P0.1, P0.2 e P0.3 com tempdir; nenhum defeito nas duas fronteiras de segurança, mas **quatro** das minhas asserções não testavam nada e só a mutação mostrou | [D-112](#d-112--fatia-f-duas-fronteiras-de-segurança-fuzzadas-e-quatro-testes-meus-que-não-testavam-nada) |
| 2026-09-28 23:48 | Fatia E: `markup::parse` **abortava o processo** com 10 000 elementos aninhados (estouro de pilha, não capturável); teto de profundidade, `markup` a `pub`, e o P0.11 pela costura pública | [D-111](#d-111--fatia-e-o-leitor-tolerante-derrubava-o-processo-por-aninhamento) |
| 2026-09-28 23:36 | Fatia D: 12 propriedades (P0.4, P0.10, P0.13, P1.1, P1.2), `local::wrap` extraído para o controle de injeção ser alcançável, e a ordem prometida no P0.4 do prompt corrigida | [D-110](#d-110--fatia-d-doze-propriedades-e-uma-afirmação-do-prompt-que-não-se-sustenta) |
| 2026-09-28 23:28 | Fatia C: 22 propriedades sobre as superfícies puras, e o achado que elas existiam para achar — um segredo com caractere não-ASCII vazava seu esqueleto ASCII para status, log e agente | [D-109](#d-109--primeiras-propriedades-e-um-vazamento-de-credencial-que-elas-fecharam) |
| 2026-09-28 23:14 | Fatia B: `deny.toml` com as quatro seções verdes (allowlist exata de 8 licenças, não um superconjunto generoso), `cargo-deny` **agendado no `master` e nunca em PR**, e dependabot com a política de pin do `rust-mcp-sdk` | [D-108](#d-108--cadeia-de-suprimentos-cargo-deny-agendado-e-dependabot) |
| 2026-09-28 23:05 | Fatia A da proposta de PBT/CI: workflow com dois jobs, `permissions` mínimo, actions por SHA, `--locked`, timeout e `concurrency`; `forbid(unsafe_code)`, `deny(print_stdout)`, porta do CA-10 e guarda de fixture — os quatro passavam limpos antes de serem exigidos | [D-107](#d-107--ci-endurecido-dois-jobs-e-quatro-promessas-viram-portas) |
| 2026-09-28 22:43 | Opção E do item 4: uma rajada de edições passa a ser **uma** pergunta ao ripwire em vez de uma por edição — medido 1 injeção em 12 edições, cada uma custando ~99 ms jogados fora; 12 edições caem de 1169 para 278 ms | [D-106](#d-106--uma-rajada-de-edições-é-uma-pergunta-não-uma-por-edição) |
| 2026-09-28 22:28 | Item 4 fechado na opção B: a versão do ripwire vem do estado de sessão em vez de um processo por evento. E o ripwire real, agora no PATH, **derruba a tese central da proposta** — C e D ficam recusadas, E é a única que sobra | [D-105](#d-105--a-versão-do-ripwire-deixa-de-custar-um-processo-por-evento-de-hook) |
| 2026-09-28 22:09 | Itens 2 e 5 decididos: o precheck do `is_fresh` **recusado** (ele pularia a reverificação de elegibilidade, que é a fronteira de consentimento) e o fan-out do `context_after_edit` **feito** nos dois laços, 3,6–4,0x e custo plano no número de símbolos | [D-104](#d-104--decisão-dos-itens-2-e-5-recusado-e-feito) |
| 2026-09-28 21:34 | Revisão dos testes-ouro: só um era sensível a plataforma, e ele pegava um **defeito real** — `add_notes` estourava itens cujo corpo já tinha ido ao modelo local. Corrigido reservando a escrituração escrita após o encaixe | [D-103](#d-103--uma-nota-não-custa-mais-um-item-que-já-foi-evidência) |
| 2026-09-28 21:02 | O teste-ouro do D-099 fixava `shown` em números de **uma** máquina e quebrou no CI: `provenance.workspace` carrega o caminho do workspace, ~40 bytes mais curto no Linux. Trocado por um limite de desperdício medido em tempo de execução | [D-102](#d-102--o-teste-ouro-do-d-099-era-dependente-de-plataforma-e-deixou-o-master-vermelho) |
| 2026-09-28 20:48 | Proposta do item 4 em `spec/plan/`: ~43 ms de overhead fixo por evento de hook, dos quais ~38 são duas subidas de processo; cinco opções comparadas, e a recomendação é a barata e isolada (não subir um processo só para ler a versão) | [D-101](#d-101--proposta-para-o-item-4-o-ripwire-por-evento-de-hook) |
| 2026-09-28 20:32 | Itens 7 e 10 medidos e **recusados** — as duas premissas estavam erradas e a "otimização" do merge era 3x mais lenta; e os números do D-096 ao D-099 refeitos em release, porque os publicados eram de build debug | [D-100](#d-100--itens-7-e-10-medidos-e-recusados-e-os-números-do-d-096-ao-d-099-refeitos-em-release) |
| 2026-09-28 20:18 | O `budget_tokens` passa a ter teto aplicado (100.000, o que o schema MCP já declarava sem impor) e o shaping do envelope deixa de re-serializar o envelope por candidato: 16x no orçamento padrão e 48x no teto | [D-099](#d-099--teto-aplicado-no-budget_tokens-e-shaping-do-envelope-em-tempo-linear) |
| 2026-09-28 20:05 | `status()` deixa de pagar uma ida e volta upstream por leitura: a sonda de disponibilidade vale por `STATUS_PROBE`; 10 leituras caem de 220 para 22 ms a 20 ms de RTT, e um ripwire ocupado de 3 s para 1 s | [D-098](#d-098--a-sonda-de-disponibilidade-do-status-reaproveitada-por-uma-janela) |
| 2026-09-28 19:47 | Teto nos três caches sem limite, dimensionado por medição: `SemanticCache` em 4.000, `SessionMemory` em 5.000, notas em 500, despejo do mais antigo | [D-097](#d-097--teto-nos-três-caches-medido-antes-de-escolher-os-números) |
| 2026-09-28 18:20 | Revisão de arquitetura: 10 gargalos medidos e ordenados; `snapshot` fica 2–4x mais rápido, o gate paga 2 idas e voltas em vez de 3, e o fan-out dos `edit_check` foi revertido por colidir com RF-14 | [D-096](#d-096--gargalos-de-arquitetura-medidos-e-os-dois-primeiros-corrigidos) |

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

Plano completo em [plan-fases-2-3.md](plan/plan-fases-2-3.md). Estado: **proposta, aguardando
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

Plano completo em [plan-fases-4-5.md](plan/plan-fases-4-5.md). Estado: **proposta, aguardando
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

## D-082 — Métricas e etapas online

Fatias S5.10 e S5.11 do plano (PRD §23.11).

- **Métricas.** `status.online.metrics` traz as 15 métricas do §23.11, com os mesmos nomes.
  - Por request, medidas pelo `online::metrics::Metered`, que embrulha o classificador
    configurado; o scheduler só vê o embrulho:
    - `jev_requests_total` e `jev_questions_total`;
    - `jev_in_flight`, um medidor que também desce quando o request é abortado;
    - `jev_batch_items` e `jev_request_bytes`, cada um como `{total, max}`;
    - `jev_latency_ms` com p50/p95/p99 nas últimas 256 latências.
  - `jev_response_bytes` vem do próprio `JevClient`, que conta o corpo lido; a trait
    `Classifier` ganhou `response_bytes()`, que vale zero por padrão.
  - Por descoberta, vindas do relatório do scheduler e do coordenador:
    `jev_cache_hits_total`, `jev_rate_limit_total`, `jev_retry_total`, `jev_split_total`,
    `semantic_candidates_total` (arquivos perguntados, do planner e do lookahead),
    `semantic_selected_ranges_total` e `semantic_only_candidates_total`.
  - `online_context_tokens_estimated` mede o envelope final: bytes dos itens
    `semantic_location` e das anotações `semantic` que chegaram ao agente, divididos por 4
    como no orçamento.
  - Os campos já existentes do bloco `status.online` continuam, por compatibilidade.
- **Etapas (spans).** Cada chamada de um processo `--online` ganha `stages` no seu registro de
  `recent_requests`, com nome, duração e número de requests. Aparecem na ordem em que terminam:
  `semantic.navigation.batch` (admissão, com lookahead), `semantic.selection.batch`,
  `semantic.discovery` (a descoberta inteira), `context.merge` e `context.budget`.
  - O registro compartilha o `request_id` de `provenance.request_id` e nunca traz query, path
    ou código.
  - Processos offline não têm o campo.
  - Decisão do plano mantida: nada do crate `tracing`. As etapas são registros do status,
    como as chamadas upstream.
- **Testes.** Quatro novos no seam 1: todas as métricas conferidas contra o que o dublê
  recebeu, sem path nem query; retry, split e `429` contados; as etapas sob o `request_id`;
  nenhuma etapa no offline. O e2e com o cliente real passou a conferir `jev_response_bytes`.
- Overhead local, em release: p95 21,5 ms, sem mudança. Suítes: 213 verdes no build padrão e
  224 com `online`, 2 ignorados; clippy e fmt limpos nas duas.

## D-083 — `doctor --jev-probe`

Fatia S5.12 do plano (§23.6, D-064).

- **`doctor::jev_probe(&dyn Classifier)`**, no build padrão, envia **uma** pergunta de
  `file_admission` pelo `prompts/v1`, com conteúdo inventado e embutido no binário
  (`PROBE_PATH = probe/example.py`, uma função `add`). Ela não recebe o workspace, então não
  tem como ler nada dele.
  - A checagem `jev_probe` traz modelo, latência e probabilidade quando dá certo.
  - Na falha, só a categoria e o status do erro (D-069, D-079).
- **CLI.** `doctor --jev-probe [--jev-model M]`; `--jev-model` sem `--jev-probe` é erro de uso.
  Com a feature `online`, o probe monta o `JevClient` com a credencial do ambiente, o endpoint
  allowlisted e o modelo pinado por padrão. Sem a feature, a checagem falha e diz como
  recompilar. Sem `--jev-probe`, o `doctor` não cria cliente HTTP.
- **Testes.**
  - Seam 1: um único request, conteúdo sintético, guidance do `prompts/v1` e nenhum path do
    workspace; a falha mostra só a categoria.
  - Seam 4: exatamente um request no fio.
  - Seam 5: flags; sem o probe, nenhuma checagem `jev_probe`; sem a feature, falha clara.
- **Ao vivo (autorizado em D-076):** `jev-1.13.0` respondeu à pergunta sintética em 304 ms
  (p = 0,77), com as outras 8 checagens `ok` e a chave ausente da saída. Sem a credencial, a
  checagem falha com a mensagem que nomeia a variável.
  - Detalhe menor: essa mensagem é a mesma do `serve` e começa com "--online needs...". No
    `doctor` ela continua clara, mas pode ganhar um texto próprio no S5.13.
- Suítes: 218 verdes no build padrão e 229 com `online`, 2 ignorados; clippy e fmt limpos nas
  duas.

## D-084 — `install --online`

Fatia S5.13 do plano (§23.6, D-064).

- **Claude Code.** No `.mcp.json`, o servidor ganha `--online` nos `args` e
  `"env": {"RIPWIRE_BROKER_JEV_API_KEY": "${RIPWIRE_BROKER_JEV_API_KEY}"}`. É uma referência
  que o Claude Code expande do próprio ambiente ao subir o servidor; o valor nunca é gravado.
- **Codex.** O trecho de `config.toml` continua só impresso (D-033) e ganha `"--online"` nos
  `args` e `env_vars = ["RIPWIRE_BROKER_JEV_API_KEY"]`, que repassa a variável pelo nome.
  - Achado: `codex mcp add --env` só aceita `KEY=VALUE` literal, o que gravaria a chave. O
    campo `env_vars` foi achado nos tipos do binário do Codex 0.157 (`RawMcpServerConfig`).
  - Validado com o próprio Codex: com o trecho num `CODEX_HOME` temporário, `codex mcp list`
    e `codex mcp get` leem a configuração e mostram `RIPWIRE_BROKER_JEV_API_KEY=*****`.
- **Hooks continuam offline** (D-064): os comandos de hook nunca recebem `--online`.
- O texto de consentimento do §23.6 sai com todo `install --online`. Ele lembra também que a
  chave vem do ambiente do host, que o binário precisa da feature `online` e que
  `doctor --jev-probe` confere a instalação.
- Reinstalar sem `--online` desliga o modo: a entrada do servidor é substituída e perde a flag
  e o `env`.
- O `install` nunca lê a variável da chave. O teste roda o binário com a chave no ambiente e
  confere que ela não aparece nem na saída nem nos arquivos gravados.
- **Mensagem da credencial ausente** (D-083): agora diz "RIPWIRE_BROKER_JEV_API_KEY is not set
  in the environment (--online and --jev-probe need it)", o que serve ao `serve` e ao `doctor`.
- **Teste.** Um e2e do binário no seam 5 cobre dry-run, `--write` com hooks, Codex, ausência
  da chave em tudo e reinstalação sem `--online`.
- Suítes: 219 verdes no build padrão e 230 com `online`, 2 ignorados; clippy e fmt limpos nas
  duas.

## D-085 — Testes live

Fatia S5.14 do plano (§23.14).

- `tests/online_live.rs` só compila com a feature `online`, e seus testes são `#[ignore]`: a
  suíte padrão continua offline (CA-10). Para rodar:
  `RIPWIRE_BROKER_JEV_API_KEY=... cargo test --features online --test online_live -- --ignored`.
  Sem a chave, cada teste avisa e sai.
- Só envia conteúdo inventado (o corpus do S4.0b e o `common::sample_repo`) e só imprime
  digests, probabilidades e tempos.
  - `a_real_provider_classifies_the_synthetic_corpus`: respostas válidas, uma por pergunta; o
    modelo admite `auth.py` e o teste e rejeita o CSV sem relação; `validate_token` é selecionado;
    o cliente conta os bytes recebidos.
  - `a_real_provider_enriches_a_synthetic_repository`: o caminho completo, com broker, Ripwire
    real e `JevClient`. A descoberta sai `complete`, algum item tem evidência `semantic` e o
    orçamento é respeitado.
- **Rodada ao vivo** (autorizada em D-076): os dois verdes em 1,1 s.
  - O digest do request de admissão (`d7d3e4…`) é idêntico ao gravado no S4.0b: o
    `prompts/v1` não mudou.
  - As probabilidades variaram pouco em relação à gravação (0,79 contra 0,81; 0,65 contra
    0,64). O provider não é determinístico, e por isso os testes live conferem decisões
    (admitido, rejeitado, selecionado), não valores exatos.
  - O caminho completo levou 620 ms com 2 requests.
- Suítes: 219 verdes no build padrão (2 ignorados) e 230 com `online` (4 ignorados); clippy e
  fmt limpos nas duas.

## D-086 — Documentação e ponto de parada 4

Seção §5.4 do plano.

- **README.**
  - Nova seção "Online mode (optional)": como compilar e instalar, o texto de consentimento, o
    que sai da máquina e o que nunca sai, o limite da proteção (a filtragem não garante
    reconhecer todo segredo), quando o classificador roda, os campos que o envelope ganha, os
    tetos e as flags, o status, e o aviso de modo experimental até o A/B.
  - "Secrets" deixou de ser futuro: a chave vem só de `RIPWIRE_BROKER_JEV_API_KEY`.
  - Também mudaram: comandos (`doctor --jev-probe`, `install --online`), requisitos, piso de
    512 tokens e testes (`--features online` e os live).
- **Skill e `AGENTS.md`.** Como ler `semantic`, `semantic_location` e `discovery`:
  probabilidade não é fato, um item do Ripwire `rejected` continua sendo um fato, e uma
  descoberta `incomplete` ou `interrupted` não prova ausência.
- **PRD.** Estado das Fases 4 e 5 no §19 e no §23; a forma real do envelope (§9.1) e do status
  (§9.4); as flags novas no §23.6; e, no fim do §23.17, uma tabela com a resolução e a decisão
  de cada lacuna.
- **Ponto de parada 4.** Todas as fatias de código das Fases 4 e 5 estão feitas, com a barra
  de merge da Fase 4 verde e CA-ONLINE-01 a 15 cobertos. Falta o corpus A/B e a barra de
  produto (§5.3 do plano, §23.15), que dependem de o usuário escolher três repositórios e
  aceitar o custo remoto. Até lá, `--online` é experimental.
- **Falha intermitente identificada e corrigida** (D-072, D-080, D-081). A saída guardada da
  verificação final registrou o nome: `a_hung_model_is_killed_at_the_hard_limit`, em
  `tests/summarizer.rs`, um teste da Fase 3. O suspeito de D-081 estava errado.
  - Causa: o teste roda um script travado com limite de 300 ms, e a primeira linha do script
    grava o próprio PID. Numa primeira execução depois de uma recompilação, com a máquina
    carregada, o shell às vezes era morto antes dessa linha, e a leitura do arquivo de PID
    falhava.
  - O comportamento testado estava certo: o processo tinha sido morto no limite. A corrida
    era do teste.
  - Correção só no teste: o limite passou para 2 s. A checagem de término em menos de 5 s
    continua. Rodou 20 vezes sem falha, 10 delas sob carga de 24 processos em 12 CPUs.

## D-087 — PR das Fases 4 e 5

- O branch `fase-4-online` foi publicado no `CeciApp/ripwire-broker`, e o
  [PR #1](https://github.com/CeciApp/ripwire-broker/pull/1) foi aberto para o `master`. São 18
  commits, do plano (D-058) à documentação (D-086), e 49 arquivos.
- Antes da publicação, o diff inteiro foi varrido atrás da chave do provider: nenhuma
  ocorrência. O `master` local estava igual ao `origin/master`.
- O PR descreve o que revisar com atenção: o teste de CA-10 sobre o grafo resolvido (D-059), o
  piso de 512 tokens (D-072), a sintonia do lookahead (D-081) e a correção do teste
  intermitente (D-086). O item em aberto é o corpus A/B com a barra de produto (§23.15).

## D-088 — CI, revisão `/tdd` e correções

- **CI do PR #1.** A 1ª execução falhou em `the_command_summarizer_feeds_stdin_and_reads_stdout`
  com `Text file busy` (ETXTBSY). A 2ª, no commit seguinte, passou nas duas configurações.
  - Causa: corrida do Linux, anterior a este PR. Um teste grava um script executável enquanto
    outra thread do mesmo processo faz `fork`; o filho herda o descritor de escrita até o seu
    `exec`, e executar o script nesse intervalo falha.
  - Reproduzida num container `rust:1.98.1` com 2 CPUs: 2 falhas em 300 execuções. No macOS
    não acontece.
- **Merge.** O `gh pr merge` foi bloqueado pelo classificador de permissões do modo automático
  ("Merge Without Review"). Não houve tentativa de contornar, e a decisão ficou com o usuário,
  que pediu para corrigir os achados da revisão no próprio PR.
- **Revisão `/tdd`**, feita por um agente sobre os testes do PR, com os critérios da skill:
  testes em seams públicos acordados, sem acoplamento à implementação, sem tautologia, com
  asserções fortes e sem corridas. Os achados foram verificados no código antes de cada
  correção.

| # | Achado | Tratamento |
| --- | --- | --- |
| 1 | O teste de CA-ONLINE-13 não podia falhar: a chave do cache nem recebe path ou fonte | Substituído por um teste no seam 1 que enche o cache com uma chamada real e o inspeciona por `Broker::inspect_semantic_cache()` (a "inspeção" do CA). Uma mutação que grava a query no cache o derruba |
| 2 | O dublê roteirizava falhas por ids internos (`f0`, `u0`, `p0`) e o layout dos lotes | O `FakeClassifier` ganhou gatilhos por etapa e por path; os ids ficaram só para o scheduler, cujos requests o teste monta. O teste de métricas passou a depender só do que o dublê devolveu |
| 3 | CA-ONLINE-01 só era checado no grafo do build padrão | Novo e2e no build `online`: chave no ambiente, sem `--online`; as três tools respondem sem `provenance.online`, e o status diz `offline: true` |
| 4 | Resposta desconhecida sem teste no broker | **Defeito de produto confirmado**: uma pergunta sem probabilidade válida, dentro de uma resposta HTTP válida, não marcava a descoberta como `incomplete`, e a ausência podia ser lida como irrelevância (§23.2). Agora conta em `unknown_answers`, torna a descoberta incompleta e aparece na limitação. Teste vermelho antes da correção |
| 5 | O "informa omissões" do CA-ONLINE-14 podia nunca rodar | A varredura exige que ao menos um orçamento trunque e confere `next_step` |
| 6 | O fixture HTTP/1.1 fecha a conexão a cada request | Novo fixture com keep-alive prova o reuso de uma conexão do pool. A lacuna do cancelamento sob HTTP/2 (reset de stream) ficou documentada no cabeçalho do arquivo |
| 7 | O teste do endpoint só lia um getter | Passou a fixar a assinatura do único construtor público: um parâmetro de URL quebraria a compilação do teste |
| 8 | O teste de cancelamento HTTP dependia de 250 ms reais e tirava a foto antes do `abort()` | Foto depois do `abort()` e prazo de 2 s. Os 250 ms da v0.1 continuam provados no seam 3, com tempo controlado |
| 9 | A métrica de bytes enviados era conferida pela mesma fórmula da produção | No seam 1, limites independentes; o valor exato passou para o seam 4, contra os bytes que o fixture recebeu |
| 10 | Nomes que prometiam mais do que o teste verificava | O teste de inelegíveis confere `incomplete`; o do `doctor` foi renomeado para `doctor_has_no_probe_check_unless_asked`; o do drop agora enfileira de fato um retry, que nunca sai |
| 11 | O teste de cancelamento no cooldown dependia da ordem do `JoinSet` | `j1` responde depois do `429` de `j0`; a contagem no cancelamento não muda nem depois do cooldown. 20 repetições verdes |
| 12 | Os testes live passavam sem a chave | Falham sem a chave (ou sem ripwire) quando pedidos com `--ignored` |
| 13 | Um teste de `install --online` cobria cinco comportamentos | Dividido em seis testes, um por comportamento |
| 14 | Linha morta `let _ = SystemTime::now();` | Removida |
| 15 | `cargo tree --offline` poderia falhar num runner limpo (incerto) | Verificado: passou nas duas execuções do CI, num runner limpo, antes de o `reqwest` ser baixado. Sem mudança |
| 16 | ETXTBSY | Correção nos sete lugares que gravam e executam scripts: `common::write_executable` grava por um `sh` filho, e o processo de teste nunca segura o descritor de escrita. Trocar o nome do arquivo, como o relatório sugeria, não resolveria, porque o descritor herdado aponta para o mesmo inode |
| 17 | O teste do probe comparava com as constantes de produção | Compara com literais |
| — | Fidelidade do dublê | O dublê entra em pânico se uma regra devolver probabilidade fora de [0, 1] ou se o prompt não for de nenhuma etapa do `prompts/v1` |

- Suítes: 225 verdes no build padrão (2 ignorados) e 238 com `online` (4 ignorados); clippy e
  fmt limpos nas duas. Os testes live passaram com a chave e falharam sem ela.
- **CI** no commit das correções (`3aaf8ed`): verde nas duas configurações.
- **Validação do ETXTBSY** no mesmo container (`rust:1.98.1`, 2 CPUs, 16 threads de teste,
  300 execuções de `tests/summarizer.rs`): 0 ETXTBSY e 0 outras falhas, contra 2 em 300 antes.
  - Ressalva estatística: com uma taxa-base de ~0,7%, zerar em 300 execuções aconteceria por
    acaso em ~13% das vezes mesmo sem correção. O número é coerente com a correção, mas o
    argumento principal é o mecanismo: o processo de teste nunca mais segura o descritor de
    escrita.

## D-089 — Revisão de segurança

- `/security-review` sobre o branch `fase-4-online` contra o `master`, pedido pelo usuário.
  Foram duas etapas: um agente identificou candidatos, e a filtragem de falsos positivos
  descartaria os abaixo de 8/10 de confiança. **Nenhum candidato passou do limiar de 80%**,
  então não houve o que filtrar.
- **Caminhos verificados e por que são seguros:**
  - **Path traversal e raiz absoluta:** `relative_parts` só aceita componentes normais, e a
    raiz é canonicalizada.
  - **Symlinks:** recusados em todo componente, e o lookahead não os segue.
  - **`.gitignore`/`.ignore`:** conferidos por componente, com nome exato.
  - **Ocultos, credenciais e chaves privadas:** recusados; um marcador de chave privada recusa
    o arquivo inteiro.
  - **Entrada do agente:** só o texto da tarefa vai ao provider, por desenho.
  - **Endpoint:** constante, só HTTPS, sem redirect e sem proxy, com a validação de
    certificado padrão do rustls.
  - **Ativação:** só a flag `serve --online`, que é confiável.
  - **Chave:** `SecretString`, header marcado como sensível, erros só com categoria e status,
    `Retry-After` redigido, e só a referência gravada pelo `install`.
  - **Respostas do provider:** o modelo precisa ser o pinado, ids desconhecidos invalidam a
    resposta, e as probabilidades precisam ser finitas em [0, 1]. Só a probabilidade entra no
    envelope.
- **Observações abaixo do limiar:**
  - A corrida entre checar os componentes e ler o arquivo, em `snapshot()`, exige um atacante
    local com escrita concorrente no workspace. É teórica e fica aberta.
  - **Arquivos de ambiente sem ponto** (`prod.env`, `app.env`) não estavam na lista de nomes
    sensíveis e podiam ser enviados se não estivessem no `.gitignore`. Corrigido em TDD: o
    teste de elegibilidade ganhou os dois casos e falhou, com `config/prod.env` lido, antes de
    `env` entrar em `SENSITIVE_EXTENSIONS`. O README lista `*.env`.
- Suítes: 225 verdes no build padrão (2 ignorados) e 238 com `online` (4 ignorados); clippy e
  fmt limpos nas duas.

## D-090 — Links do README para o PRD

- Pedido do usuário: revisar o `README.md` depois que o PRD anterior, a spec do Jev e o
  `merge.md` foram para `spec/old/` (D-057).
- **Conferência:** nenhuma referência apontava para `spec/old/`. Os arquivos citados existem,
  as âncoras internas batem com os títulos, e cada "PRD §x" corresponde à seção de mesmo número
  e assunto no PRD atual (`spec/ripwire-broker-mcp.md`, v0.3). Fora de `spec/`, o repositório
  também não cita caminhos antigos.
- **Mudança:** as sete citações viraram links para as seções do PRD: §7.3, §8.4, §10.3, §23,
  §23.11, §23.15 e a Fase 6 do roadmap. As âncoras seguem a regra de slug do GitHub, com
  acentos mantidos, no mesmo estilo dos links internos do próprio PRD. Um script conferiu cada
  âncora contra os títulos do PRD.

## D-091 — Revisão do repositório e correções de robustez

- Pedido do usuário: revisar o código do repositório e rodar toda a suíte. A suíte já estava
  verde (225 no build padrão, 238 com `online`); a revisão leu os ~9.200 linhas de `src/` e
  apontou cinco defeitos, todos corrigidos em TDD, cada um com o teste vermelho observado antes
  da correção.
- **`markup::parse` entrava em panic** (`src/markup.rs`). O passo do caractere de aspas fazia
  `self.i += 1` para um char que pode ser multibyte ou inexistente, então `rest()` fatiava fora
  dos limites ou no meio de um caractere: `<ctx a=` e `<ctx a=é>` derrubavam a chamada. Como o
  payload vem do stdout do ripwire e todos os chamadores em `normalize` tratam a falha como
  `unparsed_upstream`, um panic ali contradizia o contrato do módulo. Agora, nada depois do `=`
  encerra o elemento com o que já foi lido, e aspas avançam por `len_utf8()`.
- **Valor de atributo sem aspas engolia o resto do documento** (ciclo separado, depois de
  reverter o código escrito sem teste vermelho). `<ctx a=é><sigs>…</sigs></ctx>` perdia o
  `<sigs>` inteiro, porque o `é` era tratado como aspa de abertura e a busca pela aspa de
  fechamento consumia tudo. Um valor sem aspas agora termina no próximo espaço, `/` ou `>`.
- **`gate_notice` deduplicava um vetor não ordenado** (`src/hook.rs`). `Vec::dedup` só remove
  duplicatas adjacentes, e os riscos chegam em ordem de prioridade, não por tipo: dois
  `quality_regression` em volta de um `quality_minor` apareciam como três itens na linha do
  gate. Ordena antes do `dedup`, com `is_sorted()` para evitar o sort no caso comum.
- **`memory_recall` descartava um bloco final sem par** (`src/normalize.rs`). Os blocos são
  consumidos em pares (cabeçalho, corpo); uma resposta cortada no meio deixava o último
  cabeçalho fora, sem qualquer limitação. Um resto não-vazio agora gera `unparsed_upstream`
  dizendo quantos blocos chegaram incompletos — o documento ausente nunca é adivinhado (RF-12).
  Um resto só de espaços não gera limitação, então respostas bem-formadas seguem sem ruído.
- **`NoteEngine::note` esperava por polling** (`src/notes.rs`). O laço acordava a cada 5 ms até
  `summarizer_wait`, o que custava um tick por grupo de notas mesmo quando o modelo já havia
  respondido. Passou a usar um `Notify`. O ponto delicado é a perda de sinal: uma nota que
  assenta entre o `settled()` e o `await` deixaria o waiter dormindo até o timeout, então o
  waiter é registrado com `Notified::enable()` **antes** de ler o estado, e o produtor chama
  `notify_waiters()`. O teste mede tempo virtual com `start_paused = true`: com polling a
  resposta custava 15 ms (três grupos × 5 ms), agora custa zero.
- **`install::plan` entrava em panic com caminho não-UTF-8** (`src/install.rs`). O `json!` do
  `.mcp.json` serializava o `&Path` do binário direto, e `Path: Serialize` falha para bytes
  inválidos; o ramo do Codex fazia conversão lossy e gravaria um caminho corrompido no TOML.
  Os dois caminhos (binário e workspace) passam por uma validação única no topo de `plan()`.
  **Lacuna declarada:** o teste só exercita o caminho do binário, porque o APFS do macOS
  rejeita nomes de arquivo com UTF-8 inválido e o diretório do workspace não pode ser criado
  para o segundo caso.
- Suítes: 231 verdes no build padrão (2 ignorados) e 244 com `online` (4 ignorados) — seis
  testes novos, nenhum teste pré-existente alterado. Clippy e fmt limpos nas duas features.
- **Observações não corrigidas**, deixadas para decisão do usuário: o registro `Inflight`
  (`src/mcp.rs`) vaza uma entrada por `tools/call` que o SDK rejeita antes do handler (um
  `tools/call` sem `name` deixa `tracked_calls` em 1 para sempre, retendo o texto da tarefa em
  memória, contra o que o comentário de D-052 promete); e `NoteEngine::settled` devolve
  `cached: false` para um segundo waiter servido do cache.

## D-092 — Vazamento do `Inflight` e cache hit de nota

Pedido do usuário: corrigir as duas observações que o [D-091](#d-091--revisão-do-repositório-e-correções-de-robustez)
deixou abertas. Ambas em TDD, com o teste vermelho observado antes da correção.

### O registro `Inflight` guardava ids que ninguém reivindicava

- O observer enfileira o id de todo `tools/call` que passa por `on_receive`, e o handler o
  reivindica em `start()`. Quando o SDK responde **antes** de chegar ao handler, o id fica no
  `waiting` para sempre — e a chave carrega o texto da tarefa, contra o que o D-052 promete.
- **Primeira tentativa, descartada:** usar `McpObserver::on_send` para limpar o id quando a
  resposta sai. O gancho existe, mas a leitura do SDK mostrou que ele **não** cobre respostas a
  requests do cliente: `server_runtime.rs` devolve a resposta de `handle_message` e a escreve
  com `transport.send_message` direto, e `on_send` só é chamado em `send()` (mensagens iniciadas
  pelo servidor) e em `try_deliver_notification`. O teste continuou vermelho, então o gancho foi
  removido em vez de ficar como código morto.
- **Medição antes de decidir.** Uma sonda percorreu seis formas malformadas e mostrou que
  vazam exatamente duas, as duas em que a conversão para `CallToolRequestParams` falha:
  `name` ausente ou não-string, e `arguments` que não é objeto. As outras quatro
  (`_meta` inválido, ferramenta desconhecida, tipo errado de argumento) **chegam** ao handler e
  são reivindicadas normalmente.
- **Correção:** o observer aplica o mesmo critério de admissão do SDK e só enfileira o que um
  handler pode receber. Sem suposição de tempo e sem cache com teto — o `start()` continua a
  única via de consumo.
- **Risco residual declarado:** se uma versão futura do SDK passar a rejeitar outra forma antes
  do handler, ela voltaria a vazar. Não há teto de tamanho no registro, porque nenhuma outra
  forma foi reproduzível: escrever um teto sem um teste que falhe seria código sem prova.
- Detalhe observado de passagem: `tracked_calls` conta **chaves**, não ids, então dois ids sob a
  mesma chave apareciam como um. Ficou como está; o campo é de observabilidade.

### Uma nota servida pela geração de outra chamada vinha como `cached: false`

- Há no máximo uma geração em voo (D-036). Quando uma segunda chamada pede a mesma nota, ela
  não gera nada: espera a geração da primeira e lê o resultado do cache. Ainda assim recebia
  `cached: false`, e o `cache_hits` do status não subia.
- O significado de `cached` está fixado pelo teste
  `a_cached_note_is_reused_without_calling_the_model`: verdadeiro quando a chamada **não** pagou
  uma execução do modelo, com `cache_hits` andando junto. Pelo critério, o waiter é um cache hit.
- **Correção:** `note()` registra se foi ela que iniciou a geração, e `settled()` decide o
  `cached` e conta o hit a partir disso. Quem paga o modelo continua com `cached: false`.
- O teste é determinístico com `start_paused = true`: o `sleep` virtual só avança quando toda
  task está parada, o que prova que a segunda chamada está esperando antes de a geração ser
  liberada. Ele também fixa que o modelo é chamado uma única vez para as duas.

- Suítes: 233 verdes no build padrão (2 ignorados) e 246 com `online` (4 ignorados) — dois
  testes novos, nenhum teste pré-existente alterado. Clippy e fmt limpos nas duas features.

## D-093 — Fechamento das ressalvas do `install` e do `Inflight`

Pedido do usuário: fechar as duas ressalvas declaradas no
[D-092](#d-092--vazamento-do-inflight-e-cache-hit-de-nota).

### `install`: o workspace passou a ser validado antes do disco

- A ressalva era que o segundo ponto de chamada do guard de UTF-8 não tinha teste, porque o
  APFS do macOS não permite criar um diretório com nome inválido.
- **A saída não foi um teste só-Linux.** Um teste com `#[cfg(target_os = "linux")]` seria
  compilado fora aqui, ou seja, eu não conseguiria vê-lo falhar — e o guard já existia, então
  não haveria ciclo vermelho nenhum. Em vez disso, mudou a **ordem**: o UTF-8 do
  `args.workspace` é conferido antes do `canonicalize`. Um caminho que uma config de host nunca
  poderia carregar é recusado pelo que ele é, sem depender de o diretório existir.
- Agora o teste roda em qualquer plataforma e foi observado vermelho: antes dizia
  `workspace /tmp/rip\xffwire-workspace: No such file or directory`, não "UTF-8".
- A conferência depois do `canonicalize` continua, porque resolver o caminho pode trazer bytes
  que o argumento não tinha, por um symlink para um diretório com nome inválido. Esse ramo
  segue inalcançável no macOS, mas agora é defesa em profundidade atrás de um guard testado, e
  não o único guard.

### `Inflight`: teto com remoção do mais antigo

- A ressalva era a ausência de teto: o D-092 fechou as duas formas reproduzíveis, mas uma
  versão futura do SDK que responda outra forma antes do handler voltaria a vazar.
- **O teste exigiu um desvio de convenção, registrado aqui.** O repositório não tinha nenhum
  teste inline em `src/`: tudo é testado pelas costuras públicas. O teto, porém, só dispara em
  vazamentos que o guard do D-092 tornou irreproduzíveis, e `received()` é chamado no mesmo task
  que reivindica o id, então nenhuma costura pública consegue encher o `waiting`. A escolha foi
  entre um `#[cfg(test)] mod tests` em `src/mcp.rs` e código de produção sem prova; ficou o
  teste inline. Se a convenção importa mais que a prova, é reverter o teto.
- `MAX_WAITING = 256`, com `waiting` guardando `(ordem, id)` para saber qual é o mais antigo.
- Dois testes: o limite (3.000 ids enfileirados ficam em 256 — observado vermelho em 3.000) e a
  **ordem** da remoção. O segundo passou de primeira, o que não prova nada, então a
  implementação foi temporariamente invertida para remover o mais novo: o teste falhou, e
  voltou a passar com a remoção correta. Sem essa inversão ele seria um teste que nunca se
  provou capaz de pegar o defeito.
- Fica como está, já registrado no D-092: `tracked_calls` conta chaves, não ids. É campo de
  observabilidade, e mudá-lo não faz parte destas ressalvas.

- Suítes: 236 verdes no build padrão (2 ignorados) e 249 com `online` (4 ignorados) — três
  testes novos, nenhum pré-existente alterado. Clippy e fmt limpos nas duas features.

## D-094 — Reversão do teto do `Inflight`

- Decisão do usuário, depois de o [D-093](#d-093--fechamento-das-ressalvas-do-install-e-do-inflight)
  apresentar a escolha: **manter a convenção** de testar só pelas costuras públicas e reverter o
  teto. O primeiro (e único) `#[cfg(test)] mod tests` de `src/` saiu junto com ele.
- Revertido: `MAX_WAITING`, a função `trim`, o contador `arrivals` e o par `(ordem, id)` nas
  filas. O `waiting` volta a ser `HashMap<String, VecDeque<String>>`, e `start` e `cancel`
  voltam à forma anterior. O `src/mcp.rs` ficou byte a byte igual ao estado do
  [D-092](#d-092--vazamento-do-inflight-e-cache-hit-de-nota): 111 linhas fora, as 7 originais de
  volta.
- **Não revertido:** o guard de admissão do D-092 e a reordenação do `install` (a outra metade
  do D-093). Ambos conferidos no lugar depois da reversão. Os testes deles continuam:
  `a_rejected_tools_call_is_not_tracked_forever` e
  `install_refuses_a_workspace_path_that_is_not_utf8`.
- **Risco residual, de novo aberto e registrado aqui para não se perder:** o registro não tem
  teto. As duas formas que vazavam estão fechadas pelo guard do D-092, e um teste de regressão
  as cobre; mas se uma versão futura do `rust-mcp-sdk` passar a responder outra forma de request
  antes de chegar ao handler, os ids voltam a acumular pela vida do processo, com o texto da
  tarefa nas chaves. O sinal de alarme é o `tracked_calls` do recurso de status crescer e não
  voltar a zero.
- A razão de não haver teto não é técnica, é de processo: ele só dispara em vazamentos que o
  guard tornou irreproduzíveis, e `received()` roda no mesmo task que reivindica o id, então
  nenhuma costura pública consegue encher o `waiting`. Sem teste possível pelas costuras
  públicas, a escolha ficou entre quebrar a convenção e não ter o teto — e a convenção venceu.
- Suítes: 234 verdes no build padrão (2 ignorados) e 247 com `online` (4 ignorados) — os dois
  testes inline saíram, nenhum outro teste alterado. Clippy e fmt limpos nas duas features.

## D-095 — Prompt de testes de propriedade e CI/CD

Pedido do usuário, em duas partes: ler o código para preencher as lacunas de
`spec/prompt/ci-cd.md` e traduzi-lo; depois auditar o prompt quanto a segurança e práticas de
DevOps.

### Procedência do arquivo

- `spec/prompt/` **nunca foi versionado** — `git log` desse caminho é vazio. O prompt original,
  em inglês, 57 linhas, foi escrito sem acesso ao `src/` e marcava suas lacunas como `inferred`,
  `UNKNOWN` e `SPECULATING`. Ele foi sobrescrito, e por decisão do usuário o backup foi apagado,
  então **não é recuperável**. O que ele afirmava está resumido abaixo para o registro não se
  perder.
- O original supunha: papel do sistema inferido das dependências; `Targets: bin and/or lib —
  UNKNOWN`; "não invente MSRV, proponha um `rust-toolchain.toml`"; uma tabela P0/P1 marcada
  `SPECULATING` com cinco candidatos; e uma forma de CI exigida como se de zero.

### Lacunas preenchidas

- **Alvos:** `lib` + `bin` + 2 examples + 12 alvos de teste, via `cargo metadata`.
- **MSRV:** já existe. `rust-toolchain.toml` fixa o canal `1.98.1` com `clippy` e `rustfmt`; não
  havia o que propor.
- **Papel:** três ferramentas, o recurso de status, e o caminho
  `mcp → broker → router → upstream → normalize → dedup → budget`.
- **Invariantes com os números do código**, não do PRD: pisos de orçamento 256/512, limiares
  estritos 0,25/0,50, os quatro limites de requisição, os seis do `reader`, as 12 variantes de
  `Ineligible`, `GATE_RISKS`, `RECENT_REQUESTS`.
- **Cobertura atual**, para o PBT somar em vez de repetir: 234 verdes no default (2 ignorados) e
  247 com `online` (4 ignorados), com a tabela por alvo fechando exatamente com o `cargo`, os três
  motivos de `#[ignore]` e os auxiliares já disponíveis em `tests/common/`.

### As duas correções que mudam o plano

1. **Os melhores alvos de PBT são privados.** `normalize`, `router`, `markup`, `budget` e `dedup`
   são `mod`, não `pub mod`. Isso invalida o item P0 #3 do original ("idempotence of
   `normalize()`") como estava escrito, e atinge justamente `markup::parse`, que lê entrada de fora
   do processo e já teve dois panics ([D-091](#d-091--revisão-do-repositório-e-correções-de-robustez)).
   As três saídas ficaram documentadas com o custo de cada uma, inclusive o aviso de que o teste
   inline quebra a convenção que o [D-094](#d-094--reversão-do-teto-do-inflight) acabou de preservar.
2. **O original pôs o `online` como "P1 IF online code exists", atrás da feature. Está invertido.**
   Só `credential` e `jev` são `#[cfg(feature = "online")]`; `decision`, `response`, `request`,
   `reader`, `cache`, `redact`, `retry_after`, `prompt`, `scheduler` e `metrics` compilam no build
   **default**, são `pub` e são puros. É a superfície mais rica do crate, roda no job default e não
   fere o CA-10. Sete dos doze P0 vêm dela.

### Método de verificação

Nada foi afirmado por leitura casual. As constantes foram conferidas por `grep` contra `src/`;
cada caminho que o prompt chama de "alcançável" passou por uma sonda temporária que compila
`use`/referência de fora do crate; e as afirmações de privacidade passaram pela sonda inversa,
que tem de falhar a compilação. As sondas foram removidas.

### Auditoria de segurança

- Quatro P0 foram reclassificados como **controles de segurança**, não higiene: P0.1/P0.2 fuzzam
  uma fronteira (traversal e elegibilidade, RF-02/CA-08); P0.12/P0.5 são **disponibilidade**, já
  que panic com entrada de fora do processo é negação de serviço num servidor de vida longa — o que
  reenquadra "não entra em panic", que o original listava como asserção proibida por ser fraca; e
  P0.7 é confidencialidade.
- **P0.13, novo:** texto de repositório não pode fechar o bloco que o embrulha (injeção de prompt,
  D-052). A viabilidade foi provada antes de propor: um `Envelope` hostil, com
  `</ripwire-broker-context>` dentro do `untrusted_repository_data`, é construível de fora do crate,
  e o escape `\u003c` o contém.
- **A segurança do próprio suíte** entrou no prompt, e não estava lá: propriedades que geram
  caminhos fuzzam um guarda de traversal, então raiz sempre em tempdir, nenhuma remoção com caminho
  gerado, nada de executar conteúdo gerado, tamanhos limitados (com `MAX_READ_BYTES` de 8 MiB,
  estratégia sem teto estoura o runner) e revisão de `.proptest-regressions/`, que é entrada
  versionada.
- **Fixtures:** 21 arquivos, incluindo a gravação real `jev/live_v1.json`, que já se limita a
  digests, status, forma e probabilidades, a partir de corpus sintético. Nenhuma contém hoje string
  com cara de credencial — conferido. Uma gravação futura caindo direto do provider é o caminho
  mais provável de um segredo entrar no repositório, daí a guarda de CI proposta.

### Auditoria de DevOps

O workflow não tem **nenhum** controle básico. Tudo conferido, não suposto: `permissions:` ausente
(herda o padrão, que pode ser `write-all`); `actions/checkout@v4` em tag mutável;
`persist-credentials: false` ausente; **`--locked` ausente apesar de o `Cargo.lock` estar
versionado**, ou seja, o CI pode resolver versões diferentes das testadas; `timeout-minutes`
ausente, num suíte que inicia subprocessos e tem um ripwire falso que dorme 30 s; `concurrency`
ausente; `deny.toml` e `.github/dependabot.yml` inexistentes; e `.env`/`.envrc` fora do
`.gitignore`, embora o produto trate `*.env` como nome sensível ([D-089](#d-089--revisão-de-segurança)).

### Quatro controles gratuitos, verificados empiricamente

1. `#![forbid(unsafe_code)]` — o crate tem **zero** `unsafe`.
2. `#![deny(clippy::print_stdout, clippy::dbg_macro)]` em `src/lib.rs`. Em `serve` o stdout carrega
   o protocolo MCP, então um `println!` na biblioteca corrompe a sessão; fora do `main.rs` só
   existe `eprintln!`, e como `main.rs` é outro crate root os `println!` legítimos dos comandos de
   um disparo não são afetados. `clippy --all-targets --features online` passa com os dois ligados.
3. **CA-10 como porta de CI:** `cargo tree -e normal` traz 0 ocorrências de
   `reqwest`/`secrecy`/`rustls`/`hyper` no default e 2 com `--features online`.
4. Guarda de CI para manter as fixtures sintéticas.

### Recomendações fora do workflow

Proteção de branch: os dois jobs como checks obrigatórios e revisão exigida antes do merge. O
histórico recente tem PR mesclado sem revisão, inclusive os quatro desta sessão. Se a intenção é
manter assim, a decisão deve ser registrada em vez de ficar implícita.

### Correção de registro

As horas de D-092, D-093 e D-094 na tabela de índice tinham sido **aproximadas** em vez de lidas do
relógio, e ficaram no futuro (17:40, 18:25, 18:55). Foram corrigidas para as horas reais dos
commits correspondentes (16:34, 16:45, 16:52), o que restaura a ordem monotônica da tabela.

Nenhum código foi alterado: 247 testes verdes com `online`, `fmt` limpo.

## D-096 — Gargalos de arquitetura medidos, e os dois primeiros corrigidos

Pedido do usuário: revisar a arquitetura, achar gargalos, propor mitigação e ordenar por
impacto; depois atacar os itens 1, 2 e 5 da lista.

### Como a lista foi feita

Cada item do topo foi **medido**, não suposto, com sondas temporárias removidas depois. Os dez
itens, em ordem de impacto: (1) o walk do `ignore` por componente em `snapshot`; (2) `is_fresh`
relendo e re-hasheando o arquivo inteiro; (3) custo super-linear do shaping do envelope;
(4) um processo ripwire novo por evento de hook; (5) chamadas upstream independentes em série;
(6) `ps` a 5 Hz no watcher; (7) `dedup` copiando o corpo de cada item; (8) três caches sem teto;
(9) `status()` sondando o upstream a cada leitura; (10) varreduras O(n·m) no merge.

Medições que orientaram as escolhas: construir um `ignore::Walk` custa ~109 µs contra 19 µs de um
`read_dir`, e o número de irmãos no diretório quase não influi — ou seja, o custo era
`profundidade × 109 µs`, não leitura de diretório. E dobrar as entradas do envelope custava
2,4–2,97x o tempo, confirmando o item 3 como super-linear, embora com o orçamento padrão de 2500
tokens ele fique em menos de 1 ms.

### Item 1 — uma travessia do `ignore` por `snapshot`, não uma por componente

- `listed(dir, name)` construía um `WalkBuilder` por componente do caminho. Virou
  `admitted_prefixes(root, parts)`: **uma** travessia a partir da raiz, podada por `filter_entry`
  para descer só ao longo do caminho alvo, devolvendo o conjunto de prefixos admitidos. O laço por
  componente consulta esse conjunto, então **a ordem de precedência dos motivos de
  inelegibilidade não muda** — o que importa, porque o teste de contrato afirma a variante exata.
- Semântica preservada por construção: quem decide continua sendo o `ignore::Walk`, com
  `parents(true)` e o empilhamento de `.gitignore`/`.ignore`; nada foi reimplementado à mão.
- Medido: `snapshot` de 332 → 170 µs na profundidade 1, de 609 → 241 µs na 3 e de 1170 → 340 µs na
  5 (**3,4x**). O escalonamento com profundidade ficou quase plano. Por discovery isso roda até 48
  vezes (`max_candidates` 16 + `lookahead_max` 32).
- Rede de segurança: antes de mexer, o teste
  `ineligible_files_are_never_read_for_sending` ganhou três casos de **profundidade 3** —
  `.gitignore` dois níveis abaixo, um segundo `.gitignore` três níveis abaixo, e um diretório
  ignorado no meio do caminho. Ele cobria só profundidade 2, que é justamente a dimensão que a
  mudança mexe. Os casos foram confirmados **verdes no código antigo** antes da otimização: é
  caracterização de refactor, não ciclo vermelho-verde, e vale dizer isso em vez de fingir TDD.

### Item 2 — ficou pela metade, e a outra metade é decisão do usuário

- `is_fresh` caiu de 313 para 182 µs em arquivo pequeno, **de graça**, porque ele chama `snapshot`
  e herdou o ganho do item 1. Para arquivo de 384 KiB o custo é ~993 µs, quase todo leitura e
  sha256.
- O único lever restante é pré-checar `(mtime, size)` e só re-hashear se mudarem — um `stat` custa
  1,3 µs, 240x menos. **Não foi implementado**, porque enfraquece um guarda de segurança em duas
  frentes: perderia uma alteração que preserve tamanho e mtime, e perderia uma mudança de
  elegibilidade (regra de ignore alterada sem tocar o arquivo). O guarda existe para RF-ONLINE-10 /
  CA-ONLINE-11, e a corrida TOCTOU vizinha já ficou registrada como aberta no
  [D-089](#d-089--revisão-de-segurança). Trocar exatidão por 240x de velocidade num controle de
  segurança é decisão do dono do repositório, não do implementador.

### Item 5 — metade entregue, metade revertida por colidir com RF-14

- **Entregue:** em `context_before_finish`, `situational_awareness` e `quality_delta` são
  independentes e agora saem juntas com `tokio::join!`; o `affected` continua depois, porque
  depende dos arquivos alterados. De 66 para 46 ms com RTT de 20 ms. Nenhum teste quebrou.
- **Revertido:** o fan-out dos `edit_check` em `context_after_edit`. Ele levava a chamada de 241
  para 45 ms (**5,3x**), o maior ganho de toda a revisão — e quebrou
  `a_cancelled_call_stops_its_upstream_work_and_is_recorded`, que codifica RF-14
  ([D-049](#d-049--cancelamento-pelo-cliente-e-status-que-não-trava)): depois de um cancelamento,
  nenhuma chamada upstream a mais. Com `join_all`, até `max_edit_checks` chamadas são emitidas de
  uma vez, então mais trabalho já está em voo quando o cancelamento chega.
  - O invariante "cancelar interrompe o trabalho" continua valendo: as chamadas são largadas
    junto com o future. O que muda é **quanto trabalho é desperdiçado ao cancelar** — de no máximo
    1 chamada para até 5.
  - Reescrever o teste para a otimização passar seria trocar um requisito documentado por
    velocidade sem que ninguém decidisse isso. O `impact_needed` sequencial foi mantido pelo mesmo
    motivo, com um comentário no código dizendo **por que** é sequencial, para que ninguém o
    "otimize" sem ver o requisito.
  - Fica em aberto para decisão: aceitar o desperdício em troca de 5,3x, limitar a concorrência a
    2 ou 3 em voo (ganho menor, desperdício menor), ou manter sequencial.
- **Não medido, e por isso não prometido:** se o ripwire real atende requisições concorrentes. O
  binário não está no PATH desta máquina, e o `slow_ripwire` do suíte é serial por construção. O
  que ficou provado é que **o lado do broker sobrepõe** — o ganho de 66→46 ms é real no fake. Contra
  o ripwire real o ganho pode ser menor se ele serializar internamente; medir isso exige o binário.
- `FakeUpstream::latency()` foi adicionado ao suíte para essas medições e fica versionado.

### Nada de correção mudou

234 verdes no build padrão (2 ignorados) e 247 com `online` (4 ignorados), os mesmos de antes: as
três asserções novas entraram num teste existente. Clippy e fmt limpos nas duas features.

---

## D-097 — Teto nos três caches, medido antes de escolher os números

O [D-096](#d-096--gargalos-de-arquitetura-medidos-e-os-dois-primeiros-corrigidos) listou "três
caches sem teto" como item 8. O usuário pediu para medir o crescimento antes de escolher os
valores, e a medição mudou três coisas do plano.

### O que foi medido

Sonda temporária dirigindo 290 chamadas de `context_for_task` com payloads variados por
iteração — um cache que só repete a mesma chave não cresce, e medir isso não diria nada. Base de
RSS tomada na 10ª chamada, depois do aquecimento do alocador. A sonda foi removida.

| | entradas por chamada | após 300 chamadas |
| --- | --- | --- |
| `SemanticCache` | **68** | 20.400 |
| `SessionMemory.seen` | ~6 | 1.809 |
| `NoteEngine.cache` | 1 | 302 |

| configuração | delta de RSS em 290 chamadas | por chamada |
| --- | --- | --- |
| sem `--online` | 1,3 MB | 4,5 KiB |
| com `--online` | 14,4 MB | ~50 KiB |

**O cache semântico é ~90% de todo o crescimento**: 13 MB dos 14,4. Linear, sem patamar.

### O que a medição corrigiu no plano

- **68 entradas por chamada, não 7.** Só 7 requisições ao Jev acontecem por chamada; as
  requisições são em lote, então uma resposta popula dezenas de entradas. O `request_limit: 24`
  por chamada dava falsa segurança: o cache cresce ~10x mais rápido que as requisições.
- **~680 bytes por entrada, não ~200.** A aritmética da struct (`[u8; 32]` + `f64` + digest +
  `SystemTime`) dá ~200 B; o medido é 3,4x isso, por capacidade do `HashMap` dobrando, alocação
  do `request_digest` e arredondamento do alocador. Um teto dimensionado pelo cálculo erraria
  por 3x.
- **Os três não têm peso parecido.** Um domina e dois são ruído: as notas a 1 entrada por
  chamada levam ~500 chamadas para chegar ao teto, e a memória de sessão ~800.
- **Não há guarda nenhum sobre o broker.** `--ripwire-max-rss-mb`
  ([`src/supervise.rs`](../src/supervise.rs)) vigia o **ripwire**, processo filho. O broker, que
  hospeda os três caches e vive o tempo todo da sessão, não tem teto nem vigia.

### Os tetos

| cache | teto | custo no pior caso | histórico que cabe |
| --- | --- | --- | --- |
| `SemanticCache` | 4.000 | ~2,7 MB | ~59 chamadas |
| `SessionMemory.seen` | 5.000 | ~640 KB | ~800 chamadas |
| `NoteEngine.cache` (e `failed`) | 500 | ~350 KB | ~500 chamadas |

**Despejo pelo mais antigo inserido, por fila explícita de ordem, não por `stored_at`.** O
`Cached` já carrega `stored_at` e era o caminho óbvio, mas uma resposta em lote insere dezenas de
entradas dentro da resolução do relógio: os empates fariam o despejo depender de plataforma. A
fila torna "mais antigo" exato e o teste independente de clock. Atualizar uma chave já presente
não é inserção nova e não despeja nada — afirmado em teste.

### A troca que o teto do cache semântico embute

Passadas ~59 chamadas, revisitar o mesmo conteúdo com a mesma pergunta pode virar requisição nova
ao Jev: **custo em dinheiro e um envio a mais de conteúdo do workspace**. Isso toca consentimento
(§23.6), não só memória — teto mais alto protege menos a memória e mais a privacidade. Decisão do
usuário, tomada com os números à vista, não escolhida pelo implementador.

### A consequência que a memória de sessão embute

`seen` é um `BTreeSet<String>` de sha256: **não havia informação de ordem de inserção**. A fila de
ordem entrou como `#[serde(skip)]`, então **o formato do estado que os hooks persistem não muda**.
Uma memória restaurada do disco chega sem ordem, e nessa condição o despejo cai em ordem de
fingerprint até o processo repovoar a fila — documentado no campo. `PartialEq` passou a ser
implementado à mão sobre `seen` apenas, porque a ordem é escrituração de despejo e não identidade.

Esquecer um fingerprint custa uma entrega repetida daquele item, nunca uma resposta errada. E
`restore_session` apara na porta, porque o arquivo pode ter sido escrito por uma sessão mais longa
ou antes de o teto existir.

### O que os testes provam, e o que não provam

Quatro testes novos, cada um vermelho antes da correção (os três tetos não existiam como
constante, então o vermelho foi de compilação):

- `the_semantic_cache_stops_at_its_ceiling_dropping_the_oldest_first`
- `updating_a_cached_decision_evicts_nothing`
- `the_note_cache_stops_at_its_ceiling_dropping_the_oldest_first` — verifica também que a nota
  despejada é **regerada** e que a mais recente segue servindo do cache
- `the_session_memory_stops_at_its_ceiling` — aparo na restauração

Reconfirmado com a sonda de RSS: `cached_decisions` fica **fixo em 4.000** a partir da ~59ª
chamada, contra 20.400 e subindo antes. **O RSS em si não prova patamar** — subiu a 8,7 MB no meio
da corrida e caiu a 2,5 MB no fim, que é comportamento do alocador sob o rodízio de 68
inserções/despejos por chamada. A prova do teto é a contagem de entradas, não o RSS, e não vale
apresentar o segundo como se fosse o primeiro.

### Verificação

238 verdes no build padrão e 251 com `online` (eram 234 e 247; +4 testes). Clippy e fmt limpos nas
duas features. Os testes ficaram no suíte externo, não inline, como
[D-094](#d-094--reversão-do-teto-do-inflight) estabeleceu.

### Aberto

O `failed` do `NoteEngine` ficou com o mesmo teto de 500 das notas, por consistência dentro do
item; ele não foi dimensionado por medição própria, porque suas entradas são removidas na leitura
e só acumulam a falha que ninguém leu. Se isso merecer número próprio, é um ajuste de uma linha.

---

## D-098 — A sonda de disponibilidade do status, reaproveitada por uma janela

Item 9 do [D-096](#d-096--gargalos-de-arquitetura-medidos-e-os-dois-primeiros-corrigidos).
`status()` fazia um `list_tools()` no upstream **a cada leitura** do recurso de status.

### Medido

Sondas temporárias, removidas depois. Dez leituras seguidas de `status()`:

| RTT do upstream | antes | depois | sondas upstream |
| --- | --- | --- | --- |
| 1 ms | 23,0 ms | 2,3 ms | 10 → 1 |
| 5 ms | 64,8 ms | 7,0 ms | 10 → 1 |
| 20 ms | 220,5 ms | 21,8 ms | 10 → 1 |

**O caso caro não é esse.** Um ripwire ocupado não responde à sonda e a leitura só decide `busy`
quando o `STATUS_PROBE` inteiro expira — 1 segundo por leitura:

| | antes | depois | sondas |
| --- | --- | --- | --- |
| upstream ocupado, 3 leituras | 3,01 s | 1,00 s | 3 → 1 |

`busy` é justamente a resposta que custa um segundo para aprender e que menos vale a pena
perguntar duas vezes.

### A janela: `STATUS_CACHE = STATUS_PROBE`

O valor não é arbitrário nem medido — é amarrado a uma constante que já existia. Uma sonda pode
legitimamente levar até `STATUS_PROBE` (1 s) para responder, então **uma resposta da janela nunca
fica mais obsoleta do que uma sonda já tinha permissão de ficar**. A janela não acrescenta
incerteza que o `STATUS_PROBE` não carregasse.

O que isso custa: `available` e `busy` podem estar até 1 s atrasados sobre a realidade. Um ripwire
que morreu há 300 ms ainda é relatado como disponível.

O que **não** fica obsoleto: `restarts` e `last_error` continuam lidos na hora, a cada leitura. Um
reinício aparece no status na mesma hora, mesmo que a disponibilidade esteja na janela.

### Marcado no relógio de quando a resposta ficou conhecida, não de quando foi pedida

A janela conta do fim da sonda. Contando do início, uma sonda que levasse o `STATUS_PROBE` inteiro
entregaria um cache **já expirado**, e o caso ocupado — o mais caro — seria o único a não se
beneficiar.

### O lock nunca é mantido através da sonda

O cache é um `Mutex` travado só para ler e para escrever, nunca durante o `list_tools()`. Um
leitor esperando a ida e volta de outro leitor seria exatamente a paralisação que o recurso de
status não pode ter (RF-13, [D-052](#d-052--code-review-das-fases-2-e-3)). O preço aceito é que
duas leituras simultâneas podem sondar as duas — desperdício limitado a uma sonda, em troca de
nunca bloquear.

### Testes

Dois testes novos, **vermelhos antes por asserção**, não só por compilação: com a constante no
lugar e sem a lógica, as duas leituras custavam 3 sondas onde deviam custar 2.

- `back_to_back_status_reads_share_one_availability_probe` — a segunda leitura reaproveita a
  sonda e relata o mesmo
- `the_availability_probe_is_taken_again_after_its_window` — com `start_paused`, passada a janela
  o status pergunta de novo

O `FakeUpstream` ganhou o contador `probes()` e passou a aplicar `latency` também ao `list_tools`,
que antes não era observável nem atrasável. O `down` do fake foi deixado de fora do `list_tools`
de propósito: mexer nele mudaria o comportamento de testes que usam `.down()` esperando que o
`connect` funcione.

### Verificação

240 verdes no build padrão e 253 com `online` (eram 238 e 251). Clippy e fmt limpos nas duas
features.

---

## D-099 — Teto aplicado no `budget_tokens`, e shaping do envelope em tempo linear

Item 3 do [D-096](#d-096--gargalos-de-arquitetura-medidos-e-os-dois-primeiros-corrigidos).

### Primeiro: o D-096 errou o tamanho deste item

O D-096 escreveu que o custo super-linear do shaping "fica em menos de 1 ms com o orçamento padrão
de 2500 tokens". **Isso foi medido com o fixture de ~11 entradas**, e a conclusão não vale fora
dele. Medido com um pack-task sintético de entradas variáveis:

| entradas | orçamento | antes |
| --- | --- | --- |
| 128 | 2.500 | 61,05 ms |
| 512 | 2.500 | **295,41 ms** |
| 256 | 100.000 | 276,66 ms |
| 512 | 100.000 | **1,10 s** |

295 ms **no orçamento padrão**, não em orçamento grande. O item estava mal dimensionado na lista,
e por um erro meu de generalizar de um fixture pequeno.

### O diagnóstico, instrumentado

Contador temporário de serializações e de bytes serializados dentro de `estimate_tokens`:

| entradas | orçamento | serializações | bytes serializados |
| --- | --- | --- | --- |
| 512 | 2.500 | 996 | 10,1 MB |
| 512 | 100.000 | 514 | **41,5 MB** |

O custo **são os bytes**, não o número de chamadas: cada uma das ~n serializações cobre um
envelope cada vez maior. O tempo acompanha os bytes exatamente.

E o número de entradas não é limitado pelo broker — `normalize` não tem teto — e o broker pede ao
ripwire com o `budget_tokens` do cliente. Ou seja: **as entradas seguem o orçamento**, e o custo
é quadrático no orçamento.

### O teto: 100.000, que já estava declarado e não era imposto

`check_budget` só validava o mínimo. O número do teto não foi inventado: o schema MCP declara
`"maximum": 100000` desde que os schemas foram escritos ([`src/mcp.rs`](../src/mcp.rs)) — só não
havia nada que o aplicasse. O literal aparecia **em um único lugar do código e em nenhum teste ou
spec**. Um cliente chamando fora do schema, ou a CLI e os hooks, que nunca o veem, podiam pedir
qualquer `u32`.

Agora `check_budget_at` recusa acima de `MAX_BUDGET_TOKENS`, com `invalid_input`, antes de
qualquer trabalho upstream, do mesmo jeito que o mínimo já era recusado. O `src/mcp.rs` publica a
constante em vez de repetir o literal, então o schema e a validação não podem divergir.

### A contagem incremental

`over(env)` serializava o **envelope inteiro** a cada `push` de candidato. O laço do `fill` passou
a carregar o comprimento em bytes e a somar só o que cada entrada acrescenta.

**Por que é exato, e não estimativa:** `items`, `tests`, `risks` e `limitations` nunca têm
`skip_serializing_if`, então a forma é estável — um array vazio é `[]`, e `n` elementos custam
`soma + n - 1` entre os colchetes. Acrescentar um elemento custa o próprio elemento mais a vírgula
quando não é o primeiro. Nenhum outro campo muda durante o laço, e o `finish` recalcula do zero
depois. As decisões de corte são as mesmas, byte a byte.

**O que ficou intocado de propósito:** o `finish` e o `add_notes` continuam com a serialização
cheia. Lá o número de serializações já é pequeno no caso comum, e a escrituração dos campos de
`budget` — `next_step` é uma string que carrega números — mudaria o comprimento de formas que não
vale a pena rastrear à mão. Otimizar onde não havia problema só acrescentaria risco.

**Um efeito colateral bom:** nada é mais empurrado especulativamente, então o item não precisa mais
ser clonado para guardar uma versão sem corpo. O clone por item saiu junto.

**Um caso de borda preservado:** um valor que não serializa (um `f64` não-finito em
`SemanticEvidence.probability`) é tratado como "não cabe" e o item é descartado — exatamente o que
o `estimate_tokens` já fazia respondendo `u32::MAX`.

### Depois

| entradas | orçamento | antes | depois | fator |
| --- | --- | --- | --- | --- |
| 128 | 2.500 | 61,05 ms | 5,84 ms | 10,5x |
| 512 | 2.500 | 295,41 ms | 18,52 ms | **16x** |
| 128 | 100.000 | 72,02 ms | 4,72 ms | 15,3x |
| 256 | 100.000 | 276,66 ms | 10,08 ms | 27,5x |
| 512 | 100.000 | 1,10 s | 22,81 ms | **48x** |

O escalonamento virou quase linear: dobrar de 256 para 512 entradas no teto custa 2,26x, contra
3,97x antes.

### Os testes

**Vermelho de verdade, para o teto:**

- `a_budget_beyond_the_declared_maximum_is_refused` — recusa com `invalid_input`, a mensagem nomeia
  o teto, nenhuma chamada upstream acontece, e o teto **em si** é aceito (é o máximo declarado, não
  um acima dele)
- `every_tool_enforces_the_budget_ceiling` — `context_after_edit` e `context_before_finish` também

**Caracterização, para o refactor** — e vale dizer que é isso, não TDD:

- `the_shaped_envelope_never_exceeds_its_budget_and_grows_with_it` varre 288 orçamentos de 256 a
  1400 sobre 24 entradas e afirma, para cada um, que o envelope não passa do orçamento pedido, que
  `shown` nunca cai quando o orçamento cresce, e que toda entrada está ou mostrada ou contada como
  omitida. Mais **seis valores-ouro** de `shown` colhidos da implementação antiga: se a aritmética
  de bytes desviar um único byte, uma fronteira se move e um deles muda.
- Confirmado **verde no código antigo** antes do refactor. De quebra, o próprio teste caiu de
  1,02 s para 0,26 s, que é o ganho aparecendo no suíte.

### Verificação

243 verdes no build padrão e 256 com `online` (eram 240 e 253; +3 testes). Clippy e fmt limpos nas
duas features.

---

## D-100 — Itens 7 e 10 medidos e recusados, e os números do D-096 ao D-099 refeitos em release

Últimos dois itens com ganho suposto da revisão do
[D-096](#d-096--gargalos-de-arquitetura-medidos-e-os-dois-primeiros-corrigidos): (7) o `dedup`
copiando o corpo de cada item, (10) as varreduras do `merge`. **Nenhum dos dois foi
implementado**, e a medição é a razão.

### Item 10 — a descrição no D-096 estava errada, e a mitigação era mais lenta

O D-096 chamou isso de "varreduras O(n·m) no merge". O `m` é o número de arquivos da discovery, e
ele é **estruturalmente limitado**: `max_candidates` 16 mais `lookahead_max` 32, no máximo 48
([`src/online/coordinator.rs`](../src/online/coordinator.rs)). Então o custo é linear nas
entradas, com constante 48 — não quadrático.

Medido em release, o `merge` inteiro:

| entradas | merge |
| --- | --- |
| 32 | 4,46 µs |
| 128 | 7,29 µs |
| 512 | **14,67 µs** |

Catorze microssegundos. E a mitigação que eu tinha proposto — um `HashMap` por caminho construído
uma vez — foi **3x mais lenta** (372 µs contra 124 µs, em debug): as duas varreduras rodam 48 vezes
sobre comparações baratas e não alocam nada, enquanto o índice aloca um `Vec` por caminho. A
"otimização" era uma pessimização, e só a medição mostrou isso.

### Item 7 — a premissa estava errada

O D-096 disse que o problema era o `dedup` **copiar** o corpo de cada item. A medição desmente:
com 2048 entradas o `dedup` constrói 4,26 MB de chaves, e copiar 4,26 MB custa cerca de 1 ms
contra os 90 ms (debug) medidos. **O custo é alocação e hashing**, não a cópia — cada chave é
`format!`-ada e depois passa por `contains` e por `insert`, ou seja o corpo é hasheado duas vezes.

A mitigação proposta era trocar o corpo por um sha256. Medida em release, contra a
implementação atual:

| entradas | corpo | atual | com sha256 |
| --- | --- | --- | --- |
| 64 | 200 B | 47,21 µs | 107,29 µs |
| 512 | 200 B | 392,29 µs | 590,21 µs |
| 512 | 2 KB | 1,10 ms | 1,53 ms |
| 2048 | 2 KB | 4,51 ms | 4,41 ms |

**Pior em todo tamanho realista**, empatando só no maior. Um sha256 sobre o corpo custa mais que
dois SipHash sobre ele, que é o que o `HashSet` já faz. Em debug o sha256 parecia 2,5x melhor —
artefato de build não otimizado, e foi por isso que a decisão exigiu release.

Trocar igualdade exata por igualdade de digest numa função que decide o que entra no envelope, sem
ganho, seria risco de graça. Fica como está.

### A correção que importa mais: debug contra release

**Todos os números do D-096 ao D-099 foram medidos em build debug**, e os absolutos que publiquei
são inflados. Refeitos em release:

`snapshot` ([D-096](#d-096--gargalos-de-arquitetura-medidos-e-os-dois-primeiros-corrigidos)):

| profundidade | antigo | novo | fator em release | fator publicado |
| --- | --- | --- | --- | --- |
| 1 | 165,64 µs | 141,16 µs | 1,17x | — |
| 3 | 437,25 µs | 241,97 µs | 1,81x | — |
| 5 | 765,73 µs | 296,91 µs | **2,58x** | 3,4x |

Shaping do envelope ([D-099](#d-099--teto-aplicado-no-budget_tokens-e-shaping-do-envelope-em-tempo-linear)):

| entradas | orçamento | antigo | novo | fator |
| --- | --- | --- | --- | --- |
| 128 | 2.500 | 3,62 ms | 1,01 ms | 3,6x |
| 512 | 2.500 | **14,40 ms** | 1,95 ms | 7,4x |
| 128 | 100.000 | 2,93 ms | 378,04 µs | 7,8x |
| 512 | 100.000 | **31,82 ms** | 2,04 ms | 15,6x |

O D-099 publicou 295 ms e 1,10 s para essas duas últimas linhas. Em release são **14,40 ms e
31,82 ms** — cerca de 20x menos. A correção do D-099 continua sendo um ganho real de 7 a 16x, e o
teto do `budget_tokens` continua justificado por ser entrada controlada pelo chamador, mas a
urgência que os números sugeriam estava exagerada. O `shown` é idêntico entre antigo e novo nas
quatro linhas, o que reconfirma que as decisões de corte não mudaram.

### O padrão por trás disso, que vale para a próxima medição

Trabalho **ligado a I/O** quase não muda entre debug e release: o `snapshot` é dominado por
syscalls do `ignore`, e ficou 1,2–2,6x em release contra os fatores de debug. O
[D-098](#d-098--a-sonda-de-disponibilidade-do-status-reaproveitada-por-uma-janela) não precisa de
correção pelo mesmo motivo: o ganho lá são idas e voltas evitadas (10 para 1, 3 para 1), e os
tempos eram dominados pelo RTT injetado.

Trabalho **ligado a CPU** infla de 10 a 20x em debug: o shaping e o `dedup`. Foi exatamente onde
eu superestimei, e onde uma decisão marginal (o sha256 do item 7) chegou a **inverter de sinal**
entre os dois builds.

Os tetos do [D-097](#d-097--teto-nos-três-caches-medido-antes-de-escolher-os-números) não precisam
de correção: o que os dimensionou foram **contagens de entradas**, que não dependem do build. O RSS
de lá já estava declarado como não sendo prova de patamar.

### Como fica a lista do D-096

Dos dez itens: 1, 2 (parcial), 3, 5 (parcial), 8 e 9 corrigidos; **7 e 10 recusados por medição**;
6 recusado por análise (o `ps` a 5 Hz é um fork a cada 200 ms num processo cuja única função é
vigiar, e trocá-lo por `/proc` traria divergência de plataforma num caminho de segurança); 4 em
aberto, e é o único que sobra com ganho grande — um processo ripwire novo por evento de hook, cuja
mitigação é um daemon persistente, com superfície nova de IPC e de segurança. Esse merece proposta
em `spec/` antes de qualquer código.

### Verificação

Nenhuma mudança de código nesta entrada. 243 verdes no build padrão e 256 com `online`, os mesmos
do [D-099](#d-099--teto-aplicado-no-budget_tokens-e-shaping-do-envelope-em-tempo-linear). Todas as
sondas e a instrumentação temporária de `dedup`, `merge` e `budget` foram removidas, e a
visibilidade dos módulos `dedup`, `merge` e `budget`, que foi aberta para medir, voltou ao que era.

---

## D-102 — O teste-ouro do D-099 era dependente de plataforma, e deixou o master vermelho

Correção de um defeito que **eu** introduzi no
[D-099](#d-099--teto-aplicado-no-budget_tokens-e-shaping-do-envelope-em-tempo-linear) e que passou
por dois merges antes de aparecer.

### O que quebrou

`the_shaped_envelope_never_exceeds_its_budget_and_grows_with_it` fixava seis valores de `shown`
colhidos nesta máquina. No CI, no orçamento 600, deu 6 onde a tabela dizia 5.

A causa: `provenance.workspace` carrega o caminho do workspace
([`src/broker.rs`](../src/broker.rs)), e ele entra no envelope, logo entra na contagem de bytes.
Um diretório temporário no macOS é `/var/folders/.../T/.tmpXXXXXX`, uns 55 caracteres; no Linux é
`/tmp/.tmpXXXXXX`, uns 15. Os ~40 bytes de diferença são 10 tokens, o bastante para caber um item
a mais e mover toda fronteira de orçamento.

Reproduzido localmente antes de corrigir, com `TMPDIR=/tmp`: a falha é idêntica à do CI,
`budget 600: left: 6, right: 5`. `redact_workspace` não ajuda — ele só afeta o recurso de status,
não o envelope.

### Por que passou por dois merges

O `master` ficou vermelho em `b9e9385` (merge da #10) e `b3a20a5` (merge da #11). O CI de PR das
duas passou porque **o script que conduziu a pilha tratava "nenhum check pendente e nenhum
falhando" como verde** — e quando só o check do CodeRabbit existia e o job `build` ainda não tinha
sido criado, essa condição era verdadeira. Ele mesclou antes de o build rodar. O erro é de processo,
não do repositório, mas fica registrado porque explica dois commits vermelhos no histórico.

### O que os valores-ouro tinham de errado, além da plataforma

Investigando, o defeito é mais fundo que o caminho do tempdir: **aquela tabela não testava
propriedade alguma.** Ela registrava o que saía, folga inclusa. Três tentativas de substituí-la por
uma propriedade real mostraram por quê:

1. **"Pedir exatamente `estimated_tokens` deve mostrar o mesmo"** — não detecta nada. O `finish`
   recalcula `estimated_tokens` com a serialização autoritativa, então o viés se auto-cancela.
2. **"A folga é menor que o menor item"** — falha no código correto. O budgeter entra em ordem de
   prioridade, e o menor item de todos pode já estar dentro.
3. **"A folga é menor que o menor item omitido"** — também falha no código correto, e aqui está o
   achado: **o encaixe não é maximal, por construção.** O `budget::finish` escreve a escrituração
   (`shown`, `omitted` e a frase de `next_step`, que sozinha custa ~23 tokens) **depois** das
   decisões de encaixe, então sobra sempre espaço do tamanho desse bloco.

### O que ficou

O teste afirma o que é verdade e é independente de máquina:

- `estimated_tokens <= budget` — o contrato, garantido pelo `finish`
- `shown` nunca cai quando o orçamento cresce
- `shown + omitted` é o total
- **a folga é menor que o item omitido mais barato mais o bloco de escrituração**, com as duas
  quantidades medidas na própria execução, sem constante registrada

**Sensibilidade medida**, injetando viés no `added()` e verificando que o teste falha: pega desvio
de **3 bytes por entrada ou mais**, e não pega 1 nem 2. Isso está escrito no teste, não implícito.
Um desvio desse tamanho também não pode quebrar o contrato de orçamento: o `finish` remede com a
serialização autoritativa e estoura entradas até caber, então drift custa **uma entrada, nunca
estouro**. Foi essa a checagem que faltou no D-099 — lá eu registrei valores e os vi passar, sem
nunca provar que o teste detectava desvio.

Verificado sob `TMPDIR=/tmp` (curto, como no Linux) e sob um `TMPDIR` de 144 caracteres: passa nos
dois.

### Verificação

243 verdes no build padrão e 256 com `online`, as mesmas contagens — nenhum teste novo, o existente
foi reescrito. Clippy e fmt limpos nas duas features. Só `tests/broker.rs` muda; nenhuma linha de
produção foi tocada, porque o defeito era do teste e não do `budget.rs`.
## D-101 — Proposta para o item 4: o ripwire por evento de hook

Item 4 do [D-096](#d-096--gargalos-de-arquitetura-medidos-e-os-dois-primeiros-corrigidos), o único
que sobrou em aberto depois do
[D-100](#d-100--itens-7-e-10-medidos-e-recusados-e-os-números-do-d-096-ao-d-099-refeitos-em-release).
A proposta está em
[`spec/plan/proposta-ripwire-por-evento-de-hook.md`](plan/proposta-ripwire-por-evento-de-hook.md).
**Nada implementado**: o item envolve superfície de segurança nova e decisão de PRD, então vira
documento antes de virar código.

### Medido

Em release, com um dublê de `ripwire --mcp` em Python que responde na hora, mediana de 15
execuções depois de aquecimento: `user-prompt-submit` 43,0 ms, `post-tool-use` 43,0 ms, `stop`
42,6 ms.

**Os três custam o mesmo, e é o achado.** Com o dublê instantâneo não há trabalho de ripwire na
conta: os 43 ms são overhead fixo de subida, e nada nele depende do evento. Decomposto: 2,6 ms para
subir o `ripwire-broker`, 19,4 ms para `ripwire --version` (um processo inteiro descartado),
~19 ms para `ripwire --mcp` mais o handshake, ~2 ms de broker.

**Não medido, e por isso não afirmado:** o custo do ripwire real, que não está no PATH desta
máquina. Os 19,4 ms de cada subida são startup do **Python**; um ripwire em Rust subiria em poucos
milissegundos, mas indexa a cada subida, o que o dublê não faz. O número real por evento é
desconhecido; o que ficou estabelecido é que ~38 dos 43 ms são as duas subidas de processo.

Uma primeira medição deu 146,9 ms para `user-prompt-submit` contra ~50 ms dos outros. Era artefato
de cache frio — foi o primeiro do laço. Fica registrado porque a diferença de 3x parecia um achado
sobre o evento e não era.

### O fato que reenquadra o item

Um `install` padrão escreve **as duas coisas**: o servidor MCP em `.mcp.json` e os hooks em
`.claude/settings.json`. E o `BrokerServer` guarda `broker: Mutex<Option<Arc<Broker>>>`
([`src/mcp.rs`](../src/mcp.rs)), mantido pela sessão inteira, com ripwire filho vivo e aquecido.

Então já existe um broker de vida longa com ripwire quente, ocioso entre chamadas de tool,
**enquanto cada evento de hook sobe um broker e um ripwire novos ao lado dele**. O item 4 não é
necessariamente "construir um daemon" — é "deixar de duplicar um processo que já está lá".

### Cinco opções, e a que a proposta recomenda

| opção | ganho | superfície nova | risco |
| --- | --- | --- | --- |
| B — não subir um processo só para ler a versão | 19,4 ms/evento no dublê; um processo a menos sempre | nenhuma | baixo |
| E — reduzir o número de eventos | ataca o `N`, que é o que escala | nenhuma | médio, de produto |
| A — não fazer nada | — | nenhuma | zero |
| C — socket para o servidor MCP | quase todo o overhead | socket, autenticação, dois caminhos | **alto, de segurança** |
| D — daemon próprio | igual a C | tudo de C mais ciclo de vida | o mais alto |

A recomendação é **fazer B como item próprio** — um processo a menos por evento, sem superfície
nova — e **medir o ripwire real antes de considerar C ou E**, porque toda a comparação repousa num
número que não tenho.

C e D são tratadas como mudança de PRD, não refactor: exigem autenticação do socket (sem ela,
qualquer processo do usuário pede contexto do workspace, e com `--online` pede **envio remoto**),
escopo por workspace canônico (errar vaza contexto entre repositórios), e ciclo de vida sob
[D-050](#d-050--limite-de-memória-do-ripwire-por-supervisor). E qualquer uma das duas **reabre o
[D-064](#d-064--cache-diagnóstico-e-integração-proposta)**, cuja justificativa para hooks offline é
exatamente que o hook é um processo curto com consentimento por processo.

### Verificação

Nenhuma mudança de código. A sonda de medição e o dublê foram descartados. As âncoras do documento
novo para o changelog e os cinco caminhos de fonte que ele cita foram validados.

---

## D-103 — Uma nota não custa mais um item que já foi evidência

Pedido do usuário depois do [D-102](#d-102--o-teste-ouro-do-d-099-era-dependente-de-plataforma-e-deixou-o-master-vermelho):
revisar os outros testes-ouro que pudessem ter o mesmo defeito.

### A varredura

Em vez de ler teste por teste, o suíte inteiro foi rodado nas duas features variando exatamente o
que quebrou o CI — o comprimento do caminho — com `TMPDIR` de 4 e de 164 caracteres.

| classe | resultado |
| --- | --- |
| dependência de comprimento de caminho | **1 teste** |
| asserções exatas em `shown` / `omitted` / `budget_tokens` | derivam de constantes de configuração ou de fixtures; seguras |
| comprimentos comparados com literal | são limites (`<=`), não valores gravados |
| relógio real | 1 teste, `#[ignore]` e documentado para rodar à mão em release |
| tempo virtual (`start_paused`) | corretos; um afirma `Duration::ZERO`, exato por construção |

Nenhum outro teste-ouro da forma que quebrou no D-102.

### O único achado não era fragilidade de teste

`a_cut_item_never_reaches_the_model` falhava com caminho de workspace acima de ~136 caracteres. O
caminho **não é a causa**: ele só desloca quais orçamentos entram no caso ruim. O defeito existe em
qualquer plataforma, e o teste estava certo.

Reproduzido sem `TMPDIR`, com broker e modelo novos por orçamento:

| orçamento | itens no envelope | notas | item que foi evidência e saiu |
| --- | --- | --- | --- |
| 500 | 3 | 0 | `src/auth.py#login` |
| 600 | 4 | 0 | `docs/auth.md#Authentication decision` |
| 700 | 6 | 0 | `tests/test_auth.py#test_login` |

O mecanismo: `attach_notes` monta os prompts a partir de `included`, que é `env.items` **depois** do
`fill`. Em seguida `budget::add_notes` estoura primeiro as notas e **depois os itens**. Um item cujo
corpo já foi enviado ao modelo local sai do envelope.

E o detalhe que fecha o diagnóstico: nesses casos **todas** as notas foram descartadas e ainda assim
itens foram estourados. Se as notas saem, o envelope volta ao tamanho que já cabia — então o item
não foi sacrificado por uma nota, e sim pela limitação `notes_omitted`, a frase que **diz** que as
notas não couberam.

### A correção proposta tinha um furo, e mudou

A proposta original era "só acrescentar `notes_omitted` se ela couber sem estourar item". Com o
código na frente isso troca o defeito por **silêncio**: descartar a limitação faz o envelope omitir
notas sem dizer, contra o princípio de nunca descartar em silêncio, e contra PRD 10.2 #1, que manda
sempre manter as limitações.

O que os números mostram é que o envelope estoura por **menos de uma limitação**. Então a correção é
**reservar esse espaço antes do encaixe**, não descartar depois.

### `budget::notes_reserve()`

Duas frases são escritas **depois** das decisões de encaixe, e nenhuma é contabilizada enquanto as
entradas são encaixadas:

- `next_step`, que o `finish` escreve quando já sabe quantas entradas foram omitidas;
- a limitação `notes_omitted`, que o `add_notes` escreve mesmo quando nenhuma nota cabe.

É a mesma assimetria registrada no [D-102](#d-102--o-teste-ouro-do-d-099-era-dependente-de-plataforma-e-deixou-o-master-vermelho):
o encaixe não é maximal porque o `finish` escreve escrituração depois. A reserva mede as duas na
**forma mais larga alcançável** — no máximo `MAX_GROUPS` notas, e `next_step` contando entradas que
`MAX_BUDGET_TOKENS` limita — em vez da mais larga representável. Reservar para dígitos inalcançáveis
custaria itens de graça: com `usize::MAX` a reserva dava 75 tokens, com o limite real dá **66**.

A reserva **só se aplica quando há summarizer configurado**. Sem ele o `add_notes` nunca roda, e o
excesso do `next_step` não custa item a ninguém porque nada é acrescentado depois dele.

### O custo, medido

Varredura de 91 orçamentos de 300 a 1200, comparando com e sem summarizer:

| | |
| --- | --- |
| orçamentos sem custo | 73 |
| orçamentos que perdem 1 item | 18 |
| pior caso | 2 itens |

Todos na faixa apertada (310 a 640). A partir de ~700 não há custo, porque todos os itens do
fixture já cabem. **Em troca, nenhum corpo de item vai ao modelo local para depois ser descartado do
envelope.**

### Testes

Um teste novo, **vermelho por asserção antes** da correção:
`a_note_never_costs_an_item_that_was_already_evidence` varre sete orçamentos, com broker e modelo
novos em cada um para que os prompts não acumulem, e afirma que nenhuma referência que apareceu como
evidência num prompt está ausente do envelope. Falhava no orçamento 500 com `src/auth.py#login`.

O `a_cut_item_never_reaches_the_model`, que era o teste que denunciou o defeito, **passa agora
também com `TMPDIR` de 164 caracteres** — a correção curou a causa, não o sintoma.

### Verificação

244 verdes no build padrão e 257 com `online` (eram 243 e 256; +1 teste). Suíte inteiro verde nas
duas features sob `TMPDIR` de 4 e de 164 caracteres. Clippy e fmt limpos. Sondas removidas e a
visibilidade de `budget`, aberta para medir, devolvida.

---

## D-104 — Decisão dos itens 2 e 5: recusado e feito

As duas decisões que o [D-096](#d-096--gargalos-de-arquitetura-medidos-e-os-dois-primeiros-corrigidos)
deixou em aberto foram delegadas ao implementador. Ambas foram relidas no código antes de decidir,
não a partir do resumo de quem as propôs.

### Item 2 — recusado

O lever era pré-checar `(mtime, size)` e só re-hashear se mudassem: um `stat` custa 1,3 µs contra
~993 µs de leitura e sha256 num arquivo de 384 KiB, 240x menos.

**A releitura do código decide o caso.** `is_fresh` ([`src/online/reader.rs`](../src/online/reader.rs))
não compara conteúdo: ele chama `snapshot()`, que faz **a travessia de elegibilidade inteira** e
depois o hash. Um precheck por `stat` curto-circuitaria as duas coisas.

O que se perderia não é precisão de hash, é **elegibilidade**. Um `.gitignore` editado para excluir
o arquivo não muda o `mtime` nem o tamanho **do arquivo**: o precheck diria "fresco" e o broker
enviaria ao provider remoto um arquivo que as regras de ignore agora excluem. Elegibilidade é a
fronteira de consentimento (§23.6) — isso é um furo no consentimento, não uma troca de exatidão por
velocidade.

E o custo que se economizaria não está no caminho crítico: o guarda roda no máximo 48 vezes por
discovery (`max_candidates` 16 + `lookahead_max` 32), a ~182 µs cada, contra a ida e volta HTTP ao
Jev que ele protege, que custa ordens de magnitude mais. Otimizar 240x no lado barato de uma chamada
de rede é otimizar o lado errado.

**Nenhum código mudou por este item.**

### Item 5 — feito, e o número do D-096 estava errado

O D-096 reportou que o fan-out dos `edit_check` levava `context_after_edit` de 241 para 45 ms, 5,3x.
**Medido em release, o fan-out só dos checks dá 1,6x**, e a aritmética explica por quê: com 5
símbolos a chamada são 11 idas e voltas — 1 de `situational_awareness`, 5 de `edit_check`, 5 de
`impact` — e mexer só nos checks a deixa em 7. O laço de `impact` continuava sequencial e passava a
dominar. O 5,3x publicado no D-096 não se sustenta.

Se a leitura do RF-14 permite chamadas concorrentes, ela vale igual para o `impact`, que também é
independente entre símbolos e limitado pelo mesmo `max_edit_checks` — um símbolo só chega lá se o
próprio check dele voltou `changed`. Com os dois laços em fan-out a chamada são **3 idas e voltas,
independentemente do número de símbolos**:

| RTT | símbolos | sequencial | só os checks | os dois | fator |
| --- | --- | --- | --- | --- | --- |
| 5 ms | 5 | 74,9 ms | 47,4 ms | **18,9 ms** | 4,0x |
| 20 ms | 5 | 240,4 ms | 150,6 ms | **65,8 ms** | 3,7x |
| 50 ms | 5 | 569,1 ms | 362,6 ms | **156,6 ms** | 3,6x |

O custo ficou plano: 18,9 ms com 5 símbolos contra 20,5 ms com 2. Medido em release, e o ganho é
ligado a RTT, então não depende do build ([D-100](#d-100--itens-7-e-10-medidos-e-recusados-e-os-números-do-d-096-ao-d-099-refeitos-em-release)).
Isso importa porque `context_after_edit` é o caminho do `PostToolUse`, que dispara **por edição** —
a dimensão que escala num turno ([D-101](#d-101--proposta-para-o-item-4-o-ripwire-por-evento-de-hook)).

### O que o RF-14 exige, e o que o meu comentário dizia que exigia

O comentário que eu havia deixado em `src/broker.rs` afirmava que "RF-14 (D-049) exige que um
cancelamento não deixe trabalho upstream a mais em voo". **O [D-049](#d-049--cancelamento-pelo-cliente-e-status-que-não-trava)
não diz isso.** Ele trata de o cancelamento ser respeitado: o future é descartado por um
`tokio::select!`, a tool registra `outcome: "cancelled"` com as chamadas upstream já concluídas, e
nada novo é pedido depois. Nada ali limita quanto já estava em voo.

Com o fan-out, as chamadas saem **antes** do cancelamento, nunca depois. O RF-14 continua valendo.
O que muda é o **desperdício** quando um cancelamento chega no meio: até `max_edit_checks` (5)
respostas que ninguém lê, em vez de uma. São chamadas ao ripwire na mesma máquina, então o
desperdício é CPU local — **nunca uma requisição remota**, porque a etapa semântica não passa por
aqui ([D-060](#d-060--gate-por-rota-proposta)).

### O teste, e um limite dele que vale dizer

`a_cancelled_call_stops_its_upstream_work_and_is_recorded` afirmava a **lista exata** de chamadas,
`["situational_awareness", "edit_check"]`, o que só valia porque os checks corriam um por vez. Passou
a comparar o que estava em voo no instante do cancelamento com o que existe depois dele — o
requisito de verdade — mais um limite: os checks em voo são no máximo os símbolos pedidos.

Foi confirmado **verde no código sequencial** antes do fan-out, como caracterização.

**O limite honesto:** neste teste o `edit_check` está retido, então o código sequencial também não
emitiria mais chamadas de qualquer forma. A asserção "nada depois do cancelamento" tem pouca força
aqui; os dentes deste teste estão no `outcome: "cancelled"`, nas chamadas upstream registradas e no
"o broker continua servindo". Reescrever a asserção para o requisito é mais fiel que a lista exata,
mas não a torna forte, e vale dizer isso em vez de deixar parecer que torna.

### `futures-util`

Declarada como dependência direta. Ela **já estava** no grafo do build padrão, trazida pelo
`rust-mcp-sdk` via `futures`, então declarar não acrescenta nada à árvore. O CA-10 foi rodado
explicitamente: `the_build_has_no_network_stack` passa.

### Verificação

244 verdes no build padrão e 257 com `online`, as mesmas contagens — nenhum teste novo, um
reescrito. Clippy e fmt limpos nas duas features. Sonda de medição removida.

---

## D-105 — A versão do ripwire deixa de custar um processo por evento de hook

Item 4 do [D-096](#d-096--gargalos-de-arquitetura-medidos-e-os-dois-primeiros-corrigidos), fechado
na **opção B** da proposta em
[`spec/plan/proposta-ripwire-por-evento-de-hook.md`](plan/proposta-ripwire-por-evento-de-hook.md),
por escolha do usuário. O ripwire foi instalado em `~/.local/bin` (versão **0.6.5**), o que destravou
a medição que o [D-101](#d-101--proposta-para-o-item-4-o-ripwire-por-evento-de-hook) declarava como
bloqueio — e ela derruba a tese central daquela proposta.

### O ripwire real contra o dublê

O D-101 mediu com um dublê de `ripwire --mcp` em Python, porque o binário não estava no PATH:

| | dublê Python | ripwire real |
| --- | --- | --- |
| `ripwire --version` | 19,4 ms | **3,6 ms** |
| `user-prompt-submit` | 43,0 ms | **19,4 ms** |
| `post-tool-use` | 43,0 ms | **34,0 ms** |
| `stop` | 42,6 ms | **222,0 ms** |

> **Corrigido no [D-106](#d-106--uma-rajada-de-edições-é-uma-pergunta-não-uma-por-edição):** os `34,0 ms` de `post-tool-use` acima são o caminho de retorno antecipado — o placeholder do fixture não foi substituído, então o arquivo editado não resolvia dentro do workspace. O custo real é ~40 ms editando texto e ~99 ms editando código.

O D-101 concluiu: "os três custam o mesmo, e é isso que importa — os 43 ms são overhead fixo de
subida". **Não se sustenta.** Com o ripwire real os três custam coisas muito diferentes e o `stop`
custa 5x o mais barato. O que domina é o **trabalho do ripwire**, não a subida: o gate de conclusão
faz várias chamadas upstream e o ripwire trabalha de verdade em cada uma.

### A rota que a proposta sugeria não existe

A proposta oferecia duas formas para B: ler a versão do handshake MCP que já acontece, ou cachear. A
primeira **não é possível**: o `rust-mcp-sdk` 2.0.0 não expõe o `Implementation` do servidor ao
cliente — `server_details()` existe no lado servidor. É o mesmo obstáculo que o
[D-049](#d-049--cancelamento-pelo-cliente-e-status-que-não-trava) encontrou com o id JSON-RPC.

### O que foi feito

`SessionState` guarda a última leitura com o **stamp do binário** — caminho resolvido, tamanho e
mtime. Cada evento de hook é um processo novo, então sem isso cada um sobe um ripwire inteiro só
para ler `--version` e jogar fora. O campo entra como `#[serde(default)]`, o padrão já usado ali, e
um arquivo de estado anterior a esta entrada continua carregando — verificado.

**Não é hash de conteúdo, e o motivo é aritmético:** hashear vários megabytes custaria mais que os
3,6 ms do processo que se quer evitar.

**Por que `(mtime, size)` aqui e não no `is_fresh`:** no [D-104](#d-104--decisão-dos-itens-2-e-5-recusado-e-feito)
o mesmo atalho foi recusado porque lá ele pularia a reverificação de **elegibilidade**, que é a
fronteira de consentimento (§23.6). Aqui o que se protege é o `check_version`, um guarda de
**compatibilidade** — os fixtures foram gravados de 0.6.4 em diante. Errar custa uma string de
versão obsoleta em `provenance` e uma checagem de compatibilidade obsoleta até a sessão seguinte,
depois de alguém trocar o binário no meio da sessão. Não é a mesma coisa, e a distinção está escrita
no código.

Um binário que não pôde ser stampeado nunca é lembrado: um `ripwire` ainda não instalado é relido na
próxima vez, jamais cacheado como `"unavailable"`.

### O ganho entregue

| evento | antes | depois | ganho |
| --- | --- | --- | --- |
| `user-prompt-submit` | 19,4 ms | 17,0 ms | 2,4 ms (12%) |
| `post-tool-use` | 34,0 ms | 33,0 ms | 1,0 ms (3%) |
| `stop` | 222,0 ms | 218,9 ms | 3,1 ms (1,4%) |

> **Corrigido no [D-106](#d-106--uma-rajada-de-edições-é-uma-pergunta-não-uma-por-edição):** os `34,0 ms` de `post-tool-use` acima são o caminho de retorno antecipado — o placeholder do fixture não foi substituído, então o arquivo editado não resolvia dentro do workspace. O custo real é ~40 ms editando texto e ~99 ms editando código.

A proposta prometia **45% do total**, número que era artefato do startup do Python. O real é de 1,4
a 12%. Vale por remover um processo inteiro por evento, com código pequeno e testado, mas o valor é
modesto e não faz sentido apresentá-lo como mais que isso.

### O que a medição decide sobre as outras opções

- **C (socket para o servidor MCP) e D (daemon próprio): recusadas.** A justificativa das duas era
  que a subida de processo dominava. Ela é ~3 a 5 ms de 17 a 219 ms. Comprariam pouco e custariam
  socket autenticado, escopo por workspace canônico e ciclo de vida sob
  [D-050](#d-050--limite-de-memória-do-ripwire-por-supervisor), além de reabrir o
  [D-064](#d-064--cache-diagnóstico-e-integração-proposta). A medição que era o bloqueio resolveu o
  caso, e resolveu **contra** elas.
- **E (reduzir eventos): a única que sobra com ganho grande.** O `stop` a 219 ms e o `post-tool-use`
  a 33 ms **por edição** são trabalho real do ripwire. Nada mais nesta proposta ataca isso. Continua
  decisão de produto sobre PRD 8.4.

### Efeito colateral da instalação: os testes que se pulavam agora rodam

Os testes com `require_ripwire!` retornavam cedo quando o binário faltava, o que os fazia **passar
vazios**. Com o 0.6.5 no PATH: **zero pulados, todos verdes**. Os fixtures gravados batem com a saída
real do ripwire 0.6.5, o que até aqui era suposição.

### Testes

Dois novos, no seam do binário real (`tests/cli.rs`), com um dublê em Python que conta cada
`--version` — portanto rodam no CI, que não tem ripwire:

- `a_second_hook_event_does_not_ask_ripwire_for_its_version_again` — **vermelho antes**, 2 leituras
  onde deve haver 1.
- `a_swapped_ripwire_is_read_again` — a invalidação. **Provado que tem dentes**: enfraquecendo a
  comparação do stamp para só o caminho, ele falha. Um cache cuja invalidação não é testada erra em
  silêncio.

### Verificação

246 verdes no build padrão e 259 com `online` (eram 244 e 257; +2 testes), com o ripwire real no
PATH e nenhum teste pulado. Clippy e fmt limpos nas duas features. Um arquivo de estado anterior a
esta entrada carrega sem erro.

---

## D-106 — Uma rajada de edições é uma pergunta, não uma por edição

Opção E da proposta do item 4
([`spec/plan/proposta-ripwire-por-evento-de-hook.md`](plan/proposta-ripwire-por-evento-de-hook.md)),
a única que o [D-105](#d-105--a-versão-do-ripwire-deixa-de-custar-um-processo-por-evento-de-hook)
deixou com ganho grande, por escolha do usuário.

### A medição que decidiu a forma

Doze edições de arquivos de código numa sessão, com ripwire 0.6.5 real:

| | |
| --- | --- |
| edições que produziram injeção | **1 de 12** |
| custo por evento | ~99 ms |

O `PostToolUse` **já** faz todo o trabalho — `context_after_edit`, com as chamadas upstream — e só
então chama `has_news` e devolve `Ok(None)` quando nada é novidade. Onze dos doze eventos gastaram
~99 ms de trabalho de ripwire e jogaram o resultado fora.

### E uma correção do D-105

O [D-105](#d-105--a-versão-do-ripwire-deixa-de-custar-um-processo-por-evento-de-hook) publicou
`post-tool-use` a 34 ms. **Era o caminho de retorno antecipado.** O fixture aponta para
`__WORKSPACE__/a.txt` e eu o passei por stdin sem substituir o placeholder, então o arquivo editado
não resolvia dentro do workspace, `files` ficava vazio e o handler retornava antes de qualquer
chamada upstream. Medido de novo com o placeholder substituído: **~40 ms editando um `.txt` e ~99 ms
editando código**. É o segundo caso desta série em que um fixture não substituído me deu um número
errado.

### Por que coalescer por tempo, e não das outras duas formas

A proposta listava três formas para E. As outras duas foram descartadas com razão:

- **"Só edições que mudam elegibilidade"** não é calculável sem fazer a chamada — é a chamada que
  diz se há novidade.
- **"Pular quando o conjunto de arquivos não cresceu"** perde novidade **para sempre**: um arquivo
  editado vinte vezes nunca voltaria a ser relatado, e a vigésima edição pode ter introduzido um
  risco.

A janela por tempo é a única com **perda limitada**: os arquivos nomeados enquanto a janela está
aberta são guardados e viajam com a próxima resposta, então uma novidade atrasa no máximo uma edição
mais a janela — e o `Stop` roda o gate de conclusão sobre a árvore inteira de qualquer forma.

### Como ficou

Na primeira edição a resposta sai normalmente e o instante é marcado. As edições seguintes dentro de
`edit_interval_ms` não perguntam nada: os arquivos entram em `held_edits` no estado da sessão. Passada
a janela, a próxima edição pergunta com os arquivos dela **mais os guardados**, e a lista é limpa.
`MAX_HELD_EDITS` (32) limita o acúmulo; uma rajada maior relata as mais recentes, e o `Stop` cobre o
resto.

`--edit-interval-ms N` configura a janela; **`0` devolve o comportamento anterior**, uma resposta por
edição. O padrão é **1000 ms**, e esse número é juízo, não medição: as edições chegaram a ~100 ms uma
da outra, então 1000 ms coalesce cerca de dez. A cadência real de um agente em uso não é algo que eu
possa medir aqui.

**O relógio é lido no `hook::run`, na fronteira do processo, nunca dentro do `handle`.** O `handle`
recebe `now_ms: Option<u64>` e `None` significa nunca coalescer, então os 37 testes que dirigem o
`handle` direto seguem intactos e nenhum passa a depender de tempo real — a lição do
[D-102](#d-102--o-teste-ouro-do-d-099-era-dependente-de-plataforma-e-deixou-o-master-vermelho).

### Medido depois

| | sem coalescência | com o padrão | |
| --- | --- | --- | --- |
| rajada de 12 edições | 1169 ms | **278 ms** | 4,2x |
| turno inteiro, com `Stop` | 1537 ms | **611 ms** | 2,5x |

### O que a medição não prova

Nas duas corridas da sonda o número de injeções foi **0 em ambas**, então ela **não demonstra** que a
novidade sobrevive à coalescência — 0 para 0 não diz nada sobre isso. O que está verificado é o
mecanismo: um teste afirma que o arquivo guardado durante a janela viaja com a resposta seguinte, e o
`Stop` roda sempre. Apresentar os 4,2x como se também provassem preservação de novidade seria errado.

### A troca, explícita

Isto **muda comportamento observável** e vem ligado por padrão: depois de uma edição, se a anterior
foi respondida há menos de um segundo, o agente não recebe contexto agora. Ele recebe na próxima
resposta, com o arquivo incluído, ou no gate do `Stop`. É matéria de PRD 8.4, e está aqui por decisão
do usuário; `--edit-interval-ms 0` reverte.

### Testes

Dois novos, **vermelhos antes** (o campo `now_ms` não existia):

- `edits_inside_the_window_share_one_upstream_ask` — duas edições 100 ms depois da primeira não são
  respondidas e **não custam chamada upstream nenhuma**.
- `an_edit_past_the_window_is_answered_again_and_carries_what_was_held` — passada a janela a pergunta
  acontece, e o `files` do `situational_awareness` contém o arquivo que tinha sido guardado.

### Verificação

248 verdes no build padrão e 261 com `online` (eram 246 e 259; +2 testes). Clippy e fmt limpos nas
duas features. Os campos novos do estado entram como `#[serde(default)]`, então um arquivo de estado
anterior continua carregando.

---

## D-107 — CI endurecido: dois jobs, e quatro promessas viram portas

Fatia A de [`spec/plan/proposta-pbt-e-ci-cd.md`](plan/proposta-pbt-e-ci-cd.md), que nasce nesta
mesma decisão a partir de [`spec/prompt/ci-cd.md`](prompt/ci-cd.md). Os quatro pontos que a proposta
deixava para aprovação foram aprovados pelo usuário; este registro diz o que cada um virou.

### O ponto de partida

O workflow era um job `build` com quatro passos: `cargo build`, `cargo test`, e os mesmos dois com
`--features online`. **Sem `fmt`, sem `clippy`, sem `permissions`, sem `--locked`, sem timeout, sem
`concurrency`, actions por tag mutável.** E o `master` **não tinha proteção de branch nenhuma** —
`GET /branches/master/protection` devolvia 404. A porta que o D-102 descreve como frágil não era
frágil: não existia.

### Medido antes de exigir

Nada aqui foi ligado na esperança de que passasse. Cada controle foi rodado na árvore primeiro:

| controle | resultado antes de ser exigido |
| --- | --- |
| `forbid(unsafe_code)` + `deny(clippy::print_stdout, clippy::dbg_macro)` | `clippy --all-targets --locked -- -D warnings` limpo nas **duas** features |
| porta do CA-10 (`cargo tree -e normal`) | default: zero de `reqwest`/`secrecy`/`rustls`/`hyper`; com `online`: 18 linhas |
| guarda de fixture sintética | zero acertos em `tests/fixtures/` |
| `--locked` | `Cargo.lock` em sincronia |

### O que o workflow passa a ter

- **Dois jobs, `default` e `online`.** O `default` roda `fmt --all --check`, `clippy --all-targets
  -D warnings`, os testes, a porta do CA-10 e a guarda de fixture. O `online` roda clippy e testes
  com a feature. O build default **continua tendo de passar sem pilha de rede** (CA-10), e agora
  isso é verificado por máquina, complementando o teste `the_build_has_no_network_stack` que já
  existia — que **não** foi duplicado.
- **`permissions: contents: read`** no topo. Sem isso o workflow herda o padrão do
  repositório/organização, que pode ser `write-all`.
- **Actions por SHA completo:** `actions/checkout@3d3c42e` (v7.0.1) e
  `taiki-e/install-action@4cef141` (v2.87.21). Tag é mutável, SHA não.
- **`persist-credentials: false`** no checkout, para o `GITHUB_TOKEN` não ficar no `.git/config`
  ao alcance dos passos seguintes.
- **`--locked` em todo comando cargo.** O `Cargo.lock` é versionado e o CI podia resolver versões
  que ninguém testou.
- **`timeout-minutes: 20` por job.** O suíte sobe subprocessos, tem um ripwire falso que dorme 30 s,
  supervisores de memória e processos vigia; job sem timeout pendura o runner.
- **`concurrency` com `cancel-in-progress`** por ref.
- O gatilho **continua `pull_request`**, nunca `pull_request_target`: PR de fork não recebe segredo.

### `nextest`, com uma ressalva honesta

Os testes passam a rodar por `cargo nextest run`. **O `nextest` não roda doctests** — este crate não
tem nenhum, então nada se perde, mas fica dito para o dia em que tiver. O filtro para os testes de
propriedade (`-E 'test(/^prop_/)'`) está documentado no workflow como comentário e **ainda não é um
passo**, porque `proptest` só entra na fatia C. Apresentá-lo como pronto seria mentira.

### Os quatro pontos aprovados

1. **Visibilidade:** `markup` vai a `pub` com doc de "interno, sem promessa de estabilidade";
   `normalize`, `router`, `budget` e `dedup` ficam privados e são exercitados pela costura pública.
   **Aprovado, mas não implementado aqui** — é a fatia E.
2. **Nomes dos jobs:** `default` e `online`, e a proteção de branch **nesta mesma entrega**
   (adiante). Um comentário no workflow diz que renomear um job é decisão, não edição, porque
   renomear remove a porta em silêncio.
3. **Política de pin do dependabot:** `rust-mcp-sdk = "=2.0.0"` é exato de propósito e só sobe por
   decisão registrada no changelog, nunca por bump automático. Vale a partir de agora; o
   `dependabot.yml` em si é a fatia B.
4. **Revisão obrigatória no `master`:** a pergunta da proposta era se ela passa a ser exigida, e
   **eu a registro como não exigida**, com a razão. Exigir uma aprovação bloquearia o único padrão
   de merge em uso — o autor não pode aprovar o próprio PR, e todo PR desta série foi mesclado pelo
   próprio autor com o CI verde. Ligar isso sem um segundo revisor humano pararia o trabalho.
   **Fica explícito em vez de implícito, que era o pedido**; inverter é um ajuste de configuração,
   não de código.

### `.gitignore`

`.env` e `.envrc` entram. O produto já trata `*.env` como nome sensível
(`SENSITIVE_EXTENSIONS`, [D-089](#d-089--revisão-de-segurança)), mas o repositório não os ignorava,
então um arquivo local criado por engano era commitável.

### O que **não** entrou, de propósito

- **Cache de build.** O prompt o trata como opcional e alerta que restaurar no `main` um cache
  gravado por branch de PR é o caminho clássico de envenenamento. O suíte leva poucos minutos; não
  vale a superfície.
- **`deny.toml` e `dependabot.yml`** — fatia B. O `advisories` do `cargo-deny` consulta uma base
  viva e pode nascer vermelho por um crate transitivo, então ele entra **agendado no `master`, não
  bloqueando PR**.
- **SBOM e `harden-runner`** seguem opcionais, como a proposta diz.

### Proteção de branch

Criada nesta entrega, depois de os dois jobs rodarem uma vez para que os nomes existam: `default` e
`online` como status checks obrigatórios no `master`, com `strict` ligado.

### Verificação

`fmt` limpo, clippy limpo com `-D warnings` nas duas features, e as duas portas de shell rodadas na
árvore. 248 testes no build default e 261 com `online`, sem mudança — esta fatia não toca código de
produto além dos dois atributos de lint.

---

## D-108 — Cadeia de suprimentos: `cargo-deny` agendado e dependabot

Fatia B de [`spec/plan/proposta-pbt-e-ci-cd.md`](plan/proposta-pbt-e-ci-cd.md).

### O desconhecido que eu tinha apontado resolveu-se verde

A proposta dizia que o `advisories` era "o único desconhecido real", porque consulta uma base viva e
pode nascer vermelho por um crate transitivo. Instalei o `cargo-deny` 0.20.2 e rodei antes de
versionar qualquer configuração:

| seção | resultado |
| --- | --- |
| `advisories` | **ok**, zero entradas ignoradas |
| `bans` | ok, com 4 avisos de duplicata (abaixo) |
| `licenses` | falhava só por falta de allowlist; verde com as 8 entradas deste arquivo |
| `sources` | ok, tudo de crates.io |

### E derrubou uma afirmação minha

A proposta afirmava, na seção que eu apresentei como verificada: **"Nenhum crate duplicado em duas
versões."** É falso. O `cargo-deny` encontra **quatro** — `base64`, `getrandom`, `syn` e
`windows-sys`. Meu pipeline de conferência era

```
cargo tree --prefix none | awk '{print $1}' | sort -u | awk '{print $1}' | sort | uniq -d
```

que deduplica nome+versão e **só então** procura linhas repetidas — não podia achar nada, por
construção. É a mesma classe de erro da função de slug que primeiro me disse que 98 de 98 âncoras
estavam quebradas: a ferramenta de verificação estava errada, não o objeto verificado. As quatro são
pins transitivos, não acionáveis daqui, e ficam em `warn`.

### Um achado de licença que se dissolveu ao ser lido

O `cargo deny list` reporta **`LGPL-2.1-or-later`** no grafo, o que num crate MIT é o tipo de coisa
que para o trabalho. Lido: `r-efi 6.0.0` é `MIT OR Apache-2.0 OR LGPL-2.1-or-later`. O `list`
**decompõe expressões `OR`**, então o LGPL é uma das três opções e o braço MIT nos satisfaz — não há
obrigação de copyleft. O crate ainda por cima só entra em target UEFI. Mesma história com `BSL-1.0`
(`ryu` é `Apache-2.0 OR BSL-1.0`).

Nenhum dos dois entra na allowlist, e o `deny.toml` diz por escrito **por que não**, para ninguém
"consertar" um alarme futuro alargando a lista.

### A allowlist é exata, não generosa

Oito entradas, exatamente o que está no grafo hoje: `MIT`, `Apache-2.0`,
`Apache-2.0 WITH LLVM-exception` (o braço de exceção do `wasi` é expressão distinta para o
casador), `ISC` (a metade `online`: ring, rustls, untrusted), `Unicode-3.0` (a pilha ICU sob o
`idna`), `Unlicense` (`ignore`, `walkdir`, `memchr`), `BSD-3-Clause` (`subtle`, licença única) e
`CDLA-Permissive-2.0` (`webpki-roots`, licença única). Cada uma com o crate que a exige em
comentário, para que alargar a lista apareça como decisão no diff.

`sources` ficou em **`deny`** para registro e git desconhecidos, não em `warn`: algo fora do
crates.io é pergunta de cadeia de suprimentos, não aviso.

### Agendado no `master`, nunca em PR

Novo workflow `supply-chain.yml`, job **`cargo-deny`** — deliberadamente **não** chamado `default`
nem `online`, que são os checks obrigatórios criados no
[D-107](#d-107--ci-endurecido-dois-jobs-e-quatro-promessas-viram-portas) e não podem ser confundidos
com este. Roda por `schedule` semanal, por `workflow_dispatch`, e em push no `master` **restrito aos
caminhos** `deny.toml`, `Cargo.lock`, `Cargo.toml` e o próprio workflow — assim um `deny.toml` ruim
é pego pelo commit que o escreve, sem transformar deriva de base de advisories em falha do autor do
PR.

O `cargo-deny` é instalado **sem pin de versão**, de propósito: o objetivo do job é ver a base de
advisories de hoje com a ferramenta de hoje. As duas actions seguem pinadas por SHA.

### Dependabot, e a política de pin explícita

`cargo` e `github-actions`, semanal. **`rust-mcp-sdk` está em `ignore`**: o `=2.0.0` é exato de
propósito — a data de protocolo stateless foi validada contra um ripwire real
([D-002](#d-002--compatibilidade-de-protocolo-sdk--ripwire)) e a versão que ele reporta faz parte do
guarda de compatibilidade ([D-105](#d-105--a-versão-do-ripwire-deixa-de-custar-um-processo-por-evento-de-hook)).
Sobe por decisão registrada aqui, nunca por bump automático. Era o ponto 3 aprovado pelo usuário.

O ecossistema `github-actions` existe justamente por causa do D-107: pinar por SHA troca uma tag
mutável por uma congelada, e sem alguém propondo a atualização o pin envelhece.

### Verificação

`cargo deny --all-features check`: as quatro seções **ok**, 4 avisos de duplicata. `cargo deny check`
(só features default): as quatro **ok**. Os dois YAML validados por parser, não por leitura. Nenhum
código de produto tocado — 248 e 261 testes seguem iguais.

---

## D-109 — Primeiras propriedades, e um vazamento de credencial que elas fecharam

Fatia C de [`spec/plan/proposta-pbt-e-ci-cd.md`](plan/proposta-pbt-e-ci-cd.md): `proptest` como
dev-dependency, o alvo `tests/props.rs`, e as propriedades de P0.7, P0.5, P0.8 e P0.9. **P0.6 veio
junto**, fora da ordem da proposta, porque `decision.rs` é vizinho de `response.rs` e as duas
propriedades se escrevem com o mesmo material.

### O achado que justifica a fatia: `redact::remote_text` vazava a credencial

O controle é de confidencialidade — é a última barreira antes de texto remoto chegar a status, log ou
agente. O código fazia, nesta ordem:

1. filtrar o texto para ASCII imprimível e espaço;
2. `clean.replace(secret, "[redacted]")`.

**Se o segredo carrega qualquer caractere não-ASCII, o passo 1 o apaga do texto, e então o passo 2
procura uma forma que não está mais lá.** Com `secret = "ab\u{a9}cd"`, a saída era
`"denied: abcd at edge"` — quatro dos cinco caracteres da credencial, em claro, exatamente onde o
módulo promete que nada passa.

O teste de exemplo que já existia cobria o caso **inverso** (um caractere de controle dentro do
*texto*, com segredo ASCII, que funciona). A direção que faltava nunca tinha sido testada.

A propriedade `remote_text_never_carries_the_secret` acha e **encolhe** para
`secret = "\u{ae}a 0!"`. Ela ficou **vermelha antes** da correção e verde depois, provado revertendo
só o `redact.rs` e rodando de novo. O caso encolhido virou também teste dirigido em
`tests/online_units.rs`, porque uma semente de regressão pode ser apagada e um teste com nome não.

A correção filtra o **segredo** do mesmo jeito que o texto e substitui essa forma. Pode
**super-redigir** — um segredo `a\u{a9}b` também remove um `ab` comum do texto — e isso está
comentado no código: é a direção segura para um controle de confidencialidade, e o texto em questão
é mensagem de erro remota, não algo que alguém parseia.

### Dois achados que eram das minhas propriedades, não do código

Vale distinguir, porque contar os dois como defeito do produto seria inflar o resultado.

1. **Subnormal.** Afirmei que uma probabilidade válida volta bit-exata. `1.591213237000899e-308`
   volta `1.5912132370008993e-308`.
2. **E nem um double normal volta bit-exato:** `0.45623431892261956` volta
   `0.4562343189226195`. O `serde_json` decodifica float de forma aproximada a menos que a feature
   `float_roundtrip` esteja ligada.

O segundo é fato do sistema e fica registrado: **`parse_answers` não promete round-trip exato.** A
deriva é de **1 ULP**, está no decodificador de JSON e não no validador, e só mudaria uma decisão
para uma probabilidade a 1 ULP de `ADMISSION` ou `SELECTION` — as comparações estritas toleram isso e
nenhum classificador responde nessa resolução. **Ligar a feature para comprar exatidão onde 1 ULP não
pode importar foi recusado.** A propriedade passou a afirmar "dentro de 1 ULP e dentro da faixa", que
ainda pega regressão real: truncamento, arredondamento para duas decimais, ler o campo errado.

### E três propriedades que abortaram por desperdício, não por falha

Com `PROPTEST_CASES=4096`, três abortaram com **"Too many global rejects"** — o `proptest` desiste em
1024 rejeições, **mesmo tendo 1857 sucessos**. A causa era minha: `prop::num::f64::ANY` mais um
`prop_assume!` de "fora da faixa" descarta quase todo caso, e `joined`/`cut` independentes geram
`cut >= len` na maioria das vezes.

Reescritas para **gerar por construção** em vez de filtrar: um `prop_oneof!` com NaN, ±infinito e as
duas faixas fora de `[0,1]`; e `prop_flat_map` tirando o corte do próprio comprimento da string.
Zero rejeições, e a propriedade do segredo passou a construir deliberadamente a classe que vazou —
núcleo ASCII imprimível mais um caractere que o filtro remove — em vez de gerar strings arbitrárias e
esperar acertar.

### O filtro do CI, corrigindo o comentário que o D-107 deixou

O [D-107](#d-107--ci-endurecido-dois-jobs-e-quatro-promessas-viram-portas) deixou como comentário
`-E 'test(/^prop_/)'`. **Está errado para este repositório:** os testes são nomeados como frases
(`a_note_never_costs_an_item_that_was_already_evidence`), e renomear tudo para carregar um prefixo
custaria mais do que compra. O filtro passa a ser por alvo, `-E 'binary(props)'`, e deixa de ser
comentário: virou passo, com `PROPTEST_CASES=4096`, que custa **~9 s** porque este alvo só tem
funções puras.

### Semente de regressão

O caminho do arquivo de regressões é fixado explicitamente
(`FileFailurePersistence::Direct("tests/props.proptest-regressions")`). Sem isso o `proptest` procura
um `lib.rs`/`main.rs` acima do teste — que nunca existe para um alvo de integração — e avisa no log a
cada corrida.

As duas sementes que ficaram das minhas propriedades defeituosas foram **apagadas**: as estratégias
mudaram de forma, e semente velha reinterpretada contra estratégia nova é ruído. Nenhum arquivo é
versionado nesta entrega porque não há falha pendente; a regra de versioná-lo segue valendo para
quando houver.

### Uma nota de processo: pela terceira vez o instrumento estava errado

O comando com que eu ia declarar a fatia verde era

```
cargo clippy --all-targets --locked -q -- -D warnings 2>&1 | tail -3 && echo "clippy ok"
```

O status de um pipeline é o do **último** comando, então o `tail` sempre devolve 0 e o `echo`
imprimia "ok" com o clippy falhando — havia um `unused_assignments` real em `props.rs`. É o terceiro
caso desta série: a função de slug que disse que 98 de 98 âncoras estavam quebradas, o pipeline de
duplicatas do [D-108](#d-108--cadeia-de-suprimentos-cargo-deny-agendado-e-dependabot), e agora este.
**O padrão é meu, não do repositório**, e a correção é sempre a mesma: checar o código de saída em
vez de ler a saída.

### Verificação

**271** testes no build default e **284** com `online` (eram 248 e 261): 22 propriedades mais o teste
dirigido do vazamento. `fmt` limpo, clippy limpo com `-D warnings` nas duas features — conferido pelo
código de saída — e `cargo deny --all-features check` com as quatro seções ok.

---

## D-110 — Fatia D: doze propriedades, e uma afirmação do prompt que não se sustenta

Fatia D de [`spec/plan/proposta-pbt-e-ci-cd.md`](plan/proposta-pbt-e-ci-cd.md): P0.4 (lotes de
requisição), P0.10 (linha de comando), P0.13 (injeção de prompt), P1.1 (`Retry-After`) e P1.2
(ordenação de candidatos).

**Nenhum defeito de produto nestas cinco superfícies.** Elas passaram como estavam, e isso é
resultado, não anticlímax — o vazamento do [D-109](#d-109--primeiras-propriedades-e-um-vazamento-de-credencial-que-elas-fecharam)
mostrou que quando há defeito as propriedades acham.

### A afirmação do prompt que é falsa

O [`spec/prompt/ci-cd.md`](prompt/ci-cd.md), no P0.4, pede como propriedade: *"a concatenação dos
lotes mais os `too_large` é exatamente a entrada, **na ordem**"*.

**Não se sustenta.** Um item que não cabe nem sozinho vai para `too_large` **onde ele ocorre**, então
as duas listas se intercalam: com o terceiro item grande demais, os lotes carregam `[1,2,4]` e o
`too_large` carrega `[3]`, e concatenar dá `[1,2,4,3]`, que não é a entrada. Escrever a propriedade
como o prompt pede a faria falhar por estar errada, não por o código estar.

O que vale, e o que está afirmado: **cada lista é subsequência da entrada** (nenhuma reordena) **e as
duas juntas são exatamente o multiconjunto da entrada** (nada se perde nem se inventa).

Também precisei afrouxar uma cláusula — com razão registrada no teste. No `source_selection`, um
lote pode passar de `EVIDENCE_BATCH_BYTES` **quando tem um único item**: o `batches` tem uma escapada
explícita (`n == 0`) porque recusar o item sozinho jogaria fora evidência que a requisição ainda
consegue carregar. A propriedade afirma `n == 1 || text <= EVIDENCE_BATCH_BYTES`.

### `local::wrap`, extraído para o controle ser alcançável

O escapamento do P0.13 vivia dentro do `format!` de `local::prompt`, que sobe um broker e um ripwire.
Uma propriedade não alcançava isso, e reimplementar o escape no teste testaria a minha cópia, não o
código. Extraí `pub fn local::wrap(task, &Envelope) -> String`, e `prompt` passou a chamá-la — mesma
razão da decisão de visibilidade do `markup`, com a diferença de que aqui é **função nova num módulo
já público**, não mudança de visibilidade.

A propriedade é forte por construção: **a região do payload não contém nenhum `<` nem `>`**, então
não há caso a caso a analisar, e há exatamente uma ocorrência de `CONTEXT_OPEN` e uma de
`CONTEXT_CLOSE` — inclusive quando o texto de repositório é literalmente `</ripwire-broker-context>`.

### Provado que as propriedades mordem, por mutação

"Passou" não é evidência de que uma propriedade detecta a deriva que afirma. Duas mutações dirigidas,
revertidas em seguida:

| mutação | resultado |
| --- | --- |
| tirar o `.replace('<', "\u003c")` do `wrap` | **pega** — `a raw < reached the payload` |
| inverter a chave de ordenação para `(rank, priority)` | **pega** — `(3, 0) before (0, 1)` |

### Correções menores encontradas ao escrever

- **`ServeArgs` não tem `budget_tokens`.** Escrevi a propriedade de ida-e-volta contra ele por
  inércia; `budget_tokens` é campo por requisição na chamada de ferramenta, não flag de processo. O
  round-trip usa `--ripwire-max-rss-mb`, `--incremental` e `--redact-workspace`.
- **O comentário de `RankedPath.rank` estava impreciso.** Dizia "posição do primeiro item do caminho
  na saída do ripwire", mas o `enumerate()` roda **depois** do filtro de docs, então é a posição entre
  os itens que chegam. Não é defeito — só a ordem relativa importa, e filtrar a preserva — mas o
  comentário agora diz isso.
- Confirmei por sonda que `--online` fora do `serve` é recusado nos três comandos de um disparo
  (`doctor`, `hook`, `prompt`), como o prompt afirma, e a afirmação virou propriedade.

### Verificação

**283** testes no default e **296** com `online` (eram 271 e 284): +12 propriedades. Com
`PROPTEST_CASES=4096`, os 34 testes do alvo `props` levam **~20 s** — subiu de ~9 s, o que é o preço
de P0.4 e P1.2 gerarem coleções. `fmt` limpo e clippy limpo com `-D warnings` nas duas features,
conferido pelo código de saída e não pela saída, depois de um `manual_range_contains` real meu.

---

## D-111 — Fatia E: o leitor tolerante derrubava o processo por aninhamento

Fatia E de [`spec/plan/proposta-pbt-e-ci-cd.md`](plan/proposta-pbt-e-ci-cd.md): a decisão de
visibilidade aprovada, P0.12 (`markup::parse`) e P0.11 (invariantes do orçamento pela costura
pública).

### O achado, pior que um panic

`content` e `element` se chamam, então aninhamento é **recursão de pilha**. Sondado antes de escrever
qualquer propriedade:

| entrada | resultado |
| --- | --- |
| `"<a>".repeat(1_000)` | parseia |
| `"<a>".repeat(10_000)` | **`fatal runtime error: stack overflow`, SIGABRT** |

Estouro de pilha **não é panic capturável**: o `catch_unwind` não salva, o processo morre. Num `serve`
de vida longa lendo o stdout de outro processo, isso é negação de serviço a partir de entrada de fora
do processo — exatamente a classe que o P0.12 existe para fechar, e um grau acima dos dois panics do
[D-091](#d-091--revisão-do-repositório-e-correções-de-robustez).

`MAX_DEPTH = 64` fecha: passado o teto, um `<` é lido como **texto** em vez de aberto como elemento.
Os payloads reais do ripwire aninham meia dúzia de níveis, então 64 está muito acima do legítimo e
muito abaixo do que qualquer pilha nota. O custo é estrutura perdida num documento aninhado além do
que o ripwire escreve, e está comentado no código.

**Provado que o teto sustenta peso:** com `MAX_DEPTH` em 10 000 000 o teste dirigido **aborta com
SIGABRT**; com 64, passa. É por isso que o teto é constante e não comentário.

### E a propriedade pegou a minha própria regressão em minutos

O primeiro corte do conserto usava `rest[1..]` para garantir progresso no branch de texto. Isso
**fatia no meio de um caractere multibyte** e entra em panic — `start byte index 1 is not a char
boundary; it is inside 'é'`. A mesma classe de defeito que o D-091 consertou, reintroduzida por mim,
no próprio patch que fechava o aninhamento, e achada pela propriedade antes de sair da máquina. O
branch agora avança pelo **primeiro caractere**, não pelo primeiro byte.

### A decisão de visibilidade, executada

`markup` passa a `pub mod` com doc dizendo que é interno e **não carrega promessa de estabilidade**.
`normalize`, `router`, `budget` e `dedup` ficam privados, como aprovado. O `markup` é o único dos
cinco que lê bytes de fora do processo, e o achado acima é a justificativa: fuzzá-lo por um
`FakeUpstream` teria significado que cada caso atravessa spawn, JSON-RPC e normalização — e um
documento com 10 000 níveis provavelmente nunca teria sido gerado.

### P0.11 pela costura pública, e três correções ao meu próprio teste

A propriedade **não repete** o
`the_shaped_envelope_never_exceeds_its_budget_and_grows_with_it`, que já cobre teto, monotonicidade,
`shown + omitted` e folga. Ela acrescenta a consistência do bookkeeping (`truncated ⟺ omitted > 0`,
`next_step ⟺ truncated`, `shown` conta o que está lá) e que **limitações nunca são descartadas**.

Três vezes a propriedade nasceu sem valor e eu só descobri **mutando o código**:

1. **A cláusula das limitações era `0 == 0`.** O fixture não produzia limitação nenhuma. Passei a
   injetar `dropped_positive` e `capped` para ter seis.
2. **Com uma limitação só, ainda era vacuosa.** Limitações ordenam primeiro, então uma é contabilizada
   com o envelope quase vazio e cabe em qualquer orçamento legal. Precisa de várias.
3. **E o gerador nunca chegava à fronteira.** `256..=100_000` uniforme praticamente não sorteia
   256–500, que é a única região onde a cláusula morde: com o invariante quebrado de propósito e
   sorteio uniforme, 32 casos **não pegaram nada**. Agora é `prop_oneof!` pesado no piso (6:2:1).

Depois das três, as duas mutações são pegas: `truncated = true` sempre, e limitação sujeita ao
orçamento.

### E um achado sobre o produto que saiu disso

Com seis limitações e o orçamento no piso, o envelope reporta **348 tokens contra 256 pedidos**. Não
é defeito — limitações nunca são cortadas (PRD 10.2 #1), e o próprio
[`spec/prompt/ci-cd.md`](prompt/ci-cd.md) enuncia a ressalva, que eu havia omitido da propriedade.
Vale registrar que **a tensão é alcançável no `MIN_BUDGET_TOKENS` com uma resposta realista**, não só
em teoria: um agente que pede o mínimo sobre uma resposta muito truncada recebe 36% além do que pediu.
A propriedade afirma `estimated ≤ budget` **ou** `shown == 0`.

### Verificação

**287** testes no default e **300** com `online` (eram 283 e 296). `fmt` limpo e clippy limpo com
`-D warnings` nas duas features, conferido por código de saída. O alvo `props` com 4096 casos:
**~21 s**.

Nota de processo: o arquivo `tests/props.proptest-regressions` entrou no
[D-110](#d-110--fatia-d-doze-propriedades-e-uma-afirmação-do-prompt-que-não-se-sustenta) **sem eu
revisá-lo antes de commitar**, contra a regra que eu mesmo escrevi na proposta. Revisado agora: duas
sementes das corridas de mutação, sintéticas, sem caminho nem nada com cara de credencial. Ficam.

---

## D-112 — Fatia F: duas fronteiras de segurança fuzzadas, e quatro testes meus que não testavam nada

Fatia F de [`spec/plan/proposta-pbt-e-ci-cd.md`](plan/proposta-pbt-e-ci-cd.md): P0.1 (guarda de
workspace), P0.2 (política de elegibilidade) e P0.3 (unidades de evidência), em alvo próprio
`tests/props_fs.rs` porque cada caso faz E/S.

**Nenhum defeito nas duas fronteiras de segurança.** O guarda de traversal e a política de
elegibilidade passaram como estavam. O valor desta fatia está quase todo no que a mutação revelou
sobre os meus próprios testes.

### Quatro asserções minhas que não testavam nada

Cada uma passava. Cada uma foi exposta **quebrando o código de propósito** e vendo que o teste
continuava verde.

| o que eu afirmei | por que não valia | como ficou |
| --- | --- | --- |
| `!rel.contains("..")` | `"...."` é um **nome de arquivo** legítimo; só `Component::ParentDir` sobe. Falhava por a asserção estar errada, não o guarda | asserção sobre **componentes**, não substring |
| "uma listagem nunca oferece arquivo recusado" | `files_in` é **listador, não porta**: oferece nomes, e o `snapshot` decide — o chamador no `coordinator` já diz isso. Falhava em `.env` | afirma o limite que existe: o que a listagem oferecer, só elegível pode ser **lido** |
| a tabela de arquivos recusados | **nenhum caso isolava `sensitive_name`**: `.env` também é oculto, e `id_rsa`/`key.pem` também trazem marcador de chave privada. Removendo `sensitive_name` inteiro, a tabela **seguia verde** | entrou `service.key`, nome sensível com conteúdo inócuo e não oculto |
| cobertura das unidades sob `if !units.is_empty()` | o guarda dispensava a asserção exatamente quando a unidade perdida era **a única**. Removendo `out.extend(open)`, nada falhava | por caso, sem guarda — e o caso vazio tratado à parte |

A terceira é a mais séria: era defesa em profundidade escondendo um teste cego. Três controles
independentes recusavam os mesmos arquivos, então a tabela media o resultado e não o controle que eu
pensava estar medindo.

E a quarta rendeu um fato que eu tinha suposto errado: **um arquivo vazio é elegível**, snapshota com
digest de nenhum byte, e tem zero unidades.

### As mutações, todas pegas depois

| mutação | resultado |
| --- | --- |
| `resolve_existing` devolve o caminho lexical, sem resolver symlink | **pega** — o caminho por `out/` (symlink para `/etc`) é aceito e o oráculo o resolve fora da raiz |
| `sensitive_name` sempre `false` | **pega** — `service.key: expected eligible=false, got Ok(...)` |
| linhas de símbolo ignoradas | **pega** — `no unit starts at symbol line 2` |
| a última unidade aberta descartada | **pega** — `a non-empty file produced no unit` |

### As regras de segurança do próprio suíte, cumpridas

Duas destas propriedades geram **caminhos** e perguntam a um guarda de traversal, então: raiz sempre
em `tempfile::TempDir`; **nenhum caminho gerado é criado, escrito ou removido** — string gerada só é
*perguntada*; os arquivos são feitos à mão em nomes fixos; nada gerado é executado; e onde o conteúdo
é gerado (o `is_fresh` e as unidades), o **caminho é fixo e o valor gerado é o conteúdo**. Tamanhos
com teto muito abaixo de `MAX_READ_BYTES`.

O oráculo do P0.1 é o que o prompt pede: quando o caminho aceito existe, ele tem de
**`canonicalize` de volta para dentro da raiz**. É essa cláusula que pega a fuga por symlink, e a
raiz de teste tem um symlink para `/etc` de propósito — lido nunca, escrito nunca.

### No CI

Passo próprio, `-E 'binary(props_fs)'`, **sem** `PROPTEST_CASES=4096`: a contagem (48) está no
arquivo, porque cada caso escreve arquivos. O alvo inteiro leva **0,24 s**, então o custo é
desprezível e a separação existe pelo motivo certo — não misturar um alvo que faz E/S com um que roda
milhares de casos.

### Verificação

**294** testes no default e **307** com `online` (eram 287 e 300). `fmt` limpo e clippy limpo com
`-D warnings` nas duas features, conferido por código de saída depois de um `unused import` real meu.

---

## D-113 — Fatia G: o escalonador, e a quarta vez que o instrumento era o problema

Fatia G e última de [`spec/plan/proposta-pbt-e-ci-cd.md`](plan/proposta-pbt-e-ci-cd.md): P1.3, o
escalonador com o `FakeClassifier` que já existia. **Nenhum defeito.**

### O que a propriedade afirma

Sobre misturas geradas de jobs e configurações, com tempo pausado: o teto de requisições em voo
(RF-ONLINE-08) **observado pelo próprio classificador**, o orçamento de requisições contando toda
tentativa, nada de id inventado, e — a que custa mais para acertar — **uma resposta nunca migra para
a pergunta de outro job** quando as respostas voltam fora de ordem. Cada item é roteirizado com uma
probabilidade própria, senão todos compartilhariam um valor e qualquer troca passaria.

O teste terminar é a asserção de ausência de deadlock.

### Duas afirmações minhas que não valiam

1. **"Todo job submetido aparece no relatório."** Falso, e não é defeito: `unfinished` é documentado
   como jobs **admitidos** e nunca respondidos, então um job ainda na fila quando o escalonador para
   no `request_limit` não está em lista nenhuma — nunca foi admitido. Eu afirmei uma promessa que o
   relatório não faz, o mesmo erro do `files_in` no
   [D-112](#d-112--fatia-f-duas-fronteiras-de-segurança-fuzzadas-e-quatro-testes-meus-que-não-testavam-nada).
   O que vale é a direção de segurança: **um job fora da contabilidade só é aceitável enquanto o
   relatório se declara incompleto**, porque ausência nunca pode ser lida como resposta (PRD §23.2).

2. **`report.requests <= request_limit` sozinho não vê subcontagem.** Fazer o contador parar de
   incrementar o mantém abaixo de qualquer limite para sempre — e aí o limite nunca para nada. A
   propriedade passou a comparar com o **oráculo**: `report.requests == fake.calls()`, o que o
   classificador realmente viu.

### As quatro mutações, todas pegas

| mutação | resultado |
| --- | --- |
| teto de voo + 2 | **pega** — `2 requests in flight against a ceiling of 1` |
| `report.requests += 0` | **pega** pelo oráculo — `the report counts 0, the classifier saw 1` |
| respostas invertidas antes do `JobResult` | **pega** — `came back with another item's answer` |
| `incomplete()` devolvendo `false` | **pega** — `jobs [2, 3, 4, 5] are unaccounted for and the report calls itself complete` |

### E a nota que importa mais que a fatia

A quarta mutação eu **declarei não pega, e estava errado: ela nunca compilou.** Usei
`return false; #[allow(unreachable_code)] …`, e atributo em expressão é instável — o `cargo test`
falhou com `error[E0658]`, e o meu `grep` por uma mensagem específica não distingue "o teste passou"
de "nada compilou".

É a **quarta vez nesta série** em que o instrumento de verificação, não o objeto verificado, era o
defeituoso: a função de slug que disse que 98 de 98 âncoras estavam quebradas, o pipeline de
duplicatas do [D-108](#d-108--cadeia-de-suprimentos-cargo-deny-agendado-e-dependabot), o
`clippy | tail -3 && echo ok` do
[D-109](#d-109--primeiras-propriedades-e-um-vazamento-de-credencial-que-elas-fecharam), e agora este.
Todas as quatro têm a mesma forma: **eu li a saída em vez de checar o código de saída.**

E isso teve consequência: eu já havia mudado o gerador de `request_limit` "para consertar" a mutação 4,
sob diagnóstico errado. A mudança continua justificada pelos próprios méritos — medido nos casos
gerados, 8 jobs com limite 1 deixam 6 nunca admitidos, enquanto um sorteio uniforme largo termina
quase tudo e a cláusula não dispara —, mas **a razão que eu havia escrito no comentário era falsa** e
foi corrigida antes do commit.

### A proposta está cumprida

Todas as sete fatias: A ([D-107](#d-107--ci-endurecido-dois-jobs-e-quatro-promessas-viram-portas)),
B ([D-108](#d-108--cadeia-de-suprimentos-cargo-deny-agendado-e-dependabot)),
C ([D-109](#d-109--primeiras-propriedades-e-um-vazamento-de-credencial-que-elas-fecharam)),
D ([D-110](#d-110--fatia-d-doze-propriedades-e-uma-afirmação-do-prompt-que-não-se-sustenta)),
E ([D-111](#d-111--fatia-e-o-leitor-tolerante-derrubava-o-processo-por-aninhamento)),
F ([D-112](#d-112--fatia-f-duas-fronteiras-de-segurança-fuzzadas-e-quatro-testes-meus-que-não-testavam-nada))
e G. Dois defeitos de produto achados, os dois de segurança: o vazamento de credencial do D-109 e o
abort por aninhamento do D-111.

### Verificação

**295** testes no default e **308** com `online` (eram 294 e 307). `fmt` limpo, clippy limpo com
`-D warnings` nas duas features conferido por código de saída, e `cargo deny --all-features check`
com as quatro seções ok.

---

## D-114 — O `sha2` 0.11 é recusado, e uma premissa minha estava errada

Primeira proposta do dependabot desde o
[D-108](#d-108--cadeia-de-suprimentos-cargo-deny-agendado-e-dependabot), e o primeiro teste da
política de pin que ele registrou: PR #20, `sha2` 0.10.9 → 0.11.0. **Fechada sem mesclar.**

### A premissa que eu havia afirmado, e que é falsa

Eu disse duas vezes ao usuário que o `sha2` 0.11 "mexe no que gera as chaves de cache" e que, se o
digest mudasse, "todo cache armazenado é invalidado de uma vez". **Não muda.** SHA-256 é SHA-256; a
versão do crate não altera o valor do hash.

Verificado, não deduzido: apliquei o bump, portei os sítios e rodei
`the_keys_are_stable_across_runs`, o teste-ouro do
[D-109](#d-109--primeiras-propriedades-e-um-vazamento-de-credencial-que-elas-fecharam) que fixa o hex
de `notes::key`. **Passa.** Então não há cache invalidado e nenhum arquivo de estado órfão — o
[`src/state.rs`](../src/state.rs) nomeia o arquivo de sessão pelo hash do id da sessão, e esse nome
continua idêntico.

Transformei "é um crate de hash" em "muda os hashes" sem conferir, e apresentei isso como o risco
central duas vezes. O teste-ouro que responde à pergunta existia desde o D-109.

### Os três fatos que decidem, medidos

1. **Não compila como está.** O 0.11 devolve `hybrid_array::Array` em vez de
   `generic_array::GenericArray`, e `Array` não implementa `LowerHex`: quebram **8** sítios de
   `format!("{:x}", …)` — `session`, `notes`, `state`, `summarizer`, `online::coordinator`,
   `online::reader`, mais dois em `tests/`. Com um helper de hex de três linhas compila. **O porte é
   mecânico e não é o obstáculo.**

2. **O obstáculo é que não compra nada.** O `rust-mcp-sdk 2.0.0` depende ele mesmo de `sha2 0.10.9`
   e de `hmac 0.12`:

   ```
   digest v0.10.7
   ├── hmac v0.12.1   → rust-mcp-sdk v2.0.0
   └── sha2 v0.10.9   → rust-mcp-sdk v2.0.0
   ```

   Subir o nosso direto **não remove** o 0.10 do grafo: **adiciona uma segunda cópia**. Passaríamos a
   compilar duas implementações do mesmo hash, com duas versões de `digest`, `block-buffer` e
   `crypto-common`. As duplicatas que o `cargo-deny` reporta vão de **4 para 9**. E o
   `rust-mcp-sdk = "=2.0.0"` é exato por decisão
   ([D-002](#d-002--compatibilidade-de-protocolo-sdk--ripwire),
   [D-108](#d-108--cadeia-de-suprimentos-cargo-deny-agendado-e-dependabot)), então **o `sha2` sai do
   0.10 junto com o SDK, não antes dele.**

3. **Não há advisory empurrando.** `cargo deny check advisories` está limpo no 0.10.9. É higiene de
   versão, não segurança.

### A decisão

Recusado. Trocar uma versão de um crate de hash por duas, para não ganhar nada enquanto o SDK pinado
mantém a antiga, é andar para trás. **Revisitar quando o `rust-mcp-sdk` mover** — e aí o `sha2` sobe
no mesmo movimento, que é o que a política de pin do D-108 já dizia por outras palavras.

### O que **não** foi feito, e a consequência

Eu havia recomendado também adicionar `sha2` ao `ignore` do `dependabot.yml`, com o motivo, para o
mesmo PR não voltar toda semana. **Não foi pedido e não foi feito.** A consequência é concreta: o
dependabot vai propor de novo no próximo ciclo, e esta decisão não tem dente nenhum contra isso —
alguém terá de fechar à mão outra vez, olhando para este registro.

O contrapeso de ignorar, se um dia for feito, é que um bump de **segurança** do `sha2` também
deixaria de ser proposto. Isso está coberto: o `cargo-deny` **agendado no `master`** vê advisories
independentemente do dependabot, que é exatamente a razão de ele ter ido para o schedule e não para o
CI de PR no D-108.

### Nota de método, a quinta

Ao restaurar a árvore depois do experimento, escrevi `cargo build … | tail -2 && echo ok` e o "ok"
imprimiu **com o build falhando**: um pipeline devolve o status do último comando. Mesma forma dos
quatro casos já registrados (a função de slug, o pipeline de duplicatas do D-108, o
`clippy | tail -3` do D-109, a mutação que não compilava do
[D-113](#d-113--fatia-g-o-escalonador-e-a-quarta-vez-que-o-instrumento-era-o-problema)). E, pior, o
`git checkout -- Cargo.toml` restaurou **do índice**, que o experimento havia deixado com o 0.11
staged, então a árvore parecia restaurada e não estava. Resolvido com `git reset --hard HEAD` e
conferido por `if cargo …; then`, sem pipe.

### Verificação

Árvore de volta em `sha2 = "0.10"`, build e as duas suítes verdes (295 / 308), duplicatas de novo em
**4**, `cargo deny --all-features check` com as quatro seções ok. O experimento não deixou nada
atrás: nenhum arquivo de `src/` alterado, `Cargo.toml` e `Cargo.lock` idênticos ao `master`.

## D-115 — Diagrama de arquitetura versionado

Um diagrama da arquitetura do broker, gerado com o archify e versionado em
[`spec/diagrams/`](diagrams/):

- [`ripwire-broker.architecture.json`](diagrams/ripwire-broker.architecture.json): a fonte. É ela
  que se edita; o HTML é derivado.
- [`ripwire-broker.architecture.html`](diagrams/ripwire-broker.architecture.html): o artefato
  entregue, autocontido (SVG inline, tema claro/escuro, visões guiadas).

### O que ele mostra

O caminho principal `Agent host → MCP facade → Broker core → Read-only guard → Upstream client →
ripwire --mcp`, com stdio nas duas pontas; o modo automático (hook/prompt chamando o broker
in-process e gravando fingerprints no estado de sessão); e, tracejados, os dois enriquecimentos
opcionais: notas por modelo local e o coordenador semântico do `--online` falando HTTPS com o Jev.

Os fatos vêm do README e dos comentários de módulo (`//!`) de `src/`. Cada componente aponta para
os seus arquivos (17 referências), fixadas no commit `5daaf27` do repositório público; o archify
verificou que todos existem nessa revisão.

### Duas simplificações, nomeadas

1. O **Read-only guard** aparece como um passo do caminho principal. No código é o
   `guarded_call` de [`src/broker.rs`](../src/broker.rs), a única passagem para o ripwire: só
   verbos de leitura da allowlist ([D-014](#d-014--allowlist-e-validação-de-capabilities)) passam.
   É uma função do núcleo, não um processo nem uma camada separada.
2. O **`__supervise`** (limite de memória, [D-050](#d-050--limite-de-memória-do-ripwire-por-supervisor)) está
   dentro da caixa do Upstream client, não desenhado à parte.

### Um erro da primeira versão, apontado na revisão

A primeira versão chamava esse nó de **Workspace guard**, com a seta `paths`: todas as três
ferramentas pareciam passar pela verificação de caminhos. **Não passam.** O CodeRabbit apontou no
PR #27, e o código confirma: o [`src/workspace.rs`](../src/workspace.rs) só é aplicado em
`context_after_edit`, a `files` e `symbols`, antes de qualquer chamada upstream (CA-08).
`context_for_task` e `context_before_finish` não recebem caminhos do agente e não passam por ele.
O que toda chamada atravessa é a allowlist de verbos. O nó foi renomeado para o que é universal, e
a verificação de caminhos ficou no cartão de segurança, restrita a `context_after_edit`.

### Verificação

`validate --quality showcase`: 9/9 checagens do artefato, 0 erros e 0 avisos de composição.
`deliver`: especificação sha256 `25e8d8d0…` (7732 bytes), HTML `a7287595…` (720229 bytes), 17
referências de fonte verificadas. `visual-check` no Chrome: sem rolagem em 1440×900, 1600×1000,
1920×1080 e 2048×1320; menor texto projetado 6,6 px a 1440 (mínimo 6). As capturas e o recibo do `visual-check` não foram versionados:
são reproduzíveis a partir do HTML.

**Não é atualizado sozinho.** Uma mudança de arquitetura não quebra nada aqui; o diagrama envelhece
em silêncio. Quem mudar a topologia edita o JSON e roda de novo `deliver`.

## D-116 — Plano da avaliação A/B e de `session_hits` em uso real

Pedido do usuário: implementar os itens 1 (corpus A/B e barras, §16.2, §17 e §23.15) e 2
(medir `session_hits` em uso real, §21.3) da revisão de pendências. Pediu um plano, testes que
provem a falta antes e a solução depois, e o mínimo de impacto no resto. Plano em
[`spec/plan/plano-ab-e-session-hits.md`](plan/plano-ab-e-session-hits.md).

### O que não dá para entregar num commit

Os dois itens são **medições**. O commit entrega o instrumento; a medição continua com o usuário:

- o A/B real precisa dos três repositórios, de ≥ 30 tarefas e da autorização do gasto. Cada
  execução é uma sessão paga de agente;
- o `session_hits` real precisa de dias de hooks em uso. Nesta máquina, `hook-stats` encontrou
  **zero** sessões: os hooks não estão instalados aqui.

### Item 2: a medição era impossível, não só pendente

Cada evento de hook é um processo novo (D-030). O `metrics.session_hits` vive na memória do broker e
morria com o processo; o estado em disco (D-032) guardava as impressões digitais, mas não o
contador. O teste `session_hits_survive_across_hook_processes` mostrou isso em vermelho: o segundo
processo **contou** os hits (asserção da linha 827 passou até ali), e o estado não guardou nada.

- `SessionState.stats` (`#[serde(default)]`): `events`, `injections`, `delivered`,
  `session_hits` e `started_at`. Um estado antigo carrega com zeros.
- `hook::handle` soma a diferença de `Broker::session_hits()`, um getter novo e a única
  adição ao núcleo, para não duplicar a conta do broker.
- `delivered` conta só o que chegou ao modelo inteiro. Uma referência a algo já entregue é hit,
  não entrega.
- `ripwire-broker hook-stats [--json]` soma todas as sessões e mede a repetição **entre**
  sessões. Essa repetição é o que um cache persistente acrescentaria, e as impressões digitais já
  estavam em disco. Só contagens: nenhum caminho, símbolo, impressão digital ou id de sessão,
  nem o hash.
- Regra proposta para S3.15, **a decidir pelo usuário**: depois de ≥ 20 sessões, repetição
  entre sessões < 15% recusa o cache persistente, e ≥ 30% o põe no plano.

### Item 1: `ripwire-eval`, um segundo binário

- **Onde.** `ripwire_broker::eval` e `src/bin/ripwire-eval.rs`, com
  `default-run = "ripwire-broker"`. O broker não ganha subcomando; nada do `serve` ou dos hooks
  chama o módulo.
- **Braços.** `none`, `ripwire`, `broker` e `broker-online`. O online exige a chave e recusa
  antes de rodar qualquer coisa.
- **Agente.** Claude Code headless (`stream-json`) com `--strict-mcp-config
  --setting-sources project`. Sem isso, os hooks e MCPs globais deste ambiente (ripwire, graft)
  contaminariam o braço `none`. Uma **guarda de contaminação** lê o `system/init` e invalida a
  execução com servidor MCP não declarado, ou com o servidor do braço desconectado.
- **Extração do transcript.** Tokens e custo do `result`, chamadas por classe (busca, leitura,
  edição, MCP), bytes MCP, primeira edição e arquivos apresentados pelos envelopes do broker. Um
  transcript sem `init` ou `result` é **inválido**, nunca zero.
- **Pontuação.** Recall e precisão de arquivos por `git status` contra o patch de referência,
  posição do primeiro arquivo certo, testes de referência mostrados ou rodados, e correção pelo
  `check` da tarefa.
- **Barras.** §17.1–17.5 e §23.15.1–3, com bordas exatas (tolerância 1e-9) e `insuficiente`
  abaixo de 30 tarefas em 3 repositórios válidas nos dois braços comparados.

A fixture de transcript é **sintética**, escrita conforme a documentação do `stream-json`.
Gravar uma real exige uma chamada paga, e fica para a primeira rodada autorizada.

### Um vazamento achado ao montar a primeira tarefa

Para o teste de fumaça, montei uma tarefa a partir do histórico deste repositório: o `base` no
pai de `2a646f3` e a referência no próprio `2a646f3`. Ao escrevê-la ficou claro que o desenho do
plano (`git clone` local) levaria **todos** os branches, inclusive o commit da correção, para
dentro da cópia do agente. Um `git log --all` lhe daria a resposta. Isso invalidaria em silêncio
toda tarefa tirada de commits reais, que é justamente o jeito barato de montar o corpus.

O teste `the_agent_cannot_see_history_after_the_base` ficou vermelho (o agente de teste leu
`the reference fix`). A cópia passou a ser um `git init` com `git fetch` só do id do `base`: vêm o
commit e seus ancestrais, sem branch nem tag. O repositório de origem continua só lido.

### Provado que os testes mordem, por mutação

Doze mutações:

- **Seis pegas na primeira passada.** A soma dos hits, a contagem de injeções, a ordenação
  cronológica do `hook-stats`, a guarda de contaminação, a chamada da guarda no runner e o
  `result` ausente.
- **Três sobreviveram, e cada uma virou correção.**
  - `delivered` contando referências: nenhum teste injetava referências. Novo teste
    `a_reference_to_something_already_delivered_is_a_hit_not_a_delivery`.
  - Primeira edição × última: a fixture tinha uma edição só. Novo teste
    `the_first_edit_is_the_earliest_one`.
  - `EPS = 0`: **o instrumento estava errado**. O filtro `bars` não casava com
    `the_online_bar…`, o único caso em que `1 − 40/50` dá `0,19999…`. Com o filtro certo, pega.
- **As três, repetidas, foram pegas.**

### Nota de método, a sexta

Restaurar o arquivo mutado com `shutil.copy` + `move` devolveu um mtime **anterior** ao do build do
mutante. O cargo não recompilou, e um teste "falhou" no código correto, porque rodava o binário
mutante. Resolvido com `touch` nos fontes antes de cada verificação final. É o mesmo padrão dos
cinco casos anteriores: o instrumento mentindo de um jeito plausível.

### Impacto no resto

- As suítes existentes ficaram verdes **sem editar nenhum teste antigo**.
- `cargo tree -e normal` está byte a byte igual ao de antes, então o CA-10 está intacto. A guarda
  de fixture também passa.
- A saída dos hooks para o host não mudou; o arquivo de estado ganhou um objeto de contagens.
- `ripwire --quality-delta`: a complexidade 70 de `transcript::summarize` foi dividida em
  tratadores por evento. Os achados que bloqueiam e restam são `match` de enum para `&str`
  (`Arm::name`, `Verdict::label`) lidos como clones de `event_name` e `Status::as_str`, e
  funções de teste lidas como código morto: limitações da ferramenta, aceitas.

### Também visto, não mexido

Os links `plan-fases-2-3.md` e `plan-fases-4-5.md` do PRD apontam para `spec/`, mas os planos
estão em `spec/plan/`. Esses links estão quebrados desde antes desta mudança.

### O CI pegou o que a minha máquina escondia

A primeira execução do CI no PR #28 falhou em `the_agent_cannot_see_history_after_the_base`. O
commit da "correção" no repositório de teste dependia de uma identidade do git: a minha máquina
tem uma global, e o runner não tem. O `sample_repo` já passava `-c user.email`/`-c user.name`, e
o teste novo não. Agora passa também, e a suíte inteira roda verde com
`GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null`, a condição do runner. O runner do A/B
não faz commit e não precisa de identidade.

### Verificação

**315** testes no padrão e **328** com `online` (eram 295 e 308; 20 novos: 4 de hooks, 3 de CLI e
13 do A/B). Ignorados continuam 2 e 4. `fmt` limpo, clippy limpo com `-D warnings` nas duas
features, conferido por `if cargo …; then`, sem pipe.

## D-117 — O corpus real: três repositórios, e duas falhas de isolamento

O usuário escolheu os três repositórios do A/B: dois privados, aqui chamados **A** (Elixir/Phoenix)
e **B** (site Svelte/Vite), e este. O corpus tem **32 tarefas** tiradas de commits reais e mora
**fora de qualquer repositório**: enunciados, SHAs e testes ocultos descrevem código privado. Este
repositório, público, não recebe nada disso, e este registro chama os privados só de A e B.

| repositório | tarefas | com `check` validado | `vocabulary_diverges` |
| --- | --- | --- | --- |
| A (Elixir/Phoenix) | 12 | 12 | 6 |
| `ripwire-broker` (Rust) | 11 | 11 | 4 |
| B (Svelte/Vite) | 9 | 7 | 4 |

As 32 passam no `ripwire-eval check`. Das 30 com `check`, todas passam no `validate` com a
máquina parada; as outras duas são visuais e só medem custo e recall. O trabalho foi distribuído
entre três agentes, um por repositório, sob uma especificação comum (`ab-eval/SPEC.md`):
repositório de origem só leitura, nenhum agente pago, enunciados como issues que não entregam a
solução, e `check` que precisa distinguir base de fix.

### Isolamento, primeira falha: hooks versionados

O repositório A versiona `.claude/settings.json` com **hooks do graft** e plugins. O template padrão
do D-116 usava `--setting-sources project` e os carregaria em todos os braços, inclusive no `none`.
A guarda de MCP não perceberia, porque hook não é servidor MCP. Corrigido em três frentes:

- o template passa a `--setting-sources local`, que só lê `.claude/settings.local.json` (uma cópia
  nova nunca tem). Pela documentação, isso também deixa de fora o `CLAUDE.md` do projeto e o
  `.mcp.json`; o agente ainda pode lê-los, mas ninguém os injeta;
- `--include-hook-events` no template;
- a guarda invalida qualquer execução em que um hook rodou.

### Isolamento, segunda falha: outra ferramenta de contexto pelo shell

Desde 2026-09-20 o repositório A versiona `graft/`: cerca de 700 resumos e um `INDEX.md` que manda
rodar `graft ask`. O `graft` e o `ripwire` estão no PATH desta máquina, e pelo Bash nenhuma
listagem MCP os mostra.

- **A guarda nova** invalida uma execução que roda `graft` (em qualquer braço) ou `ripwire` (fora
  do braço `ripwire`).
- **O que não é pego:** o binário `ripwire-broker`, porque no próprio repositório rodá-lo é o
  trabalho, e um `rg ripwire`, que procura a palavra e não roda a ferramenta.
- **Prevenção:** as três tarefas cujo base tem `graft/` (app-10 a app-12) o removem no `setup`. A
  remoção acontece antes do agente e não conta como edição dele.

### O que o instrumento ganhou

- **`setup`, `teardown` e `env` por tarefa.** O repositório A precisa de `deps`/`_build` e de um
  banco por execução (`MIX_TEST_PARTITION=_{run}`). O que o `setup` deixa na árvore é subtraído das
  edições do agente. O `teardown` roda sempre que a cópia existe, mesmo quando o agente nem
  começou.
- **`fix` e os marcadores `{repo}`, `{fix}` e `{run}`.** Permitem testes ocultos: o `check` traz os
  testes do commit de referência depois que o agente termina.
- **`ripwire-eval validate`.** Exige que cada `check` falhe no base e passe no fix, e grava a saída
  de cada `setup` e `check` em `validate-logs/`. O `run` guarda a do `check` junto do transcript.
- **Diretórios temporários.** Usavam `Instant::now().elapsed()` no nome, que é sempre perto de
  zero; a unicidade dependia de um número do chamador, que no `validate` se repetia. Agora há um
  contador do processo.

### Um `check` que falha só por carga

A primeira validação do corpus inteiro deu as nove tarefas app-01 a app-09 como **falhando no
fix**. O agente que as montou tinha visto as doze passarem. Rodada à mão, app-01 passou (42
testes). Repetidas com a máquina parada, **as nove passaram**. Durante a rodada que falhou, eu
compilava e testava este crate em paralelo, e as tarefas que passaram (app-10 a app-12) rodaram
depois disso.

Há duas explicações, e **nenhuma está provada**, porque o `validate` daquela rodada descartava a
saída do `check` (é a lacuna que os logs acima fecham):

1. **Carga:** timeouts de conexão do Ecto sob CPU saturada.
2. **Outra sessão no mesmo Postgres:** ao conferir depois que os repositórios de origem estavam
   intactos, achei uma **outra sessão do Claude Code** ativa no repositório A havia mais de duas
   horas, com trabalho próprio no working tree (nada deste trabalho, e não mexi em nada). Se ela
   rodou a suíte durante a validação, as duas dividiram o mesmo Postgres, onde as migrações do A
   têm efeitos globais no cluster e já colidiram entre dois agentes deste trabalho. Para a rodada paga, a consequência é concreta: um `check` derrubado por
carga vira, em silêncio, "resposta errada" para aquele braço. Por isso a rodada roda em série, com
a máquina parada, **sem nenhuma outra sessão usando o Postgres ou o working tree do repositório A**
(o `setup` copia `deps` e `_build` de lá), e com os logs de `check` disponíveis para auditar cada
falha.

### Ressalvas do corpus, para a revisão dos enunciados

- **Nomes de interface.** Alguns enunciados nomeiam funções, constantes ou campos (rb-02, rb-03,
  rb-04, rb-07, app-10, app-12, land-01, land-02) porque os testes ocultos os importam. É contrato,
  não solução, mas encurta a busca.
- **Diagnóstico no enunciado.** rb-05 já entrega o diagnóstico. Isso vale igual para todos os
  braços, mas encolhe a diferença que o A/B mede: quanto menos o agente precisa procurar, menos
  importa quem o ajuda a procurar.
- **Testes que dependem da data.** Os do repositório A usam a data de hoje e janelas de meses:
  revalidar antes de rodar.
- **Concorrência no Postgres.** As migrações do repositório A têm efeitos globais no cluster: duas
  validações em paralelo já falharam com `tuple concurrently updated`. Uma execução por vez.

### Provado que os testes mordem

- **Mutações.** As oito mutações das partes novas foram pegas: a subtração do que o `setup` deixou,
  a falha do `setup`, o id em minúsculas, o `{run}` no `env`, a guarda de hooks, a guarda de
  ferramenta pelo shell, e o log do `check` no `validate` e no `run`.
- **Testes escritos junto com o código.** Os testes dos logs foram escritos sem rodar antes do
  código, então não houve vermelho; as mutações fizeram esse papel.

### Verificação

**320** testes no padrão e **333** com `online` (eram 315 e 328 no D-116; 5 novos no A/B).
`fmt` e clippy limpos nas duas features. O eval passa sem identidade git configurada
(`GIT_CONFIG_GLOBAL=/dev/null`), e `cargo tree -e normal` continua idêntico ao de antes do D-116.

## D-118 — Revisão do PR #29, e handoff

O usuário pediu o `handoff.md` publicado no `master`. O `master` é protegido (checks obrigatórios,
`enforce_admins`), então ele entra pelo PR #29, que leva o D-117, e o #29 é mesclado. Antes disso,
os três apontamentos do CodeRabbit no #29 foram conferidos contra o código. **Os três procediam.**

### 1. A guarda de shell era fácil de contornar, e acusava inocentes

O `programs()` do D-116 dividia a linha em `|`, `;` e `&` sem olhar aspas e tomava a primeira palavra
de cada pedaço. Isso dava dois tipos de erro:

- **Contaminação que passava.** `env graft ask`, `FOO=bar ripwire .`, `bash -c "graft ask"`,
  `command graft …` e `time nohup ripwire .`: o programa visto era `env`, `FOO=bar` ou `bash`, e a
  execução contaminada contava como limpa.
- **Falso alarme.** `rg "foo|graft ask"` virava um comando `graft`, e uma execução limpa seria
  invalidada.

Agora há um mínimo de análise de shell: os comandos são divididos só fora de aspas, as atribuições
`VAR=x` e os invólucros (`env`, `command`, `exec`, `nohup`, `time`, `nice`, `sudo`) são pulados, e o
`sh`/`bash`/`zsh -c '…'` é examinado por dentro. **Limite declarado no código:** uma substituição
`$(…)` dentro de aspas duplas não é examinada. A mesma análise serve à classificação de busca e
leitura, que também ganhou com as aspas.

### 2. Um `fix` que não é commit só aparecia na rodada

O `Corpus::validate` conferia o `base`, mas não o `fix`. Com um SHA errado, o `{fix}` do teste oculto
falharia em todos os braços, e a falha seria lida como resposta errada. Agora é recusado antes de
qualquer execução. As 32 tarefas do corpus real passam.

### 3. A edição do agente num arquivo que o `setup` tocou sumia

O D-117 subtraía das edições do agente todo arquivo que o `setup` deixara modificado. Se o agente
editasse esse mesmo arquivo, a edição sumia do recall. Agora cada arquivo que o `setup` mexeu é
lembrado pelo sha256 do conteúdo (ou como ausente), e só é descontado se o agente não o mudou de
novo.

### Testes e mutação

- **Testes vermelhos primeiro.** Três novos, vermelhos antes e verdes depois.
- **Mutações:** as seis foram pegas — atribuições, invólucros, `sh -c`, separador entre aspas,
  comparação de conteúdo e validação do `fix`.

### O handoff

[`handoff.md`](../handoff.md), na raiz:

- estado por fase e como verificar;
- onde está cada coisa;
- as duas medições em andamento, com os passos na ordem;
- as pendências fora delas;
- as armadilhas já pisadas;
- as convenções.

Ele nomeia os repositórios privados do corpus só como A e B.

### Verificação

**323** testes no padrão e **336** com `online` (eram 320 e 333). `fmt` e clippy limpos nas duas
features. `ripwire-eval check` passa no corpus real com a validação nova do `fix`.

## D-119 — Pendências do handoff: `sha2` no dependabot e links dos planos

Duas pendências do [`handoff.md`](../handoff.md) que não dependiam do mantenedor.

### O `sha2` 0.11 no `ignore`

O D-114 recusou o `sha2` 0.11, mas o `dependabot.yml` não o ignorava, e o PR voltaria toda segunda.
Agora `sha2` com `versions: [">= 0.11"]` está no `ignore`, com o motivo no comentário. **Ele sobe
junto com o `rust-mcp-sdk`:** quando uma decisão mover o SDK pinado para uma versão que traga o
0.11, a mesma decisão tira essa linha.

### Links dos planos

Os planos foram para `spec/plan/`, e os links não acompanharam:

- o PRD e o changelog apontavam para `plan-fases-2-3.md` e `plan-fases-4-5.md` em `spec/`;
- os dois planos apontavam para `changelog.md` em `spec/plan/`.

Todos corrigidos. Uma varredura dos links relativos em `spec/`, `README.md`, `handoff.md` e
`integrations/` não acha outro quebrado, **exceto em `spec/old/`**, que é arquivo histórico e fica
como está.

### Verificação

Sem mudança de código. A sintaxe do `ignore` com `versions` é a da documentação do dependabot; só a
próxima execução semanal confirma que o PR não reabre.

## D-120 — O cancelamento sob HTTP/2 ganha teste

Pendência do [`handoff.md`](../handoff.md): o CA-ONLINE-12 (um cancelamento MCP chega ao pedido
HTTP) só era provado em HTTP/1.1, onde abortar fecha a conexão TCP. Em produção, o `reqwest` pode
negociar HTTP/2 por ALPN, e aí a conexão é compartilhada e continua aberta: o sinal de aborto é um
`RST_STREAM` por pedido. Um cliente que só largasse o futuro sem resetar o stream deixaria o
provedor processando, e o teste antigo não veria.

### O teste

`an_mcp_cancel_resets_http2_streams_in_flight`, em `tests/online_protocol.rs`:

- **Fixture:** um servidor `h2` que aceita cada stream, nunca responde e conta os streams que o
  cliente resetou com `CANCEL`. Um cliente que falasse HTTP/1.1 falharia no handshake e não chegaria
  a contar um pedido.
- **Cliente:** `JevClient::loopback_h2`, só para fixtures, como o `loopback`, mas com
  `http2_prior_knowledge` (h2c). Sem TLS não há ALPN para negociar, e é a única diferença para o
  cliente de produção.
- **Cenário:** o mesmo do teste HTTP/1.1, agora num helper comum aos dois: uma tarefa com etapa
  online, o primeiro pedido chega ao provedor, a chamada é abortada como no `notifications/cancelled`.
  Todo stream em voo tem de ser resetado, e nenhum pedido novo pode sair depois.

**Limite:** a negociação por ALPN sobre TLS continua só nos testes ao vivo (`tests/online_live.rs`),
que não cancelam nada. O que o h2c prova é o comportamento do cliente depois que o HTTP/2 está de pé.

### A dependência

`h2 = "0.4"` em `[dev-dependencies]`. Não é crate novo: o `reqwest` com `http2` já o traz (0.4.19 no
`Cargo.lock`). E dev-dependency não chega ao build padrão: o gate do CI e o
`the_build_has_no_network_stack` olham `cargo tree -e normal`, e os dois continuam limpos.

### O teste morde

O código de produção já fazia certo, então o teste nasceu verde. Duas mutações provam que ele não é
decorativo:

- **o envio numa task destacada** (`tokio::spawn`), de modo que abortar a chamada não larga o pedido:
  pega, e derruba também o teste HTTP/1.1;
- **o `loopback_h2` sem `http2_prior_knowledge`**: pega, com nenhum pedido chegando ao provedor, em vez
  de passar falando HTTP/1.1.

Rodado 20 vezes seguidas, sem falha.

### Verificação

**323** testes no padrão (sem mudança) e **337** com `online` (eram 336). `fmt` e clippy limpos nas
duas features, e o gate do CA-10 continua limpo.

## D-121 — Três mecanismos que reproduzem as falhas de validação do D-117

O D-117 registrou nove tarefas do repositório A falhando **no fix** na primeira validação do corpus,
e passando de novo depois, com duas suspeitas não provadas: carga e outra sessão no mesmo Postgres.
O corpus original está noutra máquina, indisponível, então a investigação usou um **corpus de
diagnóstico**: 8 tarefas de PRs recentes do A (os mais novos que o último `mix.lock`), fora de
qualquer repositório, só para `validate`, nunca para um agente. As fases foram rodadas pelo
mantenedor, uma por vez.

### O que foi medido

| fase | condição | tarefas estáveis no fix |
| --- | --- | --- |
| base | máquina parada, relógio real (01/10) | 4 ok; **2 falham pela data** |
| congelado | relógio do A preso no dia em que cada PR foi escrito | **6 ok** |
| carga | 16 processos saturando 8 núcleos, relógio congelado | 4 ok; **1 teste perdido** em 389; 1 cortada |
| disputa | a suíte inteira do A em laço no mesmo Postgres, noutra porta | **6 ok** (3 rodadas concorrentes) |
| sobreposta | duas fases ao mesmo tempo, por engano | **todas falham com `eaddrinuse`** |

Duas tarefas ficaram fora da conta: uma cujo `check` passa no base, e outra que falha no fix em
todas as condições, com o relógio congelado inclusive. A causa dessa última não foi achada, e ela
não é o sintoma do D-117.

### Os três mecanismos, por força

1. **A porta do endpoint de teste.** A suíte do A sobe o servidor HTTP numa porta fixa, a menos que
   uma variável de ambiente diga outra. Duas suítes do A ao mesmo tempo na mesma máquina: a segunda
   morre com `eaddrinuse` **antes do primeiro teste**, e o `check` inteiro conta como falha. É o
   único mecanismo visto que produz falhas **em série** que **somem sozinhas** quando a outra suíte
   para, que é o que o D-117 descreve. A "outra sessão do Claude Code" que ele achou ativa no A teria
   esse efeito se rodou a suíte. O próprio A já tinha visto isso no CI, com dois PRs em paralelo.
2. **A data.** Testes do A fixam datas ou janelas relativas a hoje. Nos commits `fix` do histórico
   eles ficam congelados como foram escritos, e envelhecem: dois `check` validados em 30/09 falharam
   em 01/10, e voltaram a passar com o relógio de teste do A preso em 30/09. **Provado por
   experimento.** O D-117 rodou exatamente na virada de 30/09 para 01/10, e o A consertou esses testes
   na mesma madrugada.
3. **A carga.** Real, mas fraca: com a CPU saturada, um teste com janela de 100 ms perdeu a
   mensagem. Um teste em cerca de 500 não explica nove tarefas.

**O Postgres compartilhado, sozinho, não derrubou nada.** O `tuple concurrently updated` do D-117
veio de duas validações aplicando migrações ao mesmo tempo, o que a regra de uma execução por vez já
impede. Esta disputa só pôs a suíte concorrente no banco depois das migrações dela, então não mede
migração contra migração.

**Para o D-117 em si, nada está provado:** sem os logs e os horários daquela rodada, porta e data
são as duas explicações que encaixam, talvez juntas.

### Regra nova para o corpus

- **Toda tarefa de um repositório cujos testes leem a data ou abrem porta fixa** declara no `env`:
  o relógio congelado no dia em que o PR foi escrito (a data de autoria do branch, `fix^2`, não a do
  merge), e uma porta própria. O A oferece as duas variáveis. "Revalidar no dia" só descobre o
  envelhecimento; congelar o evita.
- **O `deps/` do repositório de origem tem de casar com o `mix.lock` do `base`.** O `setup` o copia:
  com um `deps.get` atrasado, nenhum `check` chega a rodar testes.
- **Uma execução por vez, máquina parada, nenhuma outra sessão no A** continua valendo, mas deixa de
  ser a única proteção.

Aplicar ao corpus original fica para quando a outra máquina voltar: lá, cada tarefa do A ganha as
duas linhas de `env`, e o corpus é revalidado.

### Notas de método

- **Datas de merge enganam.** O primeiro experimento congelou o relógio na data do commit `fix`, que
  é um merge: os oito foram mesclados em 01/10, e o relógio ficou onde já estava. Nada provou nem
  refutou. A data certa é a de autoria do branch.
- **Duas fases ao mesmo tempo dividem bancos, porta e logs** (os ids de execução se repetem). A
  rodada sobreposta foi o que revelou o `eaddrinuse`, mas não vale como teste de nenhuma hipótese. O
  script de diagnóstico agora recusa uma segunda fase.
- **Um processo em segundo plano tem limite de 30 minutos** no host do agente. A fase de carga não
  cabe: foi cortada, e o resultado parcial foi lido dos logs.

### Verificação

Sem mudança de código. O script e os logs ficam no corpus de diagnóstico, fora do repositório.

## D-122 — A barra de status entra no PRD

O mantenedor escreveu uma spec da barra de status do Claude Code (`spec/status-bar.md`) e pediu um
plano. Depois pediu a fusão da spec no PRD geral, o plano em `spec/plan/`, e a remoção da spec
**só depois** de validar que todo o conteúdo foi transferido.

### O que mudou no PRD

- **§24 Barra de status do Claude Code**, novo. A seção `N` da spec é o §24.`N`, com o corpo
  transportado literalmente: só os títulos foram renumerados, e três frases que falavam do próprio
  documento passaram a falar do capítulo. Os dados de origem da spec (status, data, commit
  analisado, referência do Graft) ficaram no início do capítulo.
- **§24.13 Decisões em aberto**, novo, vindo do plano: D1 a D6, cada uma com a recomendação que o
  plano segue até o mantenedor decidir, e o defeito do `#ripwire-on` sob falha de launch, registrado
  e não corrigido.
- **§19:** a entrada "Barra de status (antes da Fase 6)", com o motivo da ordem.
- **§21.6:** aponta para o §24.13.
- Cabeçalho (versão 0.4, data), sumário.

### Por que fundir, e por que o plano não entra

É o padrão do [D-056](#d-056--fusão-do-adaptador---online-no-prd): o PRD geral é a fonte do *quê*,
e os planos ficam em `spec/plan/`. A diferença: lá o original foi para `spec/old/`; aqui o mantenedor
pediu a remoção, condicionada à validação abaixo.

### O plano

[`spec/plan/status-bar-plan.md`](plan/status-bar-plan.md): 9 tarefas em TDD. O link de spec aponta
para o §24, uma nota diz que "spec §N" é o §24.`N`, os comentários de código citam o PRD, e a
implementação passa a ser o **D-123**.

### Validação antes da remoção

Um script comparou a spec com o §24, item por item: cada parágrafo, linha de tabela, item de lista e
bloco de código da spec tem de aparecer literalmente no §24, salvo as três frases reescritas; cada
link, cada trecho em código inline e cada número também. O resultado está no fim desta entrada. Só
depois a spec foi removida.

**Resultado:** 239 linhas de corpo, 23 títulos, 9 blocos de código, 28 links, 161 trechos em código
inline e 60 números da spec; **zero ausentes** no §24, e as três frases reescritas presentes na
forma nova. O script morde: com uma linha de tabela apagada, um número trocado e um link trocado no
§24, ele acusou os três. Depois da remoção, todos os links e âncoras relativos de PRD, changelog,
handoff e plano resolvem.

### Decisões D1 a D6 aceitas

Depois da fusão, o mantenedor aceitou as seis recomendações do §24.13 e escolheu a execução do plano
por subagentes (uma tarefa por vez, com revisão antes da seguinte). O §24.13 passou de "Decisões em
aberto" a "Decisões", e o §21.6 e o §19 dizem o mesmo.

## D-123 — A barra de status é implementada

O plano de [`spec/plan/status-bar-plan.md`](plan/status-bar-plan.md) foi executado por subagentes, uma
tarefa por vez, cada uma com revisão antes da seguinte, no branch `status-bar` (base `3505e67`). O
contrato é o §24 do PRD. **Duas coisas ficam pendentes e não foram feitas:** a validação manual numa
sessão real do Claude Code e a fixture de um payload real do `statusLine` (seção própria abaixo).
**Feitas no [D-128](#d-128--validação-manual-da-barra-e-fixture-de-payload-real).**

### O que entrou

- **`ripwire-broker statusline [--workspace DIR] [--state-dir DIR] [--detail] [--width N] [--color never|always]`**
  (`src/statusline.rs`, despacho em `src/main.rs`): lê o JSON do host no stdin (até 256 KiB), lê o
  snapshot da sessão (até 16 KiB) e imprime uma linha. Sai sempre com 0. Nunca chama `settings`,
  `local::launch`, `Broker::connect` nem `Broker::status`, e nunca cria arquivo ou diretório. Sem
  `session_id`, nenhuma sessão é lida (nunca a `default`). Segmentos: `rw-brkr`, modelo com versão e
  effort (`low`/`mid`/`hig`/`xtr`/`max`), `ctx N%`, `hooks on|off|sem dados`, `última: …`, `inj N`,
  `não reenviados N`; `--detail` acrescenta `entregues`, `reuso`, `último contexto ~N tok`, idade e
  `dados antigos`. Largura por `--width`, `COLUMNS`, 100; cor só com `--color always`, com as faixas
  do §24.5.3.
- **Projeção** (`src/statusline_state.rs`): `Snapshot` com `schema_version: 1`, publicado por
  `publish` em `<state-dir>/statusline/<sha256>.json`, onde o hash cobre, com prefixo de
  comprimento, (host, sessão, raiz canônica). O `session_id` nunca vira parte de um caminho.
  Diretório `0700`, arquivo `0600`, escrita atômica (`write_private`, extraído de `StateStore::save`
  sem mudar formato nem lugar). Leitura: não arquivo regular ou symlink conta como ausente;
  `schema_version` diferente, `Incompatible` (`hooks sem dados`, nunca leitura parcial); host ou
  chave de workspace diferentes, idem. O snapshot não tem prompt, código, caminho em claro, símbolo
  nem fingerprint.
- **Hooks** (`src/hook.rs`): o `SessionState` ganha `statusline` (`Summary`: chave do workspace,
  linha de base dos contadores, última análise, última entrega). `bind` vincula a sessão ao
  workspace; `analysed` registra a análise; `project` produz o snapshot como o contador da sessão
  menos a linha de base. `hook::run` salva o estado e **só depois** publica.
- **`install claude-code … --statusline`** (`src/install.rs`): o `statusLine` entra na mesma mudança
  (`Change`) dos hooks. Comando `'<binário>' statusline --workspace '<raiz>' --color never`, sem
  `refreshInterval`. Propriedade estrutural: é nossa a barra cujo programa se chama
  `ripwire-broker` e cujo primeiro argumento é `statusline` (não por substring). `--statusline` sem
  `--hooks` vira nota (a barra ficaria sem contadores).
- **Dependência nova:** `unicode-width = "0.2"` (D1). O gate do CA-10
  (`cargo tree -e normal | grep -Ei 'reqwest|secrecy|rustls|hyper'`) sai vazio.

### Decisões D1 a D6 do §24.13, como ficaram no código

| # | decisão | motivo |
| --- | --- | --- |
| D1 | `unicode-width = "0.2"` | mede largura visual sem tabela escrita à mão; sem dependências nem rede |
| D2 | sob largura extrema sobram `rw-brkr`, `ctx N%`, `hooks off`, `última: atenção`, `última: erro`; `hooks on`, `hooks sem dados`, `última: pronta`, `última: incerta` saem depois do modelo. Ordem de remoção: detalhe, contadores, modelo, os segmentos "macios" | a spec preserva "pausa dos hooks e alerta da última análise"; `hooks on` não é pausa, `pronta` não é alerta |
| D3 | só os hooks do Claude Code publicam | a barra só existe lá; o host entra na chave e no snapshot, então publicar para o Codex depois é uma linha |
| D4 | falha do `local::launch`: a análise vira `erro`, o estado é salvo e publicado; contadores intactos | esse caminho não conta evento hoje, e mudá-lo mudaria o `hook-stats` |
| D5 | barra alheia no settings do usuário: a nossa não é escrita (sombrearia), nota com trecho manual; no `settings.local.json`: a nossa é escrita, com nota de que a local prevalece; settings do usuário ilegível ou sem como localizar: nada é escrito, nota | não pisar numa barra que o usuário escolheu |
| D6 | **Revisada** (ver "Revisão final"): `agent` (objeto) só acrescenta `agente: <nome>` depois do modelo; o snapshot é lido. ~~Original: só segmentos do host e `agente`, sem ler snapshot~~ | o original supunha que o payload de um subagente não descreve a sessão dos hooks; a documentação diz o contrário |

### Decisões tomadas durante a execução (rulings do controlador)

Cada uma resolve um defeito do plano ou da revisão, com o custo.

- **`--width 0` é aceito** (T1). Uma barra de 0 colunas não mostra nada, que é o que cabe. *Custo:* uma
  barra em branco se um host mandar `COLUMNS=0`.
- **Mutação do plano substituída** (T5). "Remover `.take(MAX_STDIN_BYTES + 1)`" é um mutante equivalente
  no nível da saída (só muda memória); passou a valer "remover a checagem `> MAX_STDIN_BYTES`", que um
  teste com JSON válido preenchido até o limite e um byte além tem de pegar. *Custo:* o `take`, que
  limita a leitura sem estourar a memória, continua sem teste.
- ~~**Guarda de agente em `main.rs` sem teste** (T5). `(Some(session), false)` é um mutante equivalente.~~
  Superada pela D6 revisada: a guarda foi removida e o snapshot é lido também com `agent`.
- **Falha de launch não persiste a versão do ripwire** (T7). A revisão achou que o salvamento novo
  guardaria `{carimbo, "unavailable"}` em `state.ripwire` para um binário que existe mas não roda, e
  eventos seguintes confiariam nisso até o arquivo mudar (um `chmod +x` não muda tamanho nem mtime).
  A correção restaura `state.ripwire` ao valor carregado nesse ramo, em vez de proibir o cache de
  `unavailable` em geral (isso mexeria na semântica do D-105). *Custo:* uma leitura de versão numa
  falha de launch é jogada fora e refeita no evento seguinte (um `ripwire --version` a mais).
- **Leitura indeterminada vale também para o `settings.local.json`** (T8). `NotFound` é ausente, qualquer
  outro erro de leitura é indeterminado; aplicado ao local também, com nota, por ser a mesma classe
  ("origem não determinada", §24.7). *Custo:* uma nota a mais num caso raro.
- **Evidência de testes pelo controlador** (T5). O suite no `89b0c50` foi rodado pelo controlador (361 /
  2 ignorados padrão; 375 / 4 `online`); são os números de registro, no lugar dos 362/376 do implementador.
- **`cargo fmt --check` quebrado no T3** foi corrigido pelo T4, que edita o mesmo arquivo; o gate
  final passa em `cargo fmt --all --check`. *Custo:* um CI vermelho se o T4 pulasse.

### Defeito existente, fora do escopo, registrado e não corrigido

Quando o `local::launch` falha, `hook::run` retorna antes de `handle` (conferido em `src/hook.rs`: o
ramo `Err` devolve `failure(..)` sem chamar `handle`), e **nem `#ripwire-on` nem `#ripwire-off`** desse
prompt são processados: o opt-out não vale e o opt-in não reativa. Com o ripwire ausente, a barra mostra
`hooks off` se a sessão já estava pausada, até um prompt com o ripwire de pé. Está no handoff. Consequência nova do D4: uma sessão cujos eventos todos falham no launch deixa um
arquivo de sessão com contadores zerados, e o `hook-stats` passa a contar mais uma sessão.

### A ordem save → publish vale por leitura, não por teste

`hook::run` só publica se `store.save(..)` retornou `Ok` (curto-circuito na mesma expressão). Provar
"publicar antes de salvar" exigiria injetar uma falha de `save` que o código não expõe, então a ordem
foi verificada lendo o código, e o que o teste prova é a consequência: o teste da publicação que falha
(diretório `statusline/` impossível de criar) vê a saída do hook inalterada, e o evento seguinte
publica os totais certos.

### Mutações por tarefa

| T | mutação | resultado |
| --- | --- | --- |
| 1 | nenhuma registrada (só parse de CLI) | n/a |
| 2 | `"hig"` -> `"hgh"` | pega (2 testes) |
| 2 | `round()` -> `floor()` no `ctx` | pega (1 teste) |
| 3 | `60..=80` -> `60..=79` nas faixas de cor | pega |
| 3 | trocar `Counter`/`Model` na ordem de remoção | sobrevivia; três testes de ordem novos, agora pega |
| 3 | trocar `Model`/`Soft` | sobrevivia; agora pega |
| 3 | tirar `Soft` da lista | pega pelo teste novo |
| 4 | tirar o prefixo de comprimento do hash do caminho | pega |
| 4 | `is_file()` -> `!is_dir()` na leitura | pega (symlinks leem como ausentes) |
| 5 | apagar a checagem `> MAX_STDIN_BYTES` | pega, depois que o teste passou a usar JSON válido |
| 5 | `(Some(session), false)` -> `(Some(session), _)` | **equivalente** então; a guarda foi removida na revisão final |
| 6 | `analysed` depois de `has_news` na edição | sobrevivia; teste de edição sem novidade, agora pega |
| 6 | remover o `analysed` do ramo `Err` | pega |
| 7 | tirar `real_session` de `publishes` | pega |
| 7 | tirar a condição `Host::ClaudeCode` | pega |
| 7 | publicar antes de salvar | **não observável** por teste (ver acima) |
| 7 | apagar a restauração de `state.ripwire` | pega |
| 8 | nome do programa por `contains("ripwire-broker")` | pega |
| 8 | tirar `bar_wanted = false` (barra do usuário alheia) | pega |
| 8 | primeiro argumento por "qualquer palavra contém `statusline`" | sobrevivia; comando alheio `'/x/ripwire-broker' hook statusline` adicionado, agora pega |
| 8 | tirar `bar_wanted = false` (settings do usuário inválido) | sobrevivia; teste novo, agora pega |
| 8 | `Err(_) => Ok(None)` na leitura de settings | pega |
| 8 | tirar `bar_wanted = false` no ramo sem localização | pega |

### Pendências menores registradas nas revisões

Nenhuma bloqueia; ficam aqui para quem mexer nos arquivos.

- `src/cli.rs`: a doc do módulo passa de 110 colunas.
- `model_label` apara antes de sanear, e `"Son \n"` deixa um espaço final; aparar depois.
- `sanitize` deixa passar caracteres Unicode de formato e bidi (U+202E, U+200B): só cosmético ou falsificação.
- O denominador do `reuso` (`session_hits + delivered`) soma sem checar; usar `saturating_add`, já que vem de arquivo.
- Sem teste: `tokens()` abaixo de 1000 e o corte do `,0`; `age()` em min/h; `updated_at` no futuro.
- Largura 0 e 4 a 6 sem teste; o `fit` acha o `ctx` pelo prefixo do texto, não por uma variante de `Keep`.
- Comentários de largura errados em testes do T3 (somas e faixas); as asserções estão certas.
- `read()` tem uma janela TOCTOU entre `symlink_metadata` e `open` (o diretório é `0700`; `O_NOFOLLOW` fecharia).
- `reading_creates_nothing` só cobre state-dir ausente, não um state-dir existente sem `statusline/`.
- `write_private` deixa `.tmp<pid>` em caso de falha (já era assim), e threads do mesmo processo dividem o nome.
- Imports no meio do arquivo em `tests/statusline.rs`; `never_starts_ripwire` prova a não-criação só sem estado prévio.
- Sem teste: os caminhos de opt-out, fora do workspace e edição coalescida deixam `last_analysis`
  intacta; `statusline: None` omitido na serialização; o teste de opt-out não checa `last_analysis` vazia.
- `no_session_id_or_codex_publishes_nothing` passa de forma vazia se nada rodar; `session_id` é lido duas vezes em `run`.
- O teste do binário não executável cobre "sem versão em cache", não "versão anterior preservada".
- A nota `hooks sem dados` sai mesmo quando a barra não é escrita; um binário renomeado ou versionado
  (`ripwire-broker-0.2`) faz a própria barra antiga contar como alheia (propriedade estrutural pelo nome do programa).
- A variante "diretório" do `settings.local.json` ilegível não tem teste (divide o ramo de JSON inválido).

Resolvidas no D-127.

### Medição (spec §9)

Binário de `cargo build --release --locked`; 300 execuções de `target/release/ripwire-broker statusline`
por caso, processo novo a cada uma, stdin com `session_id`, `workspace.project_dir`, modelo, effort e
`context_window` (script Python com `subprocess`, tempo de ponta a ponta).

- **Máquina:** Apple M3; macOS 27.0.1 (build 26A434).
- **Comando:** `target/release/ripwire-broker statusline --workspace WS --state-dir SD --detail --width 200`.
- (O script do plano, Tarefa 9 passo 2, como escrito mede o caminho sem
  snapshot.) A projeção foi publicada **antes** de medir e conferida pela saída: nos casos com snapshot, a linha
  traz `hooks on · última: atenção · inj 7 · não reenviados 18 · entregues 25 · reuso 42% · último
  contexto ~1,2k tok · há 0s`. Sem o snapshot, `hooks sem dados`. Um snapshot não lido mediria o
  caminho errado, e o primeiro roteiro do plano (`--state-dir` sem projeção) mediria só o "sem snapshot".

| caso | snapshot | p50 | p95 | max |
| --- | --- | --- | --- | --- |
| sem snapshot | nenhum | 2,5 ms | 2,8 ms | 3,2 ms |
| pequeno | 423 bytes | 2,5 ms | 2,7 ms | 3,1 ms |
| perto do limite | 16 376 bytes (limite 16 384) | 2,5 ms | 2,7 ms | 3,1 ms |

Meta: p95 < 100 ms; passa por uma ordem de grandeza. Uma rodada anterior sem `--width 200` (a linha
de detalhe não cabia em 100 colunas) deu 2,6/3,1/5,2, 2,5/2,8/29,3 (um pico isolado) e 2,6/2,9/3,5
ms (p50/p95/max). A medição não inclui o tempo de o Claude Code desenhar a barra.

### Validação manual e fixture de payload real: PENDENTES

> Feitas depois, no [D-128](#d-128--validação-manual-da-barra-e-fixture-de-payload-real). O texto
> abaixo fica como estava.

**Não foram feitas.** O mantenedor roda a validação (spec §10): instalar com
`--hooks --statusline --write` numa pasta de teste, abrir o Claude Code, mandar um prompt, editar um
arquivo, encerrar o turno, `#ripwire-off` e `#ripwire-on`, comparando a barra com
`ripwire-broker hook-log --session ID` e com o snapshot, e registrar a versão do Claude Code. O
roteiro está pronto em `~/projects/ai/CECI/statusline-manual/` (a pasta local do mantenedor, não
versionada; `ROTEIRO.md`, `capture.sh` e a pasta
`ws/`); o `capture.sh` grava o payload real do `statusLine` em `payload-<epoch>-<pid>.json`.

Também pendente: transformar um desses payloads em `tests/fixtures/statusline/claude_code.json` (com
`__WORKSPACE__`), trocar por ele os JSONs sintéticos dos testes onde couber e conferir `effort.level`,
`workspace.project_dir` e `agent` contra a spec; se algum divergir, parar e reportar. Até lá, os
testes usam entrada sintética: **a leitura do payload do Claude Code real é a hipótese menos provada
desta entrega.** A versão do Claude Code usada para escrever o código: não registrada; o agente que
preparou o roteiro encontrou a 2.1.285.

### Contagem de testes

`cargo test --all-targets --locked`, antes (master `3505e67`, worktree temporário) e depois (`e3ced9b` mais
esta entrega, que só mexe em docs):

| alvo | antes | depois |
| --- | --- | --- |
| `tests/statusline.rs` (novo) | 0 | 34 |
| `tests/cli.rs` | 42 | 62 |
| `tests/hooks.rs` | 23 | 30 |
| demais alvos | iguais | iguais |
| **padrão** | **323 passados, 2 ignorados** | **384 passados, 2 ignorados** |
| **`--features online`** | **337 passados, 4 ignorados** | **398 passados, 4 ignorados** |

Mais 61 testes nas duas features (34 + 20 + 7); `mcp_surface` fica em 11 (13 com `online`) e os alvos
`online*` não mudam. Os números de registro do T5 (`89b0c50`) eram 361/375 (padrão/`online`); o T6 somou 7, o T7 somou 5 e o
T8 somou 11 (com as rodadas de correção), o que dá os 384/398 finais.

### Verificação

`cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings` (também com
`--features online`) e as duas suítes: **OK**, sem falhas. Gate do CA-10: `cargo tree --locked -e normal`
filtrado por `reqwest|secrecy|rustls|hyper` sai vazio.

### Revisão final

Uma revisão do branch inteiro pediu oito correções; todas feitas, cada mudança de comportamento com
teste primeiro (vermelho visto) e mutação (reverter o conserto e ver o teste falhar).

- **D6 revisada (F1).** A documentação do Claude Code (`code.claude.com/docs/en/statusline`) diz que
  o objeto `agent` (`agent.name`) aparece quando a sessão **principal** roda com `--agent` ou
  configurações de agente, e que subagentes usam um `subagentStatusLine` separado. A premissa da D6
  original (o payload com `agent` é de um subagente e não descreve a sessão dos hooks) era falsa, e
  com a regra antiga quem usa `--agent` nunca via os dados dos hooks. *Decisão do mantenedor:*
  `agent` não muda o que é lido nem mostrado; a barra mostra os segmentos normais mais um segmento
  macio `agente: <nome>` logo depois do modelo (nome saneado, no máximo 24 colunas; sem nome usável,
  `agente`). A guarda `(Some(session), false)` de `main.rs` foi removida; `HostInput` guarda
  `agent_name`. A decisão original continua visível, riscada, no §24.13 e na tabela acima.
- **Teste de ponta a ponta sem ripwire (F2).**
  `installed_hook_and_bar_agree_through_a_symlinked_workspace` roda `install --hooks --statusline
  --write` por um symlink, executa por `sh -c` o comando do hook e o da barra que o install escreveu
  (mais `--ripwire /nonexistent/ripwire` e `--state-dir`) e confere `última: erro`. O install grava
  a raiz canônica, então o symlink não é discriminante sozinho: a mutação que bate é desligar a
  publicação do hook. Os helpers que rodam o binário agora removem `XDG_STATE_HOME`.
- **Defeito conhecido (F3):** documentado acima (`#ripwire-off` também fica sem processar).
- **`install` (F4, ruling).** Se o settings do usuário passou a ter barra alheia e o do projeto tem
  a **nossa** (identificada pela estrutura, como `bar()` já faz), a do projeto é removida e sai uma
  nota ("removing the broker's statusLine ... so it does not shadow ..."), no dry run e no
  `--write`; reexecutar é idempotente. Uma barra alheia no projeto nunca é removida.
- **`"statusLine": null` (F5)** lê como ausente em `bar()`, não como alheia.
- **Saída da barra (F6):** `let _ = writeln!(stdout, ..)` no lugar de `println!`, que entra em
  pânico (101) com stdout fechado (EPIPE). Teste: `a_closed_stdout_never_makes_the_bar_fail`.
- **Docs (F7, F8):** README (mesmo `--workspace` nos hooks e na barra; `--agent`; sombreamento), §19
  do PRD reformatado, e a pasta `statusline-manual/` marcada como local e não versionada.
- **Continua pendência:** o endurecimento de `write_private` contra links (e os itens da lista de
  pendências menores acima), a validação manual e a fixture real.
- **Contagem depois da revisão** (`cargo test --all-targets --locked`): padrão **391 passados, 2
  ignorados** (+7); com `online` **405 passados, 4 ignorados** (+7). `tests/statusline.rs` 34 -> 36
  (o teste de agente foi substituído, +1 de nome do agente, +1 de stdout fechado); `tests/cli.rs` 62
  -> 67 (+5: `null`, remoção da barra nossa em dois cenários, barra alheia do projeto preservada, e
  o ponta a ponta). Gate completo (fmt, clippy nas duas features, as duas suítes) OK; `cargo tree`
  filtrado sai vazio.
- **Segunda rodada.** O corte do nome do agente era quadrático e sem limite de bytes: `truncate`
  recalculava a largura da saída a cada caractere e só parava ao passar do limite, então marcas
  combinantes (largura zero) nunca o paravam (um nome de 240 KB levou 190 s e imprimiu 240 KB).
  Agora o nome é limitado a 64 caracteres antes de medir (`MAX_AGENT_CHARS`) e `truncate` mantém a
  largura corrida; o corte é aparado no fim, sem espaço sobrando. Testes novos:
  `a_zero_width_heavy_agent_name_is_bounded_and_fast`,
  `a_cut_after_a_space_leaves_no_trailing_space` e
  `the_agent_segment_is_soft_and_goes_before_the_alert_and_ctx` (larguras 40 e 80). Contagem final:
  padrão **394 passados, 2 ignorados**; com `online` **408 passados, 4 ignorados**.

## D-124 — Cores do estado dos hooks na barra

**Data:** 2026-10-01. **Pedido do mantenedor**, depois do D-123.

Com `--color always`, o segmento de estado dos hooks ganha cor, como o `ctx`:

| texto | cor | ANSI |
| --- | --- | --- |
| `hooks off` | vermelho | `\x1b[31m` |
| `hooks on` | azul claro | `\x1b[38;5;117m` |
| `hooks sem dados` | sem cor | — |

- **Por quê:** `hooks off` é uma pausa pedida pelo usuário (`#ripwire-off`) e é essencial na ordem de
  descarte (D2); o vermelho a destaca como os outros alertas. `hooks on` em azul claro mostra o estado
  normal sem confundir com as faixas do `ctx` (cinza, branco, amarelo, vermelho).
- **`hooks sem dados` fica sem cor:** não é um estado que a sessão escolheu, só a falta de projeção.
- **Escolha do azul:** o índice 117 da paleta de 256 cores (o mesmo formato do cinza 250 do `ctx`). A
  aparência exata depende da paleta do terminal, como diz o §24.5.
- **`--color never`** continua sem ANSI; o ajuste à largura continua calculado sem os escapes.
- **Teste:** `hooks_off_is_red_and_hooks_on_light_blue_when_colored` (falhou antes da mudança; a
  mutação que devolve `hooks on` a `Plain` é pega). O teste de `--color never` passou a remover também
  o escape azul ao comparar o texto visível. Contagem: padrão **395 passados, 2 ignorados**; com
  `online` **409 passados, 4 ignorados**.

## D-125 — O marcador de opt-out só vale na borda do prompt

**Data:** 2026-10-01. **Pedido do mantenedor**, no mesmo PR do D-124.

- **Defeito:** o hook tratava como comando qualquer prompt que *contivesse* `#ripwire-off`
  (`prompt.contains`), e o opt-out era testado antes do opt-in. Na sessão que implementou a barra, os
  relatórios dos subagentes chegam ao Claude Code como prompts e passam pelo `UserPromptSubmit`. Um deles
  descrevia o defeito do D-123 ("nem `#ripwire-on` nem `#ripwire-off` …") e pausou os hooks da sessão
  sem que ninguém pedisse. A barra do D-124 mostrou `hooks off` em vermelho, e o `hook-log` parava
  ~70 minutos antes.
- **Regra nova:** o marcador só conta como **palavra inteira** e só na **borda**: a última palavra do
  prompt, ou então a primeira. Se as duas bordas tiverem marcadores, vale a última. Fora disso o
  prompt é uma tarefa comum, mesmo que cite o marcador (entre crases, com barra, colado a outra
  palavra ou no meio de uma frase).
- **Tarefa enviada ao broker:** só o marcador da borda é removido. Antes, todo `#ripwire-on` era
  apagado do texto em qualquer posição.
- **Custo:** quem escrevia o marcador no meio da frase ("pode #ripwire-off agora") precisa movê-lo
  para o começo ou o fim. O README e o §8.4 dizem isso.
- **Teste:** `a_marker_only_counts_as_the_first_or_last_word_of_the_prompt` (falhou antes da mudança
  com o primeiro texto citado). Mutações pegas: voltar a `contains`; tirar a exigência de fronteira de
  palavra no fim (`x#ripwire-off`). Os testes anteriores do opt-out (marcador no começo e no fim)
  continuam passando sem mudança. Contagem: padrão **396 passados, 2 ignorados**; com `online`
  **410 passados, 4 ignorados**.
- **Não muda:** o defeito do D-123 continua (com falha de launch, o marcador da borda também não é
  processado).

## D-126 — O marcador vale mesmo sem ripwire

**Data:** 2026-10-01. **Pedido do mantenedor.** Fecha o defeito registrado no D-123 (e lembrado no
D-125).

- **Defeito:** `hook::run` só chegava a `handle`, onde o marcador do prompt é lido, depois de
  `local::launch`. Com o ripwire ausente ou quebrado, o `#ripwire-off` não pausava (o host recebia a
  mensagem de falha) e o `#ripwire-on` não reativava (a sessão pausada continuava muda). A barra
  mostrava o estado errado até um prompt com o ripwire de pé.
- **Correção:** no braço de falha do launch, um `UserPromptSubmit` tem o marcador aplicado antes de
  tudo, pela mesma função (`toggle`) que o caminho normal usa:
  - **`#ripwire-off`:** a sessão é pausada, salva e publicada, e o host recebe a mesma confirmação do
    caminho normal, não a falha. Nada é analisado, então a `última` análise não muda.
  - **`#ripwire-on`:** a sessão é reativada e segue o caminho de falha do D4 (análise `erro`, salva e
    publicada, contadores sem mudança), então a retomada fica gravada.
  - **Sem marcador:** igual a antes (sessão pausada fica muda; ativa recebe a falha).
- **Cache da versão (D-105):** na pausa, o estado volta à versão do ripwire carregada, como no caminho
  de falha, para não gravar a leitura de um binário que não roda.
- **Testes:** `a_marker_is_honoured_even_when_ripwire_cannot_launch` (falhou antes da correção: a pausa
  era respondida como falha) e uma extensão de `a_launch_failure_does_not_cache_the_unrunnable_ripwire_version`
  para a pausa. Mutações pegas: ignorar o marcador no braço de falha; não salvar a pausa; não restaurar
  a versão carregada. Contagem: padrão **397 passados, 2 ignorados**; com `online` **411 passados,
  4 ignorados**.

## D-127 — Pendências menores da barra de status

**Data:** 2026-10-01. **Pedido do mantenedor.** Fecha a lista "Pendências menores registradas nas
revisões" do D-123 (item a item, abaixo), em TDD: teste vermelho, correção, verde, e uma mutação por
teste novo (reverter ou inverter a correção, ver o teste falhar, restaurar e dar `touch`).

**Correções de comportamento**

| # | Mudança | Teste | Mutação (pega) |
|---|---|---|---|
| 1 | `model_label` apara depois de sanear (`"Son \u{7}"` deixava `"Son "`) | `model_labels_are_trimmed_after_the_control_characters_go` | voltar a aparar antes |
| 2 | `sanitize` também descarta U+061C, U+200B–U+200F, U+202A–U+202E, U+2060–U+2064, U+2066–U+2069 e U+FEFF | `invisible_format_characters_are_dropped_like_controls` (cada caractere, vizinhos da faixa que ficam, e o rótulo de ponta a ponta) | deslocar o U+FEFF |
| 3 | `reuso`: `session_hits.saturating_add(delivered)` | `counters_near_the_limit_do_not_overflow_the_reuse_rate` (`u64::MAX`; entrava em pânico em debug) | voltar ao `+` |
| 4 | `fit` acha o `ctx` pelo campo `Segment::ctx`, não pelo prefixo do texto; ordem do D2 intacta | refatoração pura: os testes de largura existentes, sem alteração | `retain` invertido e `ctx` nunca marcado: 3 e 2 testes de largura falham |
| 5 | `statusline_state::read` abre uma vez, com `O_NOFOLLOW` e `O_NONBLOCK`, e verifica o arquivo aberto (regular, tamanho); sem `symlink_metadata` | `symlinks_and_directories_read_as_missing`, `a_fifo_reads_as_missing_without_blocking` (novo; falha por timeout, sem travar) | tirar `O_NOFOLLOW`; tirar `O_NONBLOCK` |
| 6 | `write_private`: temporário com `O_EXCL` e `O_NOFOLLOW`, nome `<hash>.tmp<pid>-<n>` (o `with_extension` troca o `.json`) com contador atômico do processo (até 8 tentativas em colisão), remoção do temporário em qualquer falha, e diretório criado `0700` (a regra do diretório existente foi revista, ver a Revisão) | `writers_of_one_file_do_not_share_a_temporary`, `a_failed_write_removes_its_temporary`, `a_symlink_planted_at_a_temporary_name_is_never_written_through`, `a_looser_existing_directory_is_tightened_and_a_shared_one_is_left_alone`, `the_session_store_tightens_its_directory_too` (vermelhos antes: o das threads, com `NotFound`; o do temporário que sobrava; os dois do diretório `0755` mantido) | nome sem contador; sem a remoção; `create(true).truncate(true)` sem `O_NOFOLLOW`; sem apertar o diretório externo e o interno; sem a guarda de `sticky` |
| 7 | `hook-stats` ignora a sessão sem evento e sem fingerprint (a que só teve falhas de launch) | `hook_stats_skips_sessions_whose_events_all_failed_to_launch` | filtro desligado; só `events > 0`; só fingerprints |
| 8 | `hook::run` lê `session_id` uma vez | refatoração pura (testes de hook existentes) | n/a |
| 9 | `install`: a nota `hooks sem dados` só sai com a barra escrita; programa cujo nome começa por `ripwire-broker` e primeiro argumento `statusline` é nosso | `the_hooks_note_is_printed_only_when_our_bar_is_written`, `a_renamed_or_versioned_binary_still_owns_its_bar` | nota sem `bar_wanted`; voltar à igualdade do nome; `ends_with` em vez de `starts_with` (pega em `ownership_is_structural_not_a_substring`) |

Escolhas que o pedido deixava em aberto:

- **`libc`:** a constante `O_NOFOLLOW`/`O_NONBLOCK` vem do `libc`, que já estava no grafo normal
  (`tokio`, `sha2`) e no `Cargo.lock`. Virou dependência direta (uma linha no `Cargo.toml`, uma linha
  no `Cargo.lock`); nenhum crate novo, nenhum `unsafe` (o crate o proíbe).
- **Diretório existente (item 6):** a primeira versão apertava para `0700` o diretório existente que
  fosse do mesmo dono; revista abaixo (nunca se muda um diretório existente).
- **`hook-stats` e sessões antigas (item 7):** só some a sessão sem eventos e sem nenhuma fingerprint.
  Uma salva antes dos contadores existirem (zero eventos, fingerprints reais) continua contando, como o
  README promete. A redação da saída não mudou.

**Testes que faltavam (10 a 17):** sem defeito achado; cada um foi mutado.

- **10:** `token_counts_show_whole_below_a_thousand_and_drop_a_zero_decimal` (mutações: limite
  `< 100`; sem o corte do `,0`) e `age_counts_seconds_minutes_and_hours_and_a_future_stamp_is_zero`
  (faixa dos minutos deslocada; `wrapping_sub` no `updated_at` futuro). Futuro: `há 0s`, sem `dados antigos`.
- **11:** `tiny_widths_cut_the_prefix_on_a_column` (larguras 0, 1, 4, 5, 6 e 7; mutação: `cols.max(7)`).
- **12:** `reading_creates_nothing` ganhou o state-dir existente sem `statusline/`.
- **13:** `paths_that_run_no_analysis_leave_the_last_analysis_untouched` (opt-out em edição e `Stop`, edição
  fora do workspace, edição coalescida, com uma análise sentinela; uma mutação por caminho, chamando
  `analysed`), `a_state_that_was_never_bound_serializes_without_the_status_line_field` (sem o
  `skip_serializing_if`) e a checagem de `last_analysis` vazia em `opt_out_and_opt_in_show_in_the_projection`.
- **14:** `no_session_id_or_codex_publishes_nothing` agora exige a saída `no context` e as duas sessões
  salvas (o hook rodou), e tem o controle positivo (claude-code com id publica). Mutações: publicar para
  Codex; publicar sem id; nunca publicar.
- **15:** `a_launch_failure_does_not_cache_the_unrunnable_ripwire_version` ganhou a versão semeada para
  outro binário, que sobrevive à falha (mutação: não restaurar `loaded_ripwire` quando havia uma).
- **16:** `a_settings_local_that_is_a_directory_still_gets_our_bar_with_a_note` (mutação: um
  `settings.local.json` diretório desliga a barra).
- **17:** `the_status_line_creates_and_changes_nothing_with_existing_state_either` (árvore do state-dir
  e do workspace com tamanho e mtime, antes e depois; sessão com e sem projeção; mutação: `read` cria um arquivo).

**Limpezas:** doc do módulo de `src/cli.rs` quebrada em 110 colunas (18); comentários de largura dos
testes de ordem de descarte corrigidos com os números reais (19: sem detalhes 90 colunas, sem contadores
62, sem modelo 45, sem `hooks on` 32, só prefixo e `ctx` 17; essenciais 44); imports no topo de
`tests/statusline.rs` (20).

**Sem vermelho observável:** a abertura sem `symlink_metadata` (5) e o `O_EXCL` (6) não mudam nada
que um teste sem corrida veja; o teste do FIFO e o do symlink plantado já passavam antes e provam o
código novo pelas mutações acima (o plantado cobre 5000 nomes `tmp<pid>-<n>`).

**Contagem (antes da Revisão):** padrão **416 passados, 2 ignorados**; com `online` **430 passados,
4 ignorados** (eram 397 e 411: 19 testes novos). `cargo tree --locked -e normal | grep -Ei 'reqwest|secrecy|rustls|hyper'`
(CA-10) sem saída.

### Revisão

Uma revisão independente deu "com ajustes"; tudo abaixo foi feito em commits novos, sem reescrever o
histórico.

**Decisões**

- **Nunca mudar um diretório existente (1).** O `tighten` do item 6 dava `chmod 0700` em qualquer
  diretório existente do mesmo dono e sem `sticky`. Um `--state-dir ~` ou `.` (o workspace) seria
  alterado sem volta (e perderia setgid e bits de grupo), contra "nunca escreve no workspace". O
  `tighten` foi removido: o diretório que o código cria (o de estado, se faltar, e o `statusline/`) nasce
  `0700` desde o início, e um existente fica como está. Custo: um diretório de estado antigo e mais
  aberto continua mais aberto (os arquivos dentro seguem `0600`), e o usuário o aperta à mão. Some
  também a falha de `chmod` que derrubava a escrita inteira.
- **Programa `ripwire-broker` exato ou versionado (4).** `starts_with("ripwire-broker")` reivindicava
  `ripwire-brokerage` e `ripwire-broker-wrapper.sh`. Agora o nome é `ripwire-broker` ou
  `ripwire-broker-` seguido de dígito (`ripwire-broker-0.2`). Custo: uma cópia renomeada de outro modo
  (`ripwire-broker.old`) volta a ser tratada como barra alheia e fica como está.
- **Temporários órfãos (11).** Com nomes únicos, o temporário deixado por um crash nunca é reaproveitado.
  Aceito, sem código: é inofensivo para `sessions()`, que só lê `.json`, e `0600`.

**Correções**

| # | Mudança | Teste | Mutação (pega) |
|---|---|---|---|
| 1 | sem `tighten`; diretório criado `0700`, existente intacto (`0755`, `2770` setgid, `1777`) | `existing_directories_keep_their_mode_and_new_ones_are_private` (substitui os dois testes de aperto; vermelho: o diretório mudava) | `chmod 0700` do existente; criar com `0755` |
| 2 | `write_private` recusa diretório de outro uid (compara com o dono do temporário recém-criado, sem `unsafe`; o temporário é removido) | `a_state_directory_owned_by_another_user_is_refused`: usa `/tmp` (do root, gravável por todos), e sai cedo se o dono for o próprio usuário | a comparação sempre aceita |
| 3 | `StateStore::lock` abre com `O_NOFOLLOW` | `the_session_lock_does_not_follow_a_symlink` (vermelho: o alvo era criado) | tirar a flag |
| 4 | o nome do programa é `ripwire-broker` ou `ripwire-broker-<dígito>` | `ownership_is_structural_not_a_substring` (`ripwire-brokerage`, `ripwire-broker-wrapper.sh`, `ripwire-broker-`) e `a_renamed_or_versioned_binary_still_owns_its_bar` (`-1.0.0`, exato) | `starts_with`; sem o nome exato |
| 5 | `sanitize` descarta também U+00AD, U+034F, U+180E, U+2028, U+2029, U+FE00–U+FE0F, U+FFF9–U+FFFB, U+115F, U+1160, U+3164, U+FFA0 e U+E0000–U+E007F | `invisible_format_characters_are_dropped_like_controls` (tabela estendida e vizinhos que ficam; vermelho em U+00AD) | tirar U+2028–2029, as tags e os seletores de variação, um a um |
| 6 | docs: README (tabela de comandos, medição, migração) e PRD (§21.3) dizem que a sessão sem evento e sem fingerprint é ignorada; o handoff, na regra das 20 sessões | n/a | n/a |
| 7 | o teste "não cria nem muda nada" registra também o modo de cada entrada | `the_status_line_creates_and_changes_nothing_with_existing_state_either` | `chmod 0755` do `statusline/` dentro de `read` |
| 8 | a nota `hooks sem dados` só sai se o settings do projeto, depois da mescla, não tem hooks nossos | `the_hooks_note_is_not_printed_when_the_project_already_has_our_hooks` (vermelho: a nota saía) | voltar à condição `!args.hooks` |
| 9 | texto do D-127: `<hash>.tmp<pid>-<n>` | n/a | n/a |
| 10 | `model_label`: `trim_end` antes das reticências | `a_model_name_cut_at_a_space_has_no_space_before_the_ellipsis` (vermelho: "aaa …") | tirar o `trim_end` |
| 11 | aceito, ver acima | n/a | n/a |

O PRD §24.13 D5 e o README documentam a regra do nome do programa.

**Contagem:** padrão **419 passados, 2 ignorados**; com `online` **433 passados, 4 ignorados**.
CA-10 sem saída.

## D-128 — Validação manual da barra e fixture de payload real

**Data:** 2026-10-01. **Pedido do mantenedor.** Fecha as duas pendências do
[D-123](#d-123--a-barra-de-status-é-implementada): a validação manual (§24.10) e a fixture de um
payload real do `statusLine`. Roteiro e resultados anotados em `statusline-manual/ROTEIRO.md` (pasta
local, não versionada).

**Ambiente:** Claude Code **2.1.285**, macOS, binário release deste repositório, workspace de teste
`statusline-manual/ws/` (repositório git com `src/lib.rs` e `Cargo.toml`), hooks instalados com
`install claude-code --hooks --statusline --write`. O settings global do mantenedor tem barra própria,
então o `install` não escreveu a barra (D5, como esperado) e a barra de teste entrou como override em
`ws/.claude/settings.local.json`: primeiro o `capture.sh` (grava o stdin e repassa ao broker), depois,
na segunda rodada, o comando que o `install` imprime.

**Resultado:** nos passos 3.1 a 3.6 a barra, o snapshot em `statusline/` e o `hook-log` bateram
(`inj`, `delivered`, `estimated_tokens`, `session_hits`, `opted_out`, `last_analysis.status`). O
payload real tem `effort.level` (string, `"medium"` → `mid`), `workspace.project_dir` (string),
`model.id`/`model.display_name` e `context_window.used_percentage` (número) com os nomes e tipos de
§24.2/§24.8. `agent` não apareceu (sessão sem `--agent`): **a parte de agente do §24.8 segue sem
prova com payload real.**

**Segunda rodada (passo 6), sem o wrapper:** override com o comando que o `install` imprime
(`'<binário>' statusline --workspace '<ws>' --color never`), sessão nova `2d463a40…`. A barra ficou
`rw-brkr · Opus 5.5 mid · ctx 6% · hooks on · última: atenção · inj 1 · não reenviados 0`, igual ao
snapshot novo dessa sessão (um por sessão × workspace) e ao mesmo comando rodado à mão; nenhum payload
novo foi gravado. Antes de a barra se redesenhar, o mantenedor viu `última: incerta`, e estava
certo: o `UserPromptSubmit` também grava `last_analysis` (com o `status` do envelope), e o envelope
desse prompt tinha `status: "unknown"` (ver a divergência 6). Entre o prompt e o `Stop` a barra
mostrava `incerta`; o `Stop` (`attention_required`) trocou para `atenção`.

**Divergências (registradas, nenhuma corrigida; decisões pendentes):**

1. **`hook-log` não mostra a edição (3.2): o roteiro estava errado; não é divergência do broker.**
   O `hook-log` lista injeções por definição (README: "What the hooks injected in a session",
   "lists the last 5 injections"): imprime `state.log`, que só o `record` preenche, para toda
   resposta que chega ao modelo, PostToolUse incluído. Uma edição aparece como linha `PostToolUse`
   quando injeta (`tests/hooks.rs` exige o log `[1, 2]` de prompt e edição); sem novidade
   (`has_news`), só conta em `stats.events`, como na reprodução das fixtures. Na validação nem isso
   podia acontecer: as edições foram por Bash e o hook não rodou (divergência 2). O passo 3.2 do
   roteiro foi corrigido.
2. **`stats.events` sobe 2 por turno mesmo com edição (3.1, 3.2): não é defeito da contagem;
   é um ponto cego.** Contagem 2 → 4 → 6 → 8. O transcript da sessão mostra que o Claude editou
   `src/lib.rs` pela ferramenta Bash (`echo '…' >> src/lib.rs`), não pelo Edit. O PostToolUse do
   broker só casa `Edit|Write|MultiEdit|NotebookEdit` (`src/install.rs`), então o Claude Code nem o
   chamou; os `PostToolUse:Bash` do transcript são de hooks globais do mantenedor. A contagem está
   certa: as fixtures de `tests/fixtures/hooks/` reproduzidas num state-dir novo dão 1 → 2 → 3
   (prompt, PostToolUse de Edit, `Stop`). **Questão aberta para o §24/hooks:** edição feita por shell
   (`echo >>`, `sed -i`, formatadores, geradores) é invisível ao broker, e o Claude escolheu Bash
   sozinho para uma edição trivial. Ou o matcher passa a incluir `Bash` (e a edição é detectada pela
   árvore do git), ou o limite fica documentado. O roteiro (passo 3.2) passou a pedir o Edit
   explicitamente.
3. **`last_analysis.event` é `"Stop"`, maiúsculo; o roteiro conferia `"stop"`: erro do roteiro.**
   O snapshot do §24.6.3 traz `"event": "Stop"`; `event_name` (`src/hook.rs`) usa os nomes de evento
   do próprio Claude Code (`UserPromptSubmit`, `PostToolUse`, `Stop`), os mesmos do `hook-log`; o
   `snap` de `tests/statusline.rs` usa `"Stop"`. O passo 3.3 do roteiro foi corrigido.
4. **`hooks off` mantém `última: atenção` (3.4): conforme, por desenho; não é divergência.** O
   §24.5.3 preserva "pausa dos hooks e alerta da última análise" juntos, o D2 põe `hooks off` e
   `última: atenção`/`erro` entre os essenciais, e o §24.5.2 resolve a leitura como estado atual
   pelo rótulo ("um resultado antigo sempre aparece como `última`, nunca como saúde atual"). O
   opt-out não apaga `last_analysis`, e testes exigem isso
   (`lines_fit_40_80_and_120_columns_and_shed_in_order`, `extreme_widths_keep_alerts_then_the_prefix`,
   `paths_that_run_no_analysis_leave_the_last_analysis_untouched`). Uma `atenção` aponta obrigação
   aberta que a pausa não resolve; escondê-la iria contra "incerteza não é escondida". O exemplo
   de `hooks off` do §24.5 não tem `última:` só porque não tem análise.
5. **`há …` do `--detail` parece a idade da entrega, mas é a da observação (3.6): o código segue a
   spec; a ambiguidade é de apresentação.** `há` é `now - updated_at`, e `updated_at` é gravado a
   cada hook do broker que roda (prompt, edição, `Stop`, opt-out/opt-in), como o §24.5.2 pede ("a
   idade indica o momento da observação"). Na saída `último contexto ~523 tok · há 18s`, os 18 s
   eram desde o último hook (um `Stop`); a última entrega tinha 290 s. Mas o §24.5.2 põe `há` logo
   depois de `último contexto`, e a leitura natural é "contexto entregue há 18 s". **Decisão
   pendente no §24.5.2:** trocar o rótulo (`visto há`/`atualizado há`), mudar a posição, ou somar a
   idade da entrega (`last_delivery.at`, que o snapshot já guarda) ao `último contexto`. **Fechada no
   [D-130](#d-130--duas-idades-no-detalhe-e-o-primeiro-prompt-sem-conteúdo-não-é-injetado): as duas idades, cada uma com nome.**
6. **Envelope sem itens conta em `inj` (6): a contagem está certa; a questão é se ele deve ser
   injetado.** O `UserPromptSubmit` da segunda rodada entregou `context_for_task · 0 items · ~186
   tokens` (`delivered: 0`), e a barra mostra `inj 1`. O envelope não era vazio: o transcript mostra
   `status: "unknown"` e uma limitação `route_uncertain` ("no trace, symbol, change or docs signal
   …; pass mode … or name a symbol"). Ele chegou ao modelo, e `record` (`src/hook.rs`) conta toda
   resposta que chega, como o §24 define `inj`; `delivered` soma itens, testes, riscos e notas, não
   limitações, por isso 0. **Questão aberta para os hooks:** o primeiro prompt injeta o que o
   `context_for_task` devolver, sem filtro, enquanto o PostToolUse só injeta com `has_news`. Um
   envelope só com limitações (~186 tokens para dizer "não achei nada") pode ser sinal útil ou ruído.
   **Fechada no [D-130](#d-130--duas-idades-no-detalhe-e-o-primeiro-prompt-sem-conteúdo-não-é-injetado): não é injetado.**

**Observações do roteiro:**

- O `refreshInterval: 1` do settings global é herdado pelo override local do `statusLine` e gerou um
  payload por segundo; `"refreshInterval": 3600` no override resolveu. O roteiro (passo 1) deve dizer isto.
- Logo ao abrir, a barra já mostrava `hooks on · última: pronta`, não `hooks sem dados`: havia um
  `Stop` gravado para a sessão. Coerente com o código; o roteiro pode citar.

**Fixture:** `tests/fixtures/statusline/claude_code.json`, o último payload da sessão (depois do 3.5),
em 1 espaço de indentação como as fixtures de hooks. `cwd`, `workspace.current_dir` e
`workspace.project_dir` viram `__WORKSPACE__`, `transcript_path` vira `__TRANSCRIPT__` e
`scratchpad_dir` vira `__SCRATCHPAD__`; sem caminho, nome de usuário ou e-mail. Os outros campos
(custo, `prompt_cache`, `rate_limits`, `session_name`) ficam como vieram, para o teste ler o payload
inteiro.

**Teste novo:** `the_captured_claude_code_payload_reads_its_own_projection` (`tests/statusline.rs`)
publica um snapshot para o `session_id` da fixture e roda `statusline` **sem** `--workspace`: a raiz
sai de `workspace.project_dir` e a barra é
`rw-brkr · Opus 5.5 mid · ctx 6% · hooks on · última: atenção · inj 7 · não reenviados 18`.
Os JSONs sintéticos existentes **não** foram trocados: `SONNET` e o de
`the_manual_example_of_the_spec` conferem os exemplos escritos na spec, e os outros testam bordas
(percentual fora da faixa, tipos errados, Unicode, agente) que um payload real não cobre.

**Contagem:** padrão **420 passados, 2 ignorados**; com `online` **434 passados, 4 ignorados**
(eram 419 e 433).

## D-129 — Edições pelo shell chegam ao hook de edição

**Data:** 2026-10-01. **Pedido do mantenedor.** Desenho em
[proposta-edicoes-por-shell.md](plan/proposta-edicoes-por-shell.md). Fecha a divergência 2 do
[D-128](#d-128--validação-manual-da-barra-e-fixture-de-payload-real): um `echo >> arquivo` pelo Bash
não chegava ao `context_after_edit`. Em TDD, com uma mutação por teste novo, como no D-127.

**O que mudou**

- O `PostToolUse` do Claude Code passa a casar `Bash` (`install` escreve
  `Edit|Write|MultiEdit|NotebookEdit|Bash`; o Codex não muda).
- Antes de subir o ripwire, `hook::run` tira uma impressão digital da árvore (`git status
  --porcelain=v1 -z --untracked-files=all` com `GIT_OPTIONAL_LOCKS=0`, mais `mtime` e `size` de cada
  arquivo sujo) e a compara com a salva pelo hook anterior. Só um Bash que mudou arquivos segue para o
  `context_after_edit`, com no máximo 50 arquivos (cortados depois do filtro de workspace); um Bash só de
  leitura fica em silêncio (sem saída nenhuma), não sobe o ripwire e não conta evento.
- Só contam os caminhos mudados dentro do workspace canônico (prefixo de `Path`, não de string): num
  workspace que é subdiretório do repositório, uma mudança fora dele não sobe o ripwire. A impressão
  continua cobrindo o repositório inteiro.
- `fingerprint` devolve `Result<Fingerprint, Unusable>`, com o motivo: `NotGit` (não é repositório,
  `git` ausente ou falhou), `TooSlow` (passou dos 500 ms das duas chamadas) ou `TooDirty` (mais de 5000
  entradas no `git status`). Todos dão silêncio, sem mensagem.
- **Cache negativo.** `TooSlow` e `TooDirty` ligam `SessionState.worktree_off` (salvo, omitido quando
  falso): pelo resto da sessão nenhum hook chama o `git`, e um Bash fica sem linha de base, em silêncio e
  sem ripwire; o gate do `Stop` continua cobrindo. Uma sessão nova tenta de novo. `NotGit` não liga a
  chave: o `git` falha rápido e sair do estado é barato. O custo num repositório lento fica limitado a um
  atraso de até 500 ms por sessão.
- A chamada do `git` remove `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE` e `GIT_COMMON_DIR` herdados
  (além de `GIT_OPTIONAL_LOCKS=0`): sob um hook do próprio git eles apontariam para outra árvore.
- A impressão fica em `SessionState.worktree` (salva); `shell_edits` (a lista do comando atual) nunca é
  salva.

**Mudanças e testes**

| # | Mudança | Teste | Mutação (pega) |
| --- | --- | --- | --- |
| 1 | `src/worktree.rs`: `fingerprint` e `changed` (T1) | 11 testes em `tests/worktree.rs`, entre eles `a_subdirectory_workspace_gets_paths_from_the_repository_root`, `odd_file_names_come_through_whole`, `a_rename_reports_the_new_name_and_skips_the_old_field`, `a_held_index_lock_does_not_stop_the_fingerprint`, `too_many_dirty_files_are_too_dirty` (antes `too_many_dirty_files_have_no_fingerprint`) | sem o segundo campo do `R`; `top.join` por `root.join`; sem `out.extend`; `!=` por `==` em `changed`; `> MAX` frouxo; `--untracked-files=no`; sem `-z`; `success().then_some` sem o filtro; `stamp` sempre `None`; `--ignored`: todas pegas. **Não pega:** tirar `GIT_OPTIONAL_LOCKS=0` (ver abaixo) |
| 2 | `respond` ramo `Bash`: usa `shell_edits` no lugar dos arquivos do evento, `mem::take`, corte em 50 depois de `in_workspace`; `SessionState.worktree` salva, `shell_edits` com `serde(skip)` (T2) | `a_shell_edit_gets_the_edit_context_for_the_files_run_found`, `a_shell_command_with_no_files_asks_nothing`, `a_large_shell_change_sends_at_most_the_cap`, `a_shell_deletion_still_reaches_the_edit_context`, `shell_edits_are_never_saved_and_the_fingerprint_is` | usar sempre os arquivos do evento; `clone` em vez de `mem::take`; sem o corte; `serde(skip)` por `serde(default)`: todas pegas |
| 3 | `run`: impressão antes do ripwire para todo evento do Claude Code menos o `Stop`; Bash sem mudança, opt-out ou sem git ficam em silêncio (T3) | `a_read_only_command_never_starts_ripwire`, `a_command_that_changed_a_file_goes_on_to_ripwire`, `an_edit_moves_the_baseline_so_the_next_command_is_not_blamed`, `without_a_baseline_nothing_is_blamed`, `opted_out_commands_stay_silent_and_still_move_the_baseline`, `outside_git_a_command_is_silent`, `stop_does_not_take_a_fingerprint` | tirar o `return None`; tirar `\|\| state.opted_out`; impressão só no Bash; `(None, Some(a))` como tudo mudado; `!= Event::Stop` por `true`: todas pegas. **Não pega** nos testes da CLI: tirar `state.shell_edits = changed;` (ver abaixo) |
| 4 | `install`: matcher do Claude Code ganha `Bash`; reinstalar sobre o matcher antigo troca o grupo, sem duplicar (T4) | `install_claude_code_merges_idempotently_and_keeps_foreign_keys` (asserção atualizada), `reinstalling_over_an_old_matcher_adds_bash` | os dois vermelhos antes da mudança; o matcher antigo de volta os derruba |
| 5 | Fixture `tests/fixtures/hooks/claude_code_post_tool_use_bash.json`, gravada do Claude Code 2.1.285 (caminhos, transcript e `scratchpad_dir` trocados por `__WORKSPACE__`, `__TRANSCRIPT__`, `__SCRATCHPAD__`) (T5) | `a_real_bash_payload_after_a_shell_edit_injects_the_edit_context` (ripwire real, o caminho inteiro) | tirar `state.shell_edits = changed;`: pega (saída vazia, `EOF`) |
| 6 | `worktree::Unusable` (`NotGit`/`TooSlow`/`TooDirty`, `switches_off`), `fingerprint_within(root, budget)`, verificação do prazo antes de cada `git` (revisão final) | `a_directory_outside_git_has_no_fingerprint` e `too_many_dirty_files_are_too_dirty` (adaptados), `a_spent_budget_is_too_slow` (novo) | `TooDirty` por `NotGit`; `switches_off` sempre `true`; prazo esgotado antes do `git` como `NotGit`: todas pegas |
| 7 | `SessionState.worktree_off` e o cache negativo em `run`; o estado é salvo quando a chave liga num Bash silencioso (revisão final) | `a_slow_git_switches_detection_off_for_the_session` (`git` falso no `PATH` do filho que dorme 2 s), `a_slow_git_on_a_first_shell_command_is_remembered`, `a_too_dirty_tree_switches_detection_off_for_the_session` (5001 arquivos, `git` real atrás de um contador) | timeout como `NotGit`; ignorar a chave (lento e sujo); não salvar quando só a chave mudou (pega só no teste do primeiro Bash; o do prompt salva pelo caminho do prompt): todas pegas |
| 8 | `run`: só caminhos sob o `root` canônico (revisão final) | `a_change_outside_a_subdirectory_workspace_stays_silent` (com controle positivo dentro de `sub/`) | tirar o `retain`: pega (saída `no context` de um ripwire ausente) |
| 9 | `git` sem `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_COMMON_DIR` herdados (revisão final) | `an_inherited_git_dir_does_not_redirect_the_fingerprint` (as quatro no ambiente do filho; espera a árvore limpa do workspace) | manter cada uma das quatro, uma por vez: todas pegas |
| 10 | Codex não tira impressão (revisão final, só teste) | `codex_events_never_take_a_fingerprint` (prompt e `PostToolUse` do Codex; `worktree` fica `None`, `worktree_off` falso) | tirar `args.host == Host::ClaudeCode`: pega |

Total: 11 + 5 + 7 + 1 + 1 = 25 testes novos na primeira rodada (o da linha 4 ao lado de uma asserção
atualizada), mais 7 na revisão final (linhas 6 a 10: 1 em `tests/worktree.rs`, 6 em `tests/cli.rs`),
32 ao todo. Os testes da CLI põem o `git` falso e as variáveis `GIT_*` só no ambiente do processo filho
(`run_with_env`), nunca no do processo de teste. Na linha 3,
`an_edit_moves_the_baseline_so_the_next_command_is_not_blamed` difere do roteiro do plano (ver abaixo).

**Decisões e desvios**

- **Desvio da spec §4.4.** A spec tirava a impressão "depois de decidir a resposta". Aqui ela sai em
  `hook::run`, antes de o ripwire subir, em todo evento do Claude Code menos o `Stop`. É equivalente,
  porque um hook nunca edita arquivos: a árvore é a mesma antes e depois da decisão, e tirá-la antes
  permite não subir o ripwire num Bash só de leitura.
- **Teste alterado na Tarefa 3.** `an_edit_moves_the_baseline_so_the_next_command_is_not_blamed`, do
  roteiro, ganhou um `shell("ls")` depois do prompt. Sem ele, a mutação "impressão só no Bash" passava
  por silêncio (sem linha de base, nada é atribuído, e o teste esperava silêncio).
- **Timeout ou falha do `git`** (500 ms) troca a linha de base salva por `None`; o próximo Bash que
  editar não tem comparação e não é atribuído (spec §5.2). Timeout e excesso de entradas, desde a
  revisão final, também desligam a detecção pela sessão (cache negativo acima).
- **Workspace num subdiretório do repositório, nomes com espaço/acento/aspas, `git mv`, `index.lock`
  presente e `rm`:** cada um tem teste (T1 e T2), por serem as entradas que a spec implica e não lista.

**Mutações que nenhum teste pegou**

- **Tirar `GIT_OPTIONAL_LOCKS=0`.** O teste do `index.lock` não a vê: com o lock retido, a escrita
  opcional do índice pelo `git` é um no-op silencioso, e numa fixture com o índice fresco não há o que
  atualizar. Pegá-la exigiria uma fixture de índice desatualizado sem lock. Fica registrada; a variável
  continua no código por prevenção.
- **Tirar `state.shell_edits = changed;` em `run`.** Os testes da CLI da Tarefa 3 não a pegam (sem
  ripwire, a execução falha antes de `respond`), mas o teste da Tarefa 5, com ripwire real, pega.

**Custo (Tarefa 6)**

Release, Apple M3, Darwin 27.0.0, 50 execuções, `--ripwire /nonexistent/ripwire` (o ripwire real
nunca rodou fora do repositório). Processo inteiro do hook para um Bash só de leitura, comparado ao
`git status` sozinho:

| Repositório | Hook p50 / p95 / máx | `git status` p50 / p95 / máx | Entradas sujas |
| --- | --- | --- | --- |
| ceci_app (2.503 arquivos rastreados) | 21,7 / 24,8 / 25,9 ms | 13,5 / 14,7 / 15,1 ms | 2 |
| ripwire-broker | 16,4 / 17,6 / 24,9 ms | 6,3 / 6,7 / 6,7 ms | 0 |

O critério passa no ceci_app, que é um repositório **médio**. O critério da spec é para um repositório
**grande**, e a primeira versão do PR #39 não o mediu. Medido depois, a pedido do mantenedor, num clone
raso do kernel Linux (96.049 arquivos rastreados, 13 entradas sujas pela colisão de maiúsculas do
macOS), ele **falhava**:

| Linux, antes | p50 | p95 |
| --- | --- | --- |
| hook, Bash só de leitura (processo inteiro) | 244,8 ms | 248,9 ms |
| `git status --untracked-files=all` sozinho | 233,5 ms | 240,6 ms |
| só os não rastreados (`ls-files -o`) | 175,1 ms | 180,3 ms |
| só os rastreados (`-uno`) | 68,5 ms | 70,0 ms |

O `git status` ficava abaixo do limite de 500 ms, então o cache negativo nunca disparava, e **todo**
hook da sessão pagava ~250 ms. Nem só os rastreados cabem nos 50 ms, e os aceleradores do próprio git
(`core.untrackedCache`, `core.fsmonitor`) exigem mudar a configuração do usuário ou escrever no índice,
o que o código evita de propósito.

**Desligar no portão (decisão do mantenedor).** Uma impressão que leva mais de `SLOW_FINGERPRINT`
(50 ms, o portão da spec) conta em `SessionState.slow_fingerprints`; com `SLOW_FINGERPRINTS_OFF` (2)
seguidas, a detecção se desliga pelo resto da sessão, como num timeout. A primeira lenta ainda é usada,
porque pode ser cache frio; uma rápida zera a contagem. Duas, e não uma, também evitam que um teste sob
carga desligue a detecção por acaso. Depois:

| Repositório | primeiros 3 hooks | Bash só de leitura seguintes (p50 / p95) | detecção |
| --- | --- | --- | --- |
| Linux | 641, 252, 3 ms | 2,7 / 3,3 ms | desligada (2 lentas) |
| ceci_app | 56, 26, 26 ms | 24,6 / 25,7 ms | ligada |

| Mudança | Teste | Mutação (pega) |
| --- | --- | --- |
| contar impressões acima de 50 ms; duas seguidas desligam; uma rápida zera | `one_fingerprint_over_the_gate_is_still_used`, `two_fingerprints_in_a_row_over_the_gate_switch_detection_off`, `a_fast_fingerprint_resets_the_slow_count` (um `git` real atrás de um `sleep 0.06`) | desligar com uma (`SLOW_FINGERPRINTS_OFF = 1`); não zerar; não salvar quando só a contagem muda; nunca lenta — todas pegas |

Com 100 ms de `sleep` no `git` falso, o teste das duas seguidas passava em paralelo pelo motivo errado
(o timeout de 500 ms disparava sob carga); com 60 ms ele só passa pela regra nova.

**Revisão do CodeRabbit no PR #39.** Três achados "Major", corrigidos antes do merge:

| # | Achado | Mudança | Teste | Mutação (pega) |
| --- | --- | --- | --- | --- |
| 1 | a linha de base não era ligada ao repositório: uma sessão que passasse do repositório A ao B veria a sujeira de B como edição | `Fingerprint.top` (raiz do repositório, `serde(default)` para estados antigos); `changed` devolve nada entre repositórios diferentes | `a_fingerprint_of_another_repository_is_no_baseline`; `a_clean_tree_has_an_empty_fingerprint` exige o `top` | tirar a comparação de `top` — pega |
| 2 | o prazo de 500 ms não valia durante os `stat` dos arquivos sujos | o laço confere o prazo a cada entrada e devolve `TooSlow` | nenhum determinístico (um sistema de arquivos lento não se simula com estabilidade); a regra dos 50 ms já mede a impressão inteira, `stat` incluído | tirar a conferência — **não pega** |
| 3 | um `git` (ou invólucro) que sai deixando um processo com o stdout aberto prendia o hook no `join` do leitor, sem prazo | o leitor manda o resultado por um canal, e o hook espera com `recv_timeout` até o prazo; o leitor que sobra é abandonado e termina quando o pipe fecha | `a_git_that_leaves_its_output_open_cannot_hang_the_hook` (um `git` falso com `sleep 5 &`: o hook volta em ~1 s, e não em ~6 s, e a detecção se desliga) | `recv()` sem prazo — pega (5,7 s) |

**Pendência aberta: `bashEditDiff`.** Na captura, o payload do `PostToolUse` do Bash no Claude Code
2.1.285 trouxe `tool_response.bashEditDiff` com `files[{filePath, hunks}]`, `moreFiles` e
`changedFiles` (caminhos absolutos): o próprio host diz quais arquivos o comando mudou. O mantenedor
decidiu manter a impressão do git nesta rodada. Investigar o `bashEditDiff` (semântica quando nada
mudou, `moreFiles`, estabilidade e documentação, a versão que o introduziu) fica como pendência: ele
pode substituir ou complementar a impressão, com atribuição exata e sem depender do git. **Fechada no
[D-131](#d-131--a-lista-do-próprio-claude-code-substitui-a-impressão-do-git): a lista do host vale quando existe, e a impressão fica de reserva.**

**Contagem:** `cargo test --all-targets --locked`: **451 passados, 2 ignorados**; com
`--features online`: **465 passados, 4 ignorados** (D-127: 419 e 433; 32 testes novos, 7 deles da
revisão final), somando todas as linhas `test result:`.
Com o D-128 (PR #38) mesclado no branch: **452 passados, 2 ignorados**; com `online` **466
passados, 4 ignorados** (o teste do D-128; a correção pedida pelo CodeRabbit no teste dele não muda a
contagem). Com o desligamento no portão de 50 ms: **455 passados, 2 ignorados**; com `online` **469
passados, 4 ignorados** (3 testes novos). Com as correções da revisão do CodeRabbit: **457 passados, 2 ignorados**;
com `online` **471 passados, 4 ignorados** (2 testes novos). CA-10
(`cargo tree --locked -e normal | grep -Ei 'reqwest|secrecy|rustls|hyper'`) sem saída.

## D-130 — Duas idades no detalhe, e o primeiro prompt sem conteúdo não é injetado

**Data:** 2026-10-01. **Pedido do mantenedor.** Fecha as divergências 5 e 6 do
[D-128](#d-128--validação-manual-da-barra-e-fixture-de-payload-real), com a opção que ele escolheu em
cada uma.

**Divergência 5 — as duas idades, cada uma com nome.** O `há …` do `--detail` era a idade do snapshot
(`updated_at`, gravado a cada hook), mas, logo depois de `último contexto`, lia-se como a idade da
entrega. Agora são dois segmentos: `último contexto ~523 tok há 290s` (de `last_delivery.at`) e
`visto há 18s` (de `updated_at`). `dados antigos` continua seguindo a idade do snapshot. O §24.5.2 e o
README mudaram junto.

**Divergência 6 — o primeiro prompt sem conteúdo não é injetado.** O primeiro prompt injetava o que o
`context_for_task` devolvesse; um envelope só com limitações (o `route_uncertain` da validação, ~186
tokens) custava tokens e não dava nada para agir. Agora, sem itens, testes, riscos nem notas
(`carries_content`), o hook fica em silêncio, como o PostToolUse sem `has_news`: nada é injetado, nada
conta em `inj`, nada entra no `hook-log`. A análise (`last_analysis`, `unknown`) é gravada antes, e a
barra ainda mostra `última: incerta`. O evento continua contando em `events`.

| Mudança | Teste | Mutação (pega) |
| --- | --- | --- |
| `último contexto … há <idade da entrega>` e `visto há <idade do snapshot>` | `detail_adds_delivered_reuse_last_context_and_age` (entrega a 970, snapshot a 1.000, agora 1.020: `há 50s · visto há 20s`), `token_counts_show_whole_below_a_thousand_and_drop_a_zero_decimal`, `age_counts_seconds_minutes_and_hours_and_a_future_stamp_is_zero` (vermelhos antes) | idade da entrega calculada do `updated_at` — pega |
| `UserPromptSubmit` sem conteúdo → silêncio, análise gravada | `a_first_prompt_with_only_limitations_injects_nothing_but_records_the_analysis` (vermelho antes: injetava) | sempre injetar — pega; tirar só o termo das notas de `carries_content` — **não pega** (nenhum teste monta um envelope só com notas) |

**Contagem:** padrão **458 passados, 2 ignorados**; com `online` **472 passados, 4 ignorados** (eram 457
e 471: 1 teste novo; os 3 do detalhe foram ajustados). CA-10 sem saída.

## D-131 — A lista do próprio Claude Code substitui a impressão do git

**Data:** 2026-10-01. **Pedido do mantenedor** ("investigar, depois preferir"). Fecha a pendência
`bashEditDiff` do [D-129](#d-129--edições-pelo-shell-chegam-ao-hook-de-edição).

**Investigação.** Sete payloads reais do `PostToolUse` do Bash (Claude Code 2.1.285, um comando por
chamada, numa sessão do workspace de teste):

| Comando | `bashEditDiff` | `changedFiles` | `files` (com hunks) | `moreFiles` |
| --- | --- | --- | --- | --- |
| `cat src/lib.rs` (só leitura) | ausente | — | — | — |
| `echo x > /tmp/…` (fora do workspace) | ausente | — | — | — |
| `git diff …; cp … /private/tmp/…` (escrita fora do workspace) | ausente | — | — | — |
| `git checkout -- src/lib.rs` (reverter) | presente | 1 | 1 | 0 |
| `rm novo.txt` (apaga um não rastreado) | presente | 1 | 1, com `"deleted": true` | 0 |
| criar 60 arquivos | presente | **60 (todos)** | 5 | 55 |
| `rm gen_*.txt` (apagar os 60) | presente | **60 (todos)** | 5 | 55 |

A captura do D-129 mostrou também uma edição de arquivo rastreado (`echo >> src/lib.rs`, 1 caminho).
Conclusões: o campo só aparece quando algo mudou **dentro do workspace**, e nunca vem vazio;
`changedFiles` traz **todos** os caminhos, absolutos (o `files` com diff para em 5, e `moreFiles` conta o
resto); reverter, apagar e criar não rastreado contam. O campo não é documentado, e a ausência não
distingue "nada mudou" de "versão sem o campo".

**Desenho.**

- O primeiro payload de Bash com `bashEditDiff` grava `SessionState.host_reports_bash_edits` e apaga a
  impressão guardada.
- Com a marca: o campo presente dá os arquivos editados (`changedFiles`, cortados ao workspace e a 50,
  sem chamar o `git`); o campo ausente é "nada mudou" (silêncio, sem ripwire, sem evento). Nenhum hook da
  sessão tira impressão: o custo do `git` vai a zero, em qualquer tamanho de repositório.
- Sem a marca (versões que não mandam o campo, ou antes do primeiro comando que mude algo): a impressão
  do D-129 continua, com os mesmos limites e desligamentos.
- O corte ao workspace aceita o caminho como o host o escreve e a raiz canônica (no macOS, `/var/…` e
  `/private/var/…`).

**Custo, se a hipótese falhar:** se o host omitir o campo numa mudança real do workspace (um caso que a
captura não mostrou), essa edição fica sem contexto no meio do turno; o gate do `Stop` continua vendo.

| Mudança | Teste | Mutação (pega) |
| --- | --- | --- |
| `host_bash_edits` lê `changedFiles` | `the_hosts_own_list_of_changed_files_is_used` (fixture real de 60 arquivos, nada mudou no disco: só a lista do host faz disso uma edição) | campo nunca lido — pega (3 testes) |
| com a marca, sem o campo: silêncio e nenhum `git` | `once_the_host_reports_a_command_without_the_field_changed_nothing_and_git_rests` (`git` falso com contador) | `git` continua rodando — pega (3); impressão mantida — pega |
| lista cortada ao workspace; a marca é salva mesmo quando a resposta é silêncio | `the_hosts_list_is_cut_to_the_workspace` | sem o corte — pega; marca não salva no silêncio — pega |
| ripwire real com a lista do host | `a_real_bash_payload_naming_its_changed_file_injects_the_edit_context`; o teste do D-129 tira o campo do payload e cobre a impressão de reserva | — |

Fixture nova: `tests/fixtures/hooks/claude_code_post_tool_use_bash_many.json` (o comando dos 60
arquivos, com `__WORKSPACE__`, `__TRANSCRIPT__` e `__SCRATCHPAD__`, sem dado pessoal).


**Contagem:** padrão **461 passados, 2 ignorados**; com `online` **475 passados, 4 ignorados** (eram 457
e 471: 4 testes novos). Com o D-130 (PR #40) mesclado: **462 passados, 2 ignorados**; com `online`
**476 passados, 4 ignorados**. CA-10 sem saída.

## D-132 — Documentação alinhada ao D-131

Revisão da documentação e do diagrama depois do D-128 a D-131. Nenhuma mudança de código.

- **`integrations/claude-code/settings.json`:** o matcher do `PostToolUse` era
  `Edit|Write|MultiEdit|NotebookEdit`. Desde o [D-129](#d-129--edições-pelo-shell-chegam-ao-hook-de-edição)
  o `install` grava `Edit|Write|MultiEdit|NotebookEdit|Bash`; quem copiasse o exemplo, que o README
  indica, ficaria sem as edições feitas pelo shell. O exemplo agora é igual ao do `install`. O do
  Codex já batia.
- **PRD:** o cabeçalho, o roadmap (§19) e o §24 diziam que a validação manual e a fixture real
  estavam pendentes. Agora apontam para o D-128 e dizem como cada divergência foi fechada. O campo
  `agent` (§24.8) fica registrado como ainda não visto num payload real.
- **`handoff.md`:** estado até o D-131, contagem de testes (462 e 476), o `src/worktree.rs` em
  "Onde está o quê", e o campo `agent` como pendência nova.
- **Diagrama (`spec/diagrams/`):** ele parou antes da barra de status
  ([D-115](#d-115--diagrama-de-arquitetura-versionado)). Ganhou dois nós e três ligações:
  - **Status line** (`src/statusline.rs`, `src/statusline_state.rs`): o host manda o JSON do
    `statusLine`, e o comando só **lê** o estado que os hooks escrevem (tracejado: não chama o
    broker nem o ripwire);
  - **git** (`status --porcelain`): o hook pergunta se um comando do shell mudou arquivos (D-129).
    A ligação omite o caminho do D-131, em que a lista vem do próprio host e o `git` não é chamado;
  - o estado de sessão passa a citar `src/statusline_state.rs`, o hook cita `src/worktree.rs`, e a
    ligação entre os dois vira `writes`;
  - uma visão guiada nova, **Status bar**, e a do modo automático inclui o `git`.

  As referências agora apontam para o commit `db84cf3` (o `guarded_call` desceu da linha 708 para a
  715). Para caber em 1440×900 sem rolagem, as faixas foram aproximadas em vez de o texto encolher.
  A primeira versão falhou nisso: com 110 px a mais de altura, a página rolava 73 px.

  Verificação: `validate --quality showcase` com 9/9 checagens, 0 erros, 0 avisos e 0
  cruzamentos. `deliver`: especificação sha256 `d9e3c1fd…` (9429 bytes), HTML `3e343a37…`
  (725607 bytes), 21 referências de fonte verificadas. `visual-check` no Chrome, numa cópia com o
  mesmo sha256: sem rolagem em 1440×900, 1600×1000, 1920×1080 e 2048×1320, menor texto projetado de
  6,6 px a 1440 (mínimo 6). As capturas não foram versionadas, como no D-115.
- **Correção de uma afirmação minha:** eu disse que o archify não estava instalado nesta máquina.
  Estava, mas como skill em `~/.agents/skills/archify/`, fora do `PATH` e dos plugins onde procurei.
- **Planos (`spec/plan/`) não foram mexidos:** são o registro do que foi planejado, não o estado.

## D-133 — Auditoria da documentação contra o código

**Data:** 2026-10-02. Pedido do mantenedor: verificar se a documentação está desatualizada em relação
ao código. Nenhuma mudança de código.

**Método.** Cada flag do `--help` dos dois binários procurada no README e no PRD; as variáveis de
ambiente lidas pelo código; os parâmetros e orçamentos das tools em `src/mcp.rs`; os tipos de
limitação e de `source.basis` citados no skill; as constantes de `src/hook.rs`, `src/worktree.rs`,
`src/statusline.rs` e `src/online/` contra os números do README e do §8.4; as fixtures em
`tests/fixtures/`; e a contagem de testes (padrão **462 passados, 2 ignorados**; com `online`
**476 passados, 4 ignorados**, igual ao `handoff.md`).

**O que estava faltando ou desatualizado:**

- **`--edit-interval-ms` não aparecia em lugar nenhum fora do changelog.** O README e o §8.4 diziam
  que cada edição recebe `context_after_edit`, mas desde o
  [D-106](#d-106--uma-rajada-de-edições-é-uma-pergunta-não-uma-por-edição) uma edição dentro da
  janela (1000 ms) não pergunta nada e os arquivos vão com a próxima; só os 32 primeiros arquivos
  distintos da rajada são guardados, e os demais ficam para o `Stop` (o comentário de
  `MAX_HELD_EDITS` dizia "as mais recentes", mas o código guarda as primeiras; o comentário foi
  corrigido no PR seguinte). Os dois ganharam o parágrafo,
  com os orçamentos dos hooks (1500 no prompt, 800 depois de uma edição), que também não estavam lá.
- **README, fixtures:** dizia que as de `tests/fixtures/hooks/` vinham do Claude Code 2.1.283. As
  duas de `Bash` vêm da 2.1.285 (D-129, D-131), e não citava `tests/fixtures/statusline/` (D-128) nem
  `tests/fixtures/eval/` (sintética).
- **README, `ripwire-eval`:** `--broker`, `--ripwire`, `--timeout-s` (1800) e `--check-timeout-s`
  (600) só estavam no `--help`.
- **README:** `prompt --budget N` e `--jev-provider typesafe` (este já estava no §23.6).

**Conferido e sem mudança:** variáveis de ambiente, versão mínima do ripwire (0.6.4), toolchain
(1.98.1), orçamentos e pisos das tools (2500/1500/1800, 256 e 512 com `--online`), limites do modo
online, da barra de status e da impressão do git, os exemplos de `integrations/` e o skill. Flags que
só o README documenta (`--codex-home`, `--redact-workspace`, `--log-refs`, `--summarizer-timeout-ms`)
ficam assim: o PRD não é referência de CLI.

**Nota de método:** o primeiro laço que comparou as flags usou `for f in $flags` no zsh, que não
divide a variável, e respondeu "nada faltando". É a armadilha já listada no `handoff.md`.

## D-134 — Plano de implementação do `--memory`

**Data:** 2026-10-03 12:36.

**Decisão:** o PRD [`docs/jev-mem-prd.md`](../docs/jev-mem-prd.md) (v0.2) ganha um plano de
implementação, [`spec/plan/jev-mem-plan.md`](plan/jev-mem-plan.md). Nenhum código de produção entra
com ele.

**O que o plano fixa:**

- **TDD por tarefa, como condição de "feito":** teste vermelho na costura pública, visto falhar
  pelo motivo esperado; o menor código que o faz passar; mutação que prova que o teste morde; a
  documentação afetada; os portões locais. Só então a tarefa é marcada, com a linha dela no
  registro de evidência.
- **Seis fases**, na ordem do PRD §15: contratos; store e coleta (build padrão, sem rede);
  controle Jev (feature `online`); leitura e host; consolidação; avaliação; e um fechamento com
  auditoria e incorporação ao PRD principal. Cada critério verificável do PRD §14 tem tarefa.
- **Fica em `spec/plan/`**, com os planos das outras fases; o PRD que ele implementa continua em
  `docs/`, por ainda ser proposta.

**Achados da validação contra o código** (tabela V1 a V16 do plano). Os que mudam o que o PRD
supõe:

- o estudo que o PRD §1 dá como ausente está no checkout (`docs/jev-mem.md` e o PDF, com o mesmo
  SHA-256); a reconciliação é a primeira tarefa;
- `memory drain --online` colide com o D-064 (`--online` só no `serve`);
- o PRD não diz como o hook liga a coleta, nem o que reativa a coleta depois de `forget --all`;
- o texto MCP hoje é o próprio JSON do envelope (`src/mcp.rs`), então a seção legível do PRD §11
  muda o bloco de texto;
- o portão de fixtures do CI recusa strings com cara de credencial: os dados sensíveis dos testes
  de admissão são montados dentro do teste, não em arquivo.

**Pendente do mantenedor:** PD-1 a PD-5 do plano. Cada uma bloqueia só a tarefa indicada.

**Verificado:** linha de base no `master` em `60c77b9`: `fmt`, `clippy` (padrão e `online`),
462 testes (2 ignorados) e 476 com `online` (4 ignorados), CA-10 e a guarda de fixtures, tudo
verde. O teste de exemplo do plano (T1.1) foi compilado contra o código e falha pelos motivos que
o plano declara; `tests/cli.rs` não mudou.

## D-135 — Decisões PD-1 a PD-5 do `--memory`

**Data:** 2026-10-03 12:50.

**Decisão:** o mantenedor aceitou as cinco propostas que o plano
([D-134](#d-134--plano-de-implementação-do---memory)) deixou pendentes.

| ID | Decisão | Tarefa do plano |
| --- | --- | --- |
| PD-1 | `memory add --workspace PATH --file PATH` entra neste incremento, na Fase 1, local e sem rede, como no PRD jev-mem §5.2 | T1.13 |
| PD-2 | `--online` passa a valer para `serve` **e** `memory drain`, e só para eles; é uma exceção explícita ao [D-064](#d-064--cache-diagnóstico-e-integração-proposta), não a revogação dele | T2.11 |
| PD-3 | O hook liga a coleta com `hook … --memory`: publica no spool local, nunca faz HTTP e não implica `--online`; o instalador grava a flag só com `install --memory` | T1.15, T1.16 |
| PD-4 | Depois de `forget --all`, um marcador `revoked` no store impede a coleta; só o comando novo e local `memory resume --workspace PATH` o remove | T1.12 |
| PD-5 | A Fase 2 (controle Jev) pode começar antes de fechar as medições do `--online`, atrás de `--memory` e declarada experimental | Fase 2 |

**Consequência:** nenhuma tarefa do plano fica bloqueada por decisão. As superfícies novas
(`memory add`, `memory resume`, `hook --memory`, `install --memory`, `memory drain --online`) ainda
não estão no PRD jev-mem §4: levá-las para lá é a T0.2 do plano. Nenhum código mudou.

## D-136 — Fase 0 do `--memory`: PRD jev-mem v0.3

**Data:** 2026-10-03 13:05.

**Decisão:** as duas tarefas da Fase 0 do [plano](plan/jev-mem-plan.md) estão feitas, sem código.
O PRD [`docs/jev-mem-prd.md`](../docs/jev-mem-prd.md) passa à v0.3.

**T0.1, estudo reconciliado.** A v0.2 dava o estudo e o PDF como ausentes. Os dois estão no
repositório, e o PDF tem o mesmo SHA-256 da cópia lida (`413c5924…5e87`). O estudo
[`docs/jev-mem.md`](../docs/jev-mem.md) foi confrontado seção por seção com o PDF (texto extraído,
Tabelas 1–2, Appendix B), com o código e com o PRD. O resultado é o novo §2.4 do PRD:

- **O que confere:** os números do LoCoMo e os baselines de cada um (+11,0% e −36,7% sobre MAGMA,
  6,6× sobre Nemori), o algoritmo de escrita, consolidação e leitura com todos os limiares e
  limites, a descrição do `--online` atual (D-046, `src/notes.rs`, `src/broker.rs`), a
  impossibilidade de o servidor MCP chamar o modelo da sessão e as referências internas.
- **Duas imprecisões do estudo:** a consolidação é a cada 20 escritas Jev *bem-sucedidas*, e o
  timestamp do paper é de observação, não do evento (Appendix B.1). O PRD já tratava as duas
  (§9, §5.4).
- **Seis divergências, todas decididas a favor do PRD:** a flag fora do `serve` (D-135); a
  consulta e a preferência tiradas do prompt, que o PRD não persiste (§5.2, §10); sqlite e o hash
  do workspace sem worktree (§3, §5.1); tempo por mtime/commit (§5.4); `role: memory` (§11);
  `--memory` *exigir* `--online` em vez de implicá-lo, e a comparação com Mem0 (§4, §14).
- **Dois pontos do §2.2 atacavam a transcrição, não o estudo:** ele diz que um System One local
  não existe *no broker*, o que é verdade, e registra a escrita sem filtro como a do paper.

O §1, a última linha do §2.2, o §15 e o §17 deixam de falar em pendência.

**T0.2, superfícies do D-135 no PRD.** A tabela do §4 ganha `memory add`, `memory resume`,
`hook … --memory` e `install … --memory`, e a linha de `memory drain` diz que é a exceção ao
[D-064](#d-064--cache-diagnóstico-e-integração-proposta). O §6 nomeia o marcador `revoked`; o
CA-2 e o §16 passam a cobrir os quatro subcomandos locais e as duas portas de `--online`. A
pendência que o D-135 deixou para a T0.2 está fechada.

**Verificado:** os testes vermelhos do plano falham no PRD v0.2 (`grep "não foram encontrados"`
encontra; o `grep` das superfícies novas não encontra nada) e passam na v0.3. Portões locais
verdes; nenhum arquivo de código mudou.

## D-137 — Fase 1 do `--memory`: store e coleta

**Data:** 2026-10-03 15:12.

**Decisão:** a Fase 1 do [plano](plan/jev-mem-plan.md) está feita, uma tarefa por commit, cada
uma com o teste vermelho visto falhar, o código mínimo, mutação que derrubou o teste, a
documentação afetada e os cinco portões verdes. O registro de evidência do plano (§9) tem a
linha de cada tarefa. Nada de rede: tudo no build padrão, e o CA-10 continua valendo.

**O que entrou:** `--memory` (implica `--online`) e as cinco opções `--memory-*` com faixas;
`src/memory/` com o registro `memory/v1` e seus limites, a identidade (workspace com git-dir e
common-dir, entidades, `content_hash`, `node_id`), a admissão com o renderizador
`memory-observation/v1`, o relógio injetável e a sequência, e o store
(`<state-dir>/memory/<workspace_id>/`, 0700/0600) com spool, snapshot durável, tetos, lock de
escritor, retenção, `forget` com tombstones e o marcador `revoked`; os comandos locais
`memory status|forget|add|resume`; a coleta depois do envelope em `context_after_edit` e
`context_before_finish`; `hook … --memory`; `install --memory` e a linha `memory` do `doctor`.
Testes: 462 → 512 no build padrão, 476 → 526 com `online`.

**Decisões tomadas no caminho:**

- **A T1.7 veio antes da T1.6.** O vermelho da T1.6 ("mtime e commit não mudam o registro")
  precisa do construtor de observações, que é a T1.7.
- **`ingest_seq` é dado na incorporação, sob o lock**, não no hook: o hook não espera lock. O
  `content_hash` já excluía a sequência, então a identidade não muda.
- **PII é só e-mail.** Uma regra de sequência longa de dígitos recusaria nomes de migração
  (`20260101120000_create.exs`), comuns no corpus do A/B. Segredos: prefixos conhecidos
  (`sk-`, `AKIA`, `ghp_`…), JWT e atribuições (`password=`…). Conservador, sem promessa de pegar
  tudo, como o PRD §5.2 pede.
- **A tombstone do `memory forget` dura 365 dias**, a maior retenção permitida: o comando não
  sabe com que retenção o servidor roda. O hook guarda com a retenção padrão de 30 dias, porque
  não recebe `--memory-retention-days`.
- **O marcador `revoked` é gravado antes de apagar** no `forget --all`: uma queda no meio nunca
  deixa a coleta ligada. Ele vale contra `--memory` depois de reiniciar (achado do CodeRabbit no
  PR da Fase 0, levado ao PRD §6).
- **A coleta espera no máximo 25 ms** e nunca mais de 4 escritas em voo; o que passa disso conta
  como `unconfirmed`, nunca como durável. Uma resposta `unknown` não vira observação.
- **O `serve --memory` ainda não liga a coleta das tools.** Ligar no binário pede credencial e
  uma sessão MCP de ponta a ponta para testar; vai para a T2.11, junto com o worker. Nesta fase o
  binário coleta pelos hooks; as tools coletam no núcleo do broker, que está testado.
- **Os testes de processo do hook ficaram em `tests/cli.rs`**, ao lado da infraestrutura e2e, e
  rodam só com o ripwire real (pulados sem ele, como os demais e2e). Os testes em processo
  (`tests/hooks.rs`) rodam no CI; dois deles nasceram verdes, porque o hook já usa o broker da
  T1.14, e ficaram como caracterização, com mutação provando que mordem.
- **`integrations/` não mudou:** nada novo chega ao agente nesta fase. A T3.11 atualiza a skill
  quando `memories[]` existir.

**O que ficou sem teste, e por quê:** a comparação de uid do dono do store (exige root para criar
um arquivo de outro usuário) e o `sync` do diretório (exige queda de energia). Um mutante
equivalente: o git-dir no `workspace_id`, que é determinado pela raiz e pelo common-dir; fica
porque o PRD §5.1 o nomeia.

**SLO do hook (PRD §8.2), medido à mão em release com o ripwire real:** 60 pares alternados, p95
+6,9 e +4,1 ms, p99 +3,6 e −0,9 ms sobre um hook de cerca de 80 ms. A primeira medição, sem
alternar a ordem, deu −97 ms: o primeiro hook depois de uma mudança paga o aquecimento do
ripwire. Teste `#[ignore]` `the_hook_overhead_meets_the_slo`.

**Nota de método:** a família `shell_edits` de `tests/cli.rs` (prazos de 50 e 500 ms, D-129)
falhou três vezes nos portões desta fase, sempre sob carga, e passou em todas as repetições
isoladas, inclusive seis rodadas da suíte `cli` com e sem os testes novos de hook. É a
armadilha do `handoff.md`; os testes desta fase não a causam, mas aumentam a carga do binário
`cli`. Se aparecer no CI, a folga desses testes é o lugar de mexer.

**Auditoria do fim da fase (plano §7):** portões verdes; propriedades com 4096 casos (39) e
`props_fs` (10) verdes; CA-10 sem crate de rede; guarda de fixtures verde; nenhum `reqwest`,
`secrecy`, `println!` ou `unsafe` em `src/memory/`.

**Revisão independente do diff (plano §7, item 6), feita por um agente revisor só de leitura:**
11 achados, 3 médios. Dez corrigidos em TDD (teste vermelho, correção, mutação que derrubou):

- **Médio — `forget --all` perdia a corrida para um `enqueue` em curso:** o hook passava pela
  checagem do marcador, o `forget --all` listava e apagava o spool, e o arquivo do hook aparecia
  depois, sem tombstone. Agora o `enqueue` confere o marcador de novo depois de escrever e retira o
  que escreveu (`an_enqueue_racing_forget_all_never_survives_it`, com o ponto de injeção
  `Store::enqueue_with`).
- **Médio — temporários de escritores mortos ficavam para sempre,** fora dos tetos e fora do
  `forget --all` (o PRD §6 pede excluí-los). Agora contam no espaço do spool, a incorporação apaga
  os com mais de 60 s, e o `forget --all` apaga todos, também os do snapshot.
- **Médio — `forget --all` sobre um snapshot ilegível revogava e deixava o spool:** agora o spool
  sai antes de ler o snapshot, que continua intocado.
- **Baixo:** a mensagem do `--online` sem chave ganhara o prefixo `--online:` (saída mudada sem
  `--memory`; agora só `--memory` ganha prefixo); `memory add --file note.json` relativo falhava;
  o arquivo do `memory add` dentro do workspace só tinha o nome checado (agora o caminho inteiro
  passa pela política, então `.private/` e `target/` são recusados); uma escrita que entrasse em
  pânico prendia sua vaga em voo para sempre (agora um guarda devolve a vaga, e a reserva é
  atômica); uma entrada ruim no spool (link, diretório) travava toda incorporação (agora é
  descartada sem ser seguida); uma entrada de schema mais novo era apagada (agora fica); um
  `spool/` trocado por link era escrito através dele (agora é recusado).
- **Não corrigido, registrado:** a estimativa de tamanho do snapshot usa os bytes do spool (com
  `ingest_seq` 0), as tombstones não têm teto, e `forget`/`sweep` não checam o teto do snapshot.
  Nenhum dos três é alcançável com os limites padrão (2.000 × 16 KiB contra 64 MiB); entram na
  Fase 2, que introduz arestas e jobs e já revisita o tamanho do snapshot.

## D-138 — Fase 2 do `--memory`: controle Jev

**Data:** 2026-10-03 17:00.

**Decisão:** a Fase 2 do [plano](plan/jev-mem-plan.md) está feita em TDD, uma tarefa por commit, com
o teste vermelho visto falhar, mutação que o derrubou, documentação e os cinco portões. O registro
de evidência (§9 do plano) tem a linha de cada tarefa. A T2.0 rodou à mão com a chave real
(o mantenedor a pôs no `.env`, fora do git): `jev-1.13.0` respondeu um Choice no formato do PRD §7,
aceito pelo parser, escolhendo `after` com probabilidade 1.0 em 312 ms (conteúdo inventado). O
contrato do Choice deixa de ser só documentado.

**O que entrou:** `JevQuestion::choice` e `StateRequest` (estado estruturado qualquer); respostas
tipadas `Decision::{Noul, Choice, Unknown}` com id repetido detectado e Choice validado sem
renormalizar; um `post` comum no `JevClient` para descoberta e memória (`MemoryClassifier`);
`memory-prompts/v1` com as oito etapas; a fila (`memory::queue`) com um job por nó, leases presos a
lock de arquivo, duas execuções e a quota de 24 h no snapshot; o controlador (typing, até K
candidatos determinísticos, relações por par, arestas com limiar 0,60 e direção causal); commit
atômico por par, geração vista no commit, contador de enriquecimento uma vez por nó; falhas do
provider (401/403 suspendem, 429 só dentro do prazo, um retry, quatro tentativas); um teto de
requisições em voo por processo (`online::classifier::Shared`) e um job remoto por workspace; o
worker no `serve --memory` (que agora liga também a coleta das tools, adiada da T1.14) e
`memory drain --online`; o custo por operação no status e a quota em uso no `memory status`.
Testes: 519 → 562 no build padrão, 533 → 578 com `online` (com as correções da revisão).

**Decisões tomadas no caminho:**

- **O controlador compila no build padrão.** `online::request`, `response` e `classifier` já
  compilavam sem a feature (V15); só o `JevClient` a exige. Assim o domínio é testado nos dois
  builds e o CA-10 continua valendo. O plano previa `controller.rs` sob `cfg(feature = "online")`.
- **O formato da resposta Choice segue o PRD §7**: `{type: "choice", choice, probabilities,
  confidence}`, confirmado contra o provider pela T2.0.
- **As chaves do estado saem em ordem alfabética** (`serde_json` sem `preserve_order`): deterministas,
  o que serve à chave de cache futura. Perguntas e critérios mantêm a ordem dada.
- **Lease por lock de arquivo, não por PID**: `leases/<id>.lock`; quem consegue o lock de um job
  "leased" sabe que o dono morreu. Um lease abandonado na segunda execução falha o job.
- **Candidatos:** quem compartilha entidade, quem compartilha palavra e a observação anterior mais
  próxima; ordem por entidades, palavras, distância na sequência e id. Alias só se pergunta quando
  os ids não se cruzam; tempo implícito só quando os dois lados têm referência temporal.
- **Um par conta inteiro ou não conta:** uma decisão desconhecida descarta todas as inferências do
  par; a aresta determinística (entidade compartilhada) fica.
- **Lotes divididos renomeiam o candidato**: as perguntas citam `candidates[i]` com o índice dentro
  do pedido, não o global.
- **O teto do processo é um classificador `Shared`** com semáforo, que envolve o `JevClient` e é
  dado à descoberta e ao worker. O `Scheduler` não mudou.
- **Os testes de ciclo de vida são de biblioteca** (`memory::runtime::from_serve`, `Runtime`,
  `drain`): um teste do binário com `serve --memory` faria o worker chamar o provider real.
- **401/403 suspendem até um novo processo**, que é como a credencial muda.

**O que ficou de fora, registrado:** cache de decisões (o PRD §7 o prevê; nada de cache ainda, então
nada a contar como acerto); arestas temporais determinísticas por sequência de ingestão; a estimativa
de tamanho do snapshot e o teto de tombstones (do D-137) continuam inalcançáveis com os limites
padrão; o diagrama de `spec/diagrams/` não foi atualizado, porque o archify é um skill da máquina
do mantenedor, fora desta sessão.

**Auditoria do fim da fase (plano §7):** portões verdes; propriedades com 4096 casos (41) e
`props_fs` verdes; CA-10 sem crate de rede; guarda de fixtures verde; nenhum `reqwest`, `secrecy`,
`println!` ou `unsafe` em `src/memory/`.

**Revisão independente do diff (plano §7, item 6), por um agente revisor só de leitura:** 12
achados, 2 altos. Dez corrigidos em TDD (teste vermelho, correção, mutação que derrubou):

- **Alto — a quota de 24 h esgotada falhava todos os jobs em um minuto:** cada recusa gastava uma
  execução, e nada recuperava um job `Failed`. Agora uma tentativa que nada enviou (quota esgotada
  ou store ocupado) devolve o job com `Outcome::Defer`, sem gastar a execução; e `memory retry`
  é a ação explícita do PRD §8.2 que devolve as execuções aos jobs falhos.
- **Alto — um lock ocupado perdia respostas pagas:** `charge`, `commit_enrichment` e `finish`
  falhavam na hora com `Locked`. Agora esperam até 2 s (`Store::writer_waiting`); o escritor comum
  continua sem esperar, e o hook nunca toma o lock.
- **Médio:** o prazo de 5 s valia só para a espera de um 429 (agora limita toda tentativa); depois de
  um 429 fora do prazo os outros lotes seguiam (agora param); uma falha de transporte nas relações
  concluía o job e o contava como enriquecido (agora o job volta, as relações ficam devidas, e o
  contador sobe no `finish(Done)`, uma vez, porque `Done` é terminal); o `--online` sozinho passara a
  dividir um teto de processo com a descoberta (agora `classifier::for_process` só junta os dois com
  `--memory`); o commit não respeitava os tetos de 32.000 arestas e do snapshot (agora respeita, e
  descarta as arestas novas que transbordariam, guardando o resto da resposta).
- **Baixo:** o `drain` dizia "vazio" quando um servidor segurava o worker ou o provider recusava a
  chave (agora `Busy` e `Suspended`, com saída de erro) e ignorava o modelo e o K do servidor (agora
  `--jev-model` e `--memory-write-candidates`); os bytes de resposta da memória entravam no
  `jev_response_bytes` da descoberta (agora não); a documentação do `drain` dizia que um job cortado
  fica pendente sem gastar execução (corrigida: a execução conta).
- **Não corrigidos, registrados:** um salto do relógio para a frente congela a quota pelo tamanho do
  salto mais 24 h (é o outro lado de "relógio atrasado não libera quota"; distinguir os dois exige
  uma referência de tempo confiável que o broker não tem); a E/S de disco do worker roda nas threads
  do tokio e cada cobrança regrava o snapshot (tirar o livro-razão do snapshot é uma mudança de
  formato; fica para quando o tamanho do store pesar).
