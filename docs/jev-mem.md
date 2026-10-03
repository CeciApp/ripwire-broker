# Jev-Mem: algoritmo e `--memory` no ripwire-broker

**Projeto:** `/root/projects/ripwire-broker`  
**Data:** 2026-10-03  
**Paper:** Jiang, Li, Li. *Jev-Mem: System-One-Controlled Agentic Memory for Efficient AI Agents*. arXiv:2609.23986v1, 21 Sep 2026.  
**PDF:** `docs/2026-10-03_Jev-Mem.pdf` (SHA-256 `413c592431ce55f8372c1561b7dd572e3603d0e66b447f6e4097e4678bfc5e87`)  
**Código dos autores:** https://github.com/libingzheren/Jev-Mem  
**Jev (System One):** https://typesafe.ai/

Isto é estudo. **Não** implementa `--memory`. Não altera o comportamento de `--online`.

---

## 1. O que o paper propõe

Memória de agente hoje ou é heurística rígida ou põe um LLM autorregressivo no caminho crítico (tipar, ligar, rotear, parar). Jev-Mem separa:

| Plano | Papel | Quem |
|---|---|---|
| **System One** | Decisões frequentes, saída **limitada** (label, P ∈ [0,1], Choice) | Jev (`TypeSafeClient.system_one`) |
| **Memory plane** | Nós canônicos + 4 grafos + índices vetorial e léxico | Store local |
| **System Two** | Síntese / resposta em linguagem natural | LLM generativo **separado** |

Jev **não** gera a resposta. Só controla escrita e recuperação.

Resultados no LoCoMo (paper): LLM-as-a-Judge **0.777** (+11,0% vs melhor baseline); construção **158 s** (6,6× mais rápida); latência média de query **0,93 s** (−36,7%). Tratar como claim dos autores até reproduzir.

---

## 2. Algoritmo

### 2.1 Estado

Observação: \(o_t = (x_t, \tau_t, \mu_t)\) — conteúdo, timestamp, proveniência.

Memória:

\[
M_t = \bigl(V_t,\; \{E^g_t\}_{g \in G},\; I^{\mathrm{vec}}_t,\; I^{\mathrm{lex}}_t\bigr)
\]

\[
G = \{\mathrm{semantic},\; \mathrm{temporal},\; \mathrm{causal},\; \mathrm{entity}\}
\]

Os quatro grafos compartilham os mesmos nós \(V_t\). O mesmo par pode ter várias arestas tipadas.

### 2.2 Controlador System One

\(J(S, Q)\): estado estruturado \(S\) + lote de perguntas \(Q\).

- **Noul:** proposição independente → score em [0, 1] (não assumir calibração).
- **Choice:** alternativas mutuamente exclusivas → label + distribuição.
- Perguntas do mesmo lote **não** leem as respostas umas das outras.
- Vários Nouls no mesmo \(S\) = uma invocação em batch.

Isto é o mesmo contrato que o `--online` já usa: probabilidade tipada, nunca texto livre.

### 2.3 Escrita (construção)

Default do paper: **sem filtro de admissão**. Toda observação não vazia vira nó. Selectividade entra nas relações e na recuperação, não no “esquecer na entrada”.

1. Criar nó canônico (texto, proveniência, timestamp, embedding, entidades).
2. **Typing** (4 Nouls, overlapping): `episodic`, `semantic`, `procedural`, `preference`. Anotam o nó; não são classes exclusivas.
3. **Candidatos** (determinístico, ≤ \(K_w = 10\)): similaridade vetorial + overlap léxico + entidades + proximidade temporal. **Não** comparar com todos os nós.
4. Para cada par \((v,u)\), Jev estima: ligação semântica, causal nas duas direções, mesmo episódio; alias de entidade só se IDs não batem.
5. Temporal com timestamps confiáveis = aresta **sem** Jev. Temporal implícito = Choice: `before | after | during | contains | overlaps | same_time | unknown`.
6. Inserir aresta tipo \(g\) só se \(P(g \mid v,u) \ge \theta_{\mathrm{rel}} = 0.60\).

Fluxo: `observação → type → candidatos → relações → M`.

### 2.4 Consolidação (a cada 20 writes)

Quatro Nouls: redundância, contradição, obsolescência, utilidade do link.  
Choice `representation`: `keep_separate | merge | promote | uncertain`.

Observações cruas **permanecem**. System Two só resume se `merge` ou `promote` ≥ **0,85** e contradição < **0,85**.

### 2.5 Leitura (recuperação)

1. **Routing** (só `query`): 6 Nouls — `semantic`, `temporal`, `causal`, `entity`, `multi_hop_need`, `recency_importance`. Vários grafos podem ativar juntos.
2. Orçamento de expansão **80**. Grafo com need ≥ **0,10** ganha no mínimo 1; resto proporcional (expoente 1,0, largest remainder).
3. Âncoras: busca vetorial + keyword; depois expansão.
4. **Scoring** de candidato (4 Nouls): `relevance`, `new_information`, `relation_usefulness`, `supports_current_evidence`, combinados com cosseno query–memória, need do grafo, P da aresta, ajuste de recência. Beam **10**.
5. **Parada** (query + top-k + profundidade):
   - suficiente se `evidence_sufficient` ≥ **0,95** e `missing_evidence` e `contradiction` < **0,15**;
   - ou `continue_useful` < **0,15**;
   - ou limites: profundidade 8, 60 nós, 2400 arestas, 16 tentativas Jev, **15 s**.
6. Memórias selecionadas vão para **System Two**. Jev não gera a resposta.

Ciclo: `query → route → âncoras → expand → score → evidence check → stop | continue`.

---

## 3. O que o broker já faz (`--online`)

`--online` **não** é Jev-Mem. É um classificador de **relevância de código** no `context_for_task`, só em rotas que caem em `explore`.

| | `--online` hoje | Jev-Mem |
|---|---|---|
| Store | nenhum (só cache in-memory de decisões) | grafo multi-relacional persistente |
| Write | não grava experiência do agente | todo \(o_t\) vira nó |
| Jev | “este arquivo/trecho é relevante à tarefa?” | typing, relações, routing, scoring, stop |
| System Two | o **host** (Claude/Codex) lê o envelope MCP | LLM separado para síntese |
| Superfície | sem tool nova | ciclo write/read contínuo |

Reuse útil: cliente HTTP Jev, batch de Nouls, timeouts, `RIPWIRE_BROKER_JEV_API_KEY`, feature Cargo `online`, política de não mandar segredo.

Não reuse: tratar `--online` como memória. Hipótese de arquivo ≠ episódio de sessão.

Já existe um System Two **local opcional**: `--summarizer-cmd` (ollama/llama.cpp/`llm`), não bloqueante, cache por processo, no máximo 3 notas de 600 caracteres a partir da evidência do próprio envelope. Hooks **não** recebem notas (D-046).

---

## 4. Estudo: como seria `--memory`

### 4.1 Contrato (espelhar `--online`)

1. Sem `--memory` → zero store, zero Jev de memória.
2. Flag só no `serve` (como `--online`, D-064).
3. Credencial Jev: a mesma env `RIPWIRE_BROKER_JEV_API_KEY` se o control plane for Jev remoto. Sem chave → falha clara, sem downgrade silencioso.
4. Jev nunca vira fato, nunca edita código, nunca “autoriza” merge.
5. Persistência **local** (state dir já existe para hooks). Workspace-scoped. Sem telemetria de conteúdo.
6. `context_after_edit` / `context_before_finish` no MVP de memória: **write** de observação (o que mudou / o que ficou pendente); **read** só em `context_for_task` (e opcionalmente no primeiro `UserPromptSubmit`).

### 4.2 Observações \(o_t\) no broker

Não copiar diálogo clínico. Este produto é contexto de **código**.

Candidatos a nó:

- texto da tarefa (`context_for_task`);
- arquivos admitidos + hashes (já no envelope);
- preferência explícita do usuário no prompt (curta);
- resultado de hook `Stop` / `attention_required`;
- nota de consolidação (se System Two local rodou).

**Não** gravar: prompt completo, `.env`, diffs crus ilimitados, PII.

### 4.3 Mapeamento write/read

| Passo Jev-Mem | No broker |
|---|---|
| Nó canônico | registro em `$XDG_STATE_HOME/ripwire-broker/memory/<workspace-hash>/` (sqlite ou grafos JSONL) |
| Typing | 4 Nouls do paper, estado = texto da observação |
| Candidatos \(K_w=10\) | ripwire (símbolos/paths) + índice local vetorial se houver; senão léxico+path |
| Relações | Jev no par; temporal via mtime/commit; entity via path/símbolo |
| \(\theta_{\mathrm{rel}}=0.60\) | igual, configurável depois |
| Consolidação / 20 writes | Choice; merge/promote só com `--summarizer-cmd` |
| Routing na query | 6 Nouls sobre a tarefa |
| Âncoras | ripwire + memória local |
| Score + stop | mesmos Nouls; budget menor que 80 (orçamento de tokens do MCP) |
| System Two | **não** chamar o Opus da sessão; devolver evidência no envelope e deixar o host sintetizar |

### 4.4 System Two: três opções (ordem)

**A. Host = System Two (recomendado no MVP).** O paper já faz isso: Jev escolhe evidência; o LLM da conversa responde. O broker injeta itens `role: memory` no envelope, como `semantic_location` no `--online`. Zero API extra.

**B. `--summarizer-cmd` = System Two local.** Já existe. Usar **só** para `merge`/`promote` na consolidação (limiar 0,85). Não bloquear o envelope (`--summarizer-wait-ms`).

**C. Segunda chamada Anthropic/API.** Possível com chave **separada**. Não é “o Opus desta sessão”: outro request, outro billing, sem KV cache compartilhado. Quebra o default offline. Fora do MVP.

Não misturar A+C no mesmo caminho crítico.

### 4.5 Feature flags e CI

Espelhar `online`: Cargo feature `memory` ou reusar `online` (Jev HTTP). Dois jobs no `rust.yml` já existem (`default` / `online`). `--memory` sem Jev (só grafo + heurística) não vale o paper; sem Jev vira Mem0 pobre.

MVP: `--memory` **exige** `--online` (mesmo cliente Jev) **ou** documentar um System One local (não existe no broker hoje). Preferir exigir `--online`.

### 4.6 Fora de escopo (igual `--online`)

- Jev como gerador de resposta.
- Substituir ripwire.
- Fine-tune / continual learning no grafo.
- Enviar o grafo inteiro ao Jev.

### 4.7 Ordem de implementação (quando houver decisão)

1. Spec em `spec/` (RF `--memory`, D-IDs, o que sai da máquina).
2. Store local + testes de grafo sem rede.
3. Write path com Jev (reuse `src/online/`).
4. Read path no `context_for_task`, itens aditivos.
5. Consolidação + `--summarizer-cmd`.
6. A/B como §23.15 do `--online` — senão fica experimental.

---

## 5. Plugin pode chamar a LLM da sessão?

**Não, no contrato MCP atual do broker.** [Certain]

O host (Claude Code / Codex) **chama** tools do servidor. O servidor **não** tem um método “completa este prompt com o modelo desta conversa”. Stdio MCP = processo filho; argumentos + env. Não há callback para Opus/Sonnet.

Consequências:

| Ideia | Veredito |
|---|---|
| MCP pede ao Claude Code “roda este prompt no Opus em paralelo” | **Não existe** na API MCP que o broker usa |
| Subagentes Claude Code em paralelo | O **host** dispara; compartilham o **mesmo** processo MCP do pai (README: subagents share parent MCP). Não é o plugin invocando o modelo |
| `--summarizer-cmd` (ollama etc.) | **Sim**: o broker já chama um CLI local. Isso é System Two **à parte**, não o Sonnet da sessão |
| `anthropic` HTTP com `ANTHROPIC_API_KEY` | **Sim**, segunda inferência paga, fora da sessão. Não é “o modelo em que ele está rodando” |
| Hooks injetam contexto | O modelo da sessão **lê** o contexto; o plugin não “chama” o modelo, só alimenta o próximo turno |

“Em paralelo Opus **e** Sonnet”: só com **dois** clients/API (ou dois agentes no host). O broker não orquestra isso.

Para `--memory`, System Two = **A** (evidência no envelope). System One = Jev. Local generativo só na consolidação via summarizer já existente.

---

## 6. Referências internas

- `spec/old/jev-integration-prd.md` — `--online`
- `src/online/` — cliente Jev
- `src/summarizer.rs` — System Two local opcional
- `handoff.md` — `--online` experimental até A/B
- Paper appendix B — prompts Noul/Choice (typing, relações, routing, stop)
