# Plano — avaliação A/B e `session_hits` em uso real

Status: **instrumentos entregues; medições reais pendentes (seção 4)** · 2026-09-30 23:22, entregue 23:36 · decisão
[D-116](../changelog.md#d-116--plano-da-avaliação-ab-e-de-session_hits-em-uso-real)

Fonte: PRD §16.2–16.4, §17, §21.3 e §23.15. São os itens 1 e 2 da revisão de pendências: as
duas coisas que o projeto ainda **não mede**.

## 0. O que este plano entrega, e o que não pode entregar

Os dois itens são **medições**, não funcionalidades. Nenhum dos dois termina com um commit:

- o A/B termina quando um agente real rodar ≥ 30 tarefas em 3 repositórios, nos braços do
  PRD. Isso custa dinheiro de API, leva horas e depende da escolha dos repositórios, que é
  do usuário;
- o `session_hits` termina quando houver alguns dias de hooks em uso real.

O que dá para entregar agora é o **instrumento**, de um jeito que a medição real só dependa
de rodar um comando. As duas lacunas de hoje são concretas e testáveis:

1. **Não existe instrumento de A/B.** Não há formato de corpus, executor, extração de
   métricas do transcript nem cálculo das barras. Ninguém consegue rodar o A/B sem escrever
   tudo isso antes.
2. **`session_hits` não é observável no uso real.** Os hooks são o uso real (D-031). Cada
   evento de hook é um processo novo, e `metrics.session_hits` vive na memória desse
   processo e morre com ele. O estado em disco (D-032) guarda as impressões digitais, mas
   não o contador. Então a medição que o §21.3 pede é **impossível** hoje, não só pendente.

## 1. Item 2 — `session_hits` que sobrevive ao processo

### Desenho

- `SessionState` ganha `stats: SessionStats` com `#[serde(default)]`. Os campos são só
  contagens: `events`, `injections`, `delivered`, `session_hits` e `started_at` (segundos).
  Um arquivo de estado antigo carrega com zeros, sem migração.
- `hook::handle` soma ao estado a diferença de `Broker::session_hits()` antes e depois do
  evento. A conta continua sendo a do broker (`src/broker.rs`), sem fórmula duplicada.
  `Broker::session_hits()` é a única adição ao núcleo: um getter síncrono, porque `status()`
  é assíncrono e pode sondar o upstream.
- `delivered` conta o que chegou ao modelo inteiro: itens que não são referência, testes,
  riscos e notas das respostas injetadas. É o denominador: taxa de acerto =
  `hits / (hits + delivered)`.
- Comando novo `hook-stats [--state-dir DIR] [--json]`. Ele lê todos os estados do
  diretório e agrega:
  - **dentro da sessão:** sessões, eventos, injeções, entregues, hits e taxa. É o que a
    deduplicação por sessão já economiza;
  - **entre sessões:** quantas impressões digitais de uma sessão já estavam numa sessão
    anterior (ordem por `started_at`). É uma estimativa do que um cache **persistente**
    economizaria a mais, e é exatamente a pergunta do §21.3. Não exige estado novo: as
    impressões digitais já estão em disco desde o D-032.
- Privacidade (§16.1): o relatório só tem contagens. Nenhum caminho, símbolo, prompt ou id
  de sessão aparece, nem mesmo o hash.

### Regra de decisão proposta para o §21.3 (o usuário decide)

Depois de pelo menos 20 sessões reais:

- repetição entre sessões **< 15%**: a deduplicação por sessão basta, e S3.15 é recusado;
- **≥ 30%**: S3.15 entra no plano;
- no meio: decide-se olhando a taxa dentro da sessão e o tamanho das sessões.

Os números são ponto de partida, não medida. O instrumento reporta; a regra é do usuário.

### Testes (vermelhos antes, verdes depois)

| teste | seam | lacuna que prova |
| --- | --- | --- |
| `session_hits_survive_across_hook_processes` | 4 (`tests/hooks.rs`) | dois `Broker` (dois processos), o mesmo estado: hoje `state` não guarda nada |
| `only_what_reached_the_model_counts_as_delivered` | 4 | uma resposta suprimida (sem novidade) não conta como entregue |
| `an_old_state_file_loads_with_zero_stats` | 4 | compatibilidade: estado salvo antes desta mudança |
| `hook_stats_aggregates_sessions_without_content` | CLI (`tests/cli.rs`) | hoje o comando não existe |
| `hook_stats_estimates_what_a_persistent_cache_would_add` | CLI | repetição entre sessões, em ordem cronológica |

O teste de privacidade que já existe (`session_state_is_private_and_holds_no_prompt_or_code`)
continua valendo sobre o estado com `stats`.

## 2. Item 1 — o instrumento do A/B

### Onde mora

Num módulo `ripwire_broker::eval` e num segundo binário, `ripwire-eval`, com
`default-run = "ripwire-broker"` no `Cargo.toml`. Isso deixa o `cargo run` como está e
mantém o broker sem um subcomando novo: **o produto não muda**. Sem dependência nova: o
grafo do `cargo tree -e normal` fica idêntico (CA-10 intacto), e isso é conferido.

### Braços

A união do §16.2 com o §23.15:

| braço | MCP do agente |
| --- | --- |
| `none` | nenhum |
| `ripwire` | `ripwire mcp` direto |
| `broker` | `ripwire-broker serve` |
| `broker-online` | `ripwire-broker serve --online` (só com a chave no ambiente, por consentimento) |

**Isolamento.** O agente de cada braço roda sem configuração nenhuma: o template padrão usa
`--strict-mcp-config` e `--setting-sources local` (era `project` até o
[D-117](../changelog.md#d-117--o-corpus-real-três-repositórios-e-duas-falhas-de-isolamento), que achou
hooks versionados num dos repositórios do corpus). Hooks e MCPs
globais (este ambiente injeta ripwire e graft em toda sessão) contaminariam o braço `none`.
Uma **guarda de contaminação** lê o evento `system/init` do transcript e invalida a execução
se aparecer uma ferramenta MCP que o braço não declarou. Uma execução inválida não entra na
média; ela aparece no relatório como inválida.

### Fluxo de uma execução (tarefa × braço × repetição)

1. Um repositório novo num diretório temporário, com `git fetch` **só do `base`**: vêm o
   commit e seus ancestrais, sem branch, sem tag e sem nada posterior. Um `git clone` comum
   levaria o commit da correção junto, e numa tarefa tirada do histórico o `git log --all`
   entregaria a resposta ao agente (achado ao montar a primeira tarefa; ver §2.1). O
   repositório de origem **nunca é tocado**: nem worktree, nem metadados em `.git`.
2. Grava a configuração MCP do braço e roda o agente por um template de comando com
   `{workdir}` e `{mcp_config}`. O prompt vai pelo stdin. Cada linha do stdout recebe um
   carimbo de tempo do relógio do executor.
3. Extrai do transcript (`stream-json` do Claude Code):
   - tokens (`result.usage`) e custo;
   - chamadas de ferramenta por classe: busca, leitura, edição, MCP e outras;
   - bytes dos resultados MCP;
   - tempo até a primeira edição;
   - arquivos apresentados pelo broker (os `items[].path` dos envelopes, em ordem).
4. Pontua contra a referência:
   - arquivos modificados (`git status`) contra os arquivos do patch de referência: recall
     e precisão;
   - posição do primeiro arquivo correto apresentado;
   - testes de referência citados;
   - correção pelo comando `check` da tarefa (código de saída 0).
5. Acrescenta uma linha em `results.jsonl` e apaga a cópia. Uma linha que já existe não é
   refeita, então uma rodada interrompida continua de onde parou.

### 2.1 Um vazamento achado ao montar a primeira tarefa

A primeira tarefa de verdade foi tirada do histórico deste repositório, com o `base` no pai
de `2a646f3` e a referência no próprio `2a646f3`. Ao escrevê-la ficou claro que o `git clone`
do desenho original levaria `2a646f3` para dentro da cópia do agente. O teste
`the_agent_cannot_see_history_after_the_base` reproduziu isso: o agente de teste leu
`the reference fix` no `git log --all`. Corrigido trazendo só o `base` por `git fetch` do id.

### Barras (relatório)

Cada barra sai como `pass`, `fail` ou `insuficiente`. É `insuficiente` quando o corpus não
tem ≥ 30 tarefas e ≥ 3 repositórios, ou quando falta o braço.

- **§17, `broker` × `none`:**
  - tokens −35%;
  - chamadas exploratórias (busca + leitura) −30%;
  - taxa de conclusão ≥;
  - recall de arquivos ≥;
  - recall de testes ≥ 80%.
- **§23.15, `broker-online` × `broker`:**
  - taxa de conclusão ≥;
  - recall de arquivos +10 pp nas tarefas marcadas `vocabulary_diverges`;
  - busca + leitura −20%.
- O que já é garantido pela suíte (orçamento, sem rede, inelegíveis, concorrência) é citado
  como coberto pela suíte, não medido de novo.

### Testes (vermelhos antes, verdes depois)

Em `tests/eval.rs`, com um agente falso (script `sh`) que imita o `stream-json`:

| teste | o que prova |
| --- | --- |
| `a_corpus_is_validated_before_anything_runs` | ids duplicados, tarefa sem arquivos de referência e repo inexistente são recusados |
| `a_transcript_is_reduced_to_counts` | classificação das ferramentas, tokens do `result`, primeira edição, arquivos apresentados |
| `a_transcript_without_a_result_is_invalid_not_zero` | um parser errado não pode virar "zero tokens" |
| `an_arm_contaminated_by_a_foreign_mcp_is_invalid` | guarda de isolamento |
| `a_run_is_scored_against_the_reference_patch` | recall/precisão por `git status`, correção pelo `check` |
| `the_bars_follow_the_prd_thresholds` | bordas exatas: −35% passa, −34,9% falha; corpus pequeno vira `insuficiente` |
| `ripwire_eval_runs_every_arm_and_never_touches_the_source_repo` | e2e pelo binário: cópias, `results.jsonl`, retomada e relatório; origem intacta |
| `the_agent_cannot_see_history_after_the_base` | a correção, commitada depois do `base`, não chega à cópia do agente |
| `the_first_edit_is_the_earliest_one` | o tempo até a primeira edição não é o da última |

A fixture do transcript é **sintética**, escrita conforme a documentação do `stream-json`
(mensagens `system/init`, `assistant` com `tool_use`, `user` com `tool_result` e `result`
com `usage`). Gravar uma fixture de uma execução real exige uma chamada paga, e fica para a
primeira rodada autorizada.

## 3. Mínimo impacto no resto, como é conferido

- As suítes atuais (295 no padrão, 308 com `online`) continuam verdes sem editar nenhum
  teste existente.
- `cargo tree -e normal` fica byte a byte igual ao de antes.
- A saída dos hooks para o host não muda: só o arquivo de estado ganha um objeto de
  contagens.
- `ripwire --quality-delta` sobre a mudança.
- No núcleo, só entram `Broker::session_hits()`, `hook::handle` (a soma), o parser e o
  despacho de `hook-stats`. O A/B fica todo em `src/eval/` e `src/bin/`.

## 4. Depois do código: o que é do usuário

1. ~~Escolher os 3 repositórios e escrever (ou aprovar) as ≥ 30 tarefas.~~ Repositórios escolhidos
   (dois privados e este); 32 tarefas montadas e validadas em
   [D-117](../changelog.md#d-117--o-corpus-real-três-repositórios-e-duas-falhas-de-isolamento), fora deste
   repositório. **Falta a revisão dos enunciados pelo usuário.**
2. Autorizar o gasto. A estimativa é 30 tarefas × 3 ou 4 braços × repetições, com
   `--max-budget-usd` por execução.
3. Para o braço `broker-online`, consentir com o envio de trechos dos repositórios escolhidos
   à Jev (§23.6).
4. Usar os hooks por alguns dias e rodar `ripwire-broker hook-stats`.
