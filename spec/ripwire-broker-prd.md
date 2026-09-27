# PRD: ripwire-broker — Contexto de código orientado à tarefa para agentes MCP

> **Produto:** `ripwire-broker`
> **Categoria:** servidor MCP local de orquestração e enriquecimento de contexto de código
> **Status:** Draft para validação
> **Versão do PRD:** 0.2
> **Data:** 2026-09-27
> **Linguagem:** Rust
> **SDK MCP:** [`rust-mcp-sdk` 2.0.0](https://crates.io/crates/rust-mcp-sdk)
> **Dependência principal:** servidor MCP do [Ripwire](https://github.com/redhat-et/ripwire)
> **Postura padrão:** local, offline, read-only e com orçamento explícito de contexto

---

## Sumário

1. [Resumo executivo](#1-resumo-executivo)
2. [Problema](#2-problema)
3. [Visão do produto](#3-visão-do-produto)
4. [Objetivos e não objetivos](#4-objetivos-e-não-objetivos)
5. [Usuários e trabalhos a realizar](#5-usuários-e-trabalhos-a-realizar)
6. [Proposta de valor](#6-proposta-de-valor)
7. [Arquitetura](#7-arquitetura)
8. [Ciclo de contexto](#8-ciclo-de-contexto)
9. [Superfície MCP do MVP](#9-superfície-mcp-do-mvp)
10. [Estratégia de enriquecimento](#10-estratégia-de-enriquecimento)
11. [Estratégia de redução de tokens](#11-estratégia-de-redução-de-tokens)
12. [Exemplos de uso](#12-exemplos-de-uso)
13. [Requisitos funcionais](#13-requisitos-funcionais)
14. [Requisitos não funcionais](#14-requisitos-não-funcionais)
15. [Segurança e privacidade](#15-segurança-e-privacidade)
16. [Observabilidade e avaliação](#16-observabilidade-e-avaliação)
17. [Critérios de sucesso](#17-critérios-de-sucesso)
18. [Critérios de aceite do MVP](#18-critérios-de-aceite-do-mvp)
19. [Roadmap](#19-roadmap)
20. [Riscos e mitigações](#20-riscos-e-mitigações)
21. [Decisões em aberto](#21-decisões-em-aberto)
22. [Referências](#22-referências)

---

## 1. Resumo executivo

O `ripwire-broker` é um servidor MCP local que se conecta ao servidor MCP do
Ripwire e transforma sua superfície ampla de análise em três operações de alto
nível, orientadas ao ciclo real de trabalho de um agente:

1. preparar contexto antes de executar uma tarefa;
2. avaliar impacto depois de uma edição;
3. verificar qualidade e testes antes de concluir.

O produto não cria outro parser nem outro grafo de código. O Ripwire continua
sendo a fonte dos fatos estruturais: símbolos, chamadas, usos, impacto, testes,
qualidade, histórico e limitações da análise. O broker acrescenta uma camada de
orquestração que:

- escolhe os verbos corretos do Ripwire;
- combina e deduplica seus resultados;
- aplica um orçamento máximo de tokens;
- explica por que cada item foi incluído;
- preserva proveniência, confiança, truncamentos e limitações;
- adapta a resposta ao momento da tarefa;
- oferece uma interface MCP pequena e estável aos agentes;
- possibilita injeção automática por skill, hook ou wrapper do cliente.

O resultado esperado é que agentes encontrem mais cedo os arquivos, contratos
e testes relevantes, com menos buscas exploratórias, menos leitura integral de
arquivos e menor risco de declarar uma mudança concluída sem considerar seu
raio de impacto.

### 1.1 Hipótese central

> Uma camada pequena, determinística e orientada ao momento de trabalho consegue
> converter a profundidade analítica do Ripwire em contexto imediatamente útil
> ao agente, sem expor ao modelo toda a complexidade do Ripwire e sem enviar o
> código-fonte para serviços externos.

### 1.2 Posicionamento

| Produto | Responsabilidade principal |
| --- | --- |
| Ripwire | Extrair, indexar, relacionar e analisar fatos do repositório |
| `ripwire-broker` | Selecionar, combinar, comprimir e entregar esses fatos no momento certo |
| Cliente/agente | Usar o contexto para raciocinar, editar, testar e responder |

---

## 2. Problema

### 2.1 Problema do agente

Agentes de programação frequentemente começam uma tarefa sem conhecer o
repositório. Antes de produzir uma alteração, repetem um ciclo caro:

1. procuram termos e nomes;
2. abrem arquivos inteiros;
3. seguem imports e chamadas manualmente;
4. descobrem tarde que havia implementações irmãs;
5. editam antes de conhecer o raio de impacto;
6. escolhem testes por proximidade de nome, não por alcance real;
7. relêem parte do repositório depois da edição.

Esse comportamento consome tokens e tempo e pode produzir mudanças localmente
corretas, mas incompletas no conjunto do sistema.

### 2.2 Problema da integração direta com o Ripwire

O Ripwire já responde às perguntas relevantes, mas sua interface completa é
deliberadamente ampla. Expor todos os verbos diretamente ao modelo cria quatro
custos:

- **descoberta:** o modelo precisa escolher entre muitas operações;
- **schema:** clientes podem colocar as descrições das tools no contexto do modelo;
- **orquestração:** uma resposta final pode exigir várias chamadas e deduplicação;
- **consistência:** agentes diferentes podem consultar o Ripwire de maneiras diferentes para a mesma tarefa.

Além disso, MCP fornece ferramentas ao cliente, mas não determina como a
aplicação de IA deve usar ou injetar o contexto recebido. A automação precisa de
uma integração no host, como skill, hook ou wrapper.

### 2.3 Oportunidade

Criar uma fachada MCP pequena que preserve a capacidade analítica do Ripwire,
mas apresente ao agente somente decisões de alto nível:

```text
começar tarefa → avaliar edição → concluir com segurança
```

---

## 3. Visão do produto

### 3.1 Visão

Ser a camada local de contexto que entrega a cada agente apenas o trecho do
repositório necessário para o próximo passo, com evidência suficiente para
melhorar a mudança e pequeno o bastante para não competir com o raciocínio do
modelo.

### 3.2 Princípios

1. **Ripwire é a fonte dos fatos.** O broker não reimplementa parsing ou análise estática.
2. **Offline por padrão.** Nenhum código, prompt, caminho ou símbolo sai da máquina no MVP.
3. **Read-only por padrão.** O broker não expõe os verbos de edição do Ripwire.
4. **Contexto sob orçamento.** Toda operação possui limite explícito e relata cortes.
5. **Proveniência sempre visível.** Cada conclusão aponta para arquivos, símbolos e verbos de origem.
6. **Incerteza não é escondida.** Ambiguidade, ausência de cobertura e resultados parciais são preservados.
7. **Uma tool por momento.** O agente escolhe uma intenção de alto nível; o broker resolve a sequência interna.
8. **Corpos sob demanda.** Assinaturas e relações vêm antes; corpos completos entram apenas quando necessários.
9. **Sem instruções vindas do repositório.** Código e documentos recuperados são dados não confiáveis.
10. **Medição antes de promessa.** Economia e qualidade são avaliadas por tarefas reproduzíveis.

---

## 4. Objetivos e não objetivos

### 4.1 Objetivos do MVP

- Expor três tools MCP de alto nível.
- Conectar-se ao Ripwire MCP por `stdio` e manter a sessão aquecida.
- Operar em um workspace fixado no início do processo.
- Selecionar automaticamente os verbos upstream adequados.
- Produzir saída estruturada e legível por modelos.
- Respeitar orçamento máximo de contexto em todas as respostas.
- Integrar-se por instrução/skill com Codex e Claude Code.
- Permitir uma futura integração transparente por hook ou wrapper.
- Preservar diagnósticos, confiança, truncamento e proveniência do Ripwire.
- Medir tokens estimados, latência, chamadas upstream e arquivos apresentados.

### 4.2 Não objetivos do MVP

- Criar um novo grafo de chamadas.
- Ler ou interpretar diretamente o cache binário do Ripwire.
- Modificar arquivos do repositório.
- Substituir o executor de testes do projeto.
- Corrigir código automaticamente.
- Gerar resumos por serviço LLM remoto.
- Hospedar um serviço multi-tenant.
- Oferecer Streamable HTTP público.
- Garantir completude onde o Ripwire declara apenas um limite inferior.
- Suportar múltiplos workspaces em uma mesma instância.

---

## 5. Usuários e trabalhos a realizar

### 5.1 Personas

| Persona | Necessidade |
| --- | --- |
| Desenvolvedor com agente MCP | Obter contexto útil sem explicar manualmente a estrutura do projeto |
| Maintainer | Reduzir mudanças incompletas, contratos quebrados e testes esquecidos |
| Equipe com repositório grande | Evitar que cada sessão redescubra a mesma arquitetura |
| Autor de agentes | Expor poucas ferramentas, com comportamento previsível e baixo custo de contexto |
| Organização com código sensível | Usar análise e enriquecimento sem enviar o código para fora da máquina |

### 5.2 Jobs to be done

- “Quando eu iniciar uma tarefa, quero que o agente receba os símbolos, corpos e testes mais relevantes sem percorrer o repositório inteiro.”
- “Quando uma edição atingir código compartilhado, quero saber quais contratos, callers e arquivos podem ter sido afetados.”
- “Antes de aceitar uma tarefa como concluída, quero verificar regressões de qualidade e testes obrigatórios.”
- “Quando o contexto for limitado, quero saber o que foi omitido e como buscar o próximo nível de detalhe.”
- “Quero usar o contexto do Ripwire sem ensinar cada agente a combinar dezenas de verbos.”

---

## 6. Proposta de valor

### 6.1 O que o broker acrescenta ao Ripwire

| Capacidade do Ripwire | Enriquecimento do broker | Benefício ao agente |
| --- | --- | --- |
| Ranking de símbolos | Agrupa por papel na tarefa e explica `why_included` | O modelo sabe o que ler primeiro |
| Callers, usos e impacto | Remove duplicatas e distingue contrato, execução e configuração | Menos ruído e conclusões mais precisas |
| Corpos por handle | Busca apenas os corpos necessários ao orçamento | Menos código irrelevante no contexto |
| Testes alcançáveis | Consolida caminho, razão e comando quando disponível | Validação mais provável e reproduzível |
| Co-change e histórico | Destaca arquivos normalmente alterados juntos, mas ausentes | Menos mudanças parciais |
| `quality_delta` | Resume regressões introduzidas, não dívida preexistente | Revisão focada no que a tarefa piorou |
| Limitações e contagens parciais | Normaliza em `limitations[]` e `confidence` | O agente não interpreta ausência como prova |
| Muitos verbos MCP | Três tools estáveis de alto nível | Menor carga de escolha e schema |

### 6.2 Diferencial

O valor do `ripwire-broker` não é apenas resumir. Ele cria contexto composto e
orientado à decisão. Um resultado de início de tarefa pode reunir, em uma única
resposta:

- símbolos relevantes;
- corpos centrais;
- callers diretos;
- arquivos irmãos ou de co-change;
- documentação relacionada;
- testes alcançáveis;
- riscos e limitações;
- próximo passo recomendado.

Sem o broker, o agente precisa conhecer a taxonomia do Ripwire, realizar várias
chamadas e consolidar resultados dentro de sua própria janela de contexto.

### 6.3 Por que não ler o cache do Ripwire

O cache do Ripwire é uma otimização de parsing e velocidade. Ele é versionado,
pode ser específico da arquitetura e não constitui um contrato semântico para
consumidores externos. O broker deve consumir tools e respostas públicas do
Ripwire, permitindo que o próprio Ripwire valide atualização, ambiguidades,
arquivos ignorados e limites da análise.

Se o broker precisar de cache próprio, deverá armazenar apenas respostas
normalizadas ou resumos derivados, sempre acompanhados de:

- versão do schema do broker;
- versão do Ripwire;
- identidade do workspace;
- commit e estado dirty relevantes;
- hashes ou handles de origem;
- momento da geração;
- orçamento usado.

### 6.4 Comparação: Graft × Ripwire × jevgrep

Os três produtos reduzem a exploração manual de repositórios, mas fazem isso
com modelos diferentes. Eles são análogos na finalidade geral — entregar
contexto relevante ao agente — e não equivalentes em arquitetura ou garantias.

| Dimensão | Graft | Ripwire | jevgrep |
| --- | --- | --- | --- |
| Objetivo principal | Construir e entregar um mapa contextual do repositório | Mapear estrutura, responder consultas e verificar mudanças | Encontrar arquivos e trechos perguntando o que o código faz |
| Modelo de recuperação | Grafo estrutural local; camada semântica opcional gerada por LLM | Grafo estático determinístico, resolução de referências e ranking | Exploração hierárquica guiada por classificações do Jev |
| Unidade principal | Nós conceituais, símbolos, arquivos e relações | Símbolos, chamadas, usos, testes, histórico e métricas | Diretórios, arquivos, declarações, reading leads e excertos |
| Operação offline | Sim na camada estrutural; enriquecimento profundo depende do provedor configurado | Sim para análise, navegação e qualidade | Não no fluxo normal: exige provedor e envia conteúdo elegível ao Jev |
| Chave/API | Não para o grafo estrutural; necessária para `--deep` salvo modelo local | Não | Sim: Vercel AI Gateway, TypeSafe ou OpenRouter |
| Grafo de chamadas | Sim | Sim, com consultas amplas de impacto e uso | Não é seu contrato principal |
| Pós-edição | Blast radius, hooks e ressincronização | Contrato, impacto, testes, qualidade, co-change e revisão | O agente continua responsável por implementação e verificação |
| Testes | Pode localizar contexto e relações de teste | Calcula testes alcançáveis e obrigações conhecidas | Pode sugerir entrada de teste, mas não prova cobertura |
| Qualidade/arquitetura | Secundário ao contexto | Parte central: delta, clones, camadas, hotspots e seams | Fora do escopo principal |
| Interface documentada | CLI, MCP e integração por hooks | CLI, MCP e skills | CLI `jg` e skill; não há MCP nativo documentado |
| Persistência | Grafo local em arquivos Markdown e estrutura de símbolos | Cache local/in-memory e artefatos de índice | Cache local de respostas; saída principal em stdout |
| Linguagens | Grafo estrutural para múltiplas linguagens | Múltiplas gramáticas e formatos | Parsing de declarações para Python e TS/JS; fallback textual nos demais |
| Saída | Nós, mapa, relações e fonte relacionada | Saída analítica estruturada, com limitações explícitas | Resumo, arquivos, reading leads e fonte literal com linhas |
| Privacidade padrão | Código estrutural local; `--deep` pode usar serviço externo | Código permanece local | Conteúdo elegível é enviado ao provedor escolhido |
| Licença | MIT | Apache 2.0 | MIT |

#### Leitura estratégica

- **Graft é a referência de experiência:** demonstra como contexto pré-selecionado,
  hooks e integração com o host podem evitar que o agente recomece do zero.
- **Ripwire é a base analítica do broker:** oferece a superfície estruturada,
  offline e verificável necessária para orientação, impacto, testes e qualidade.
- **jevgrep é a referência de recuperação semântica remota:** é forte quando o
  usuário sabe descrever um comportamento, mas ainda não conhece nomes, símbolos
  ou caminhos. Sua exploração hierárquica pode encontrar arquivos por significado,
  inclusive sem um grafo prévio especializado naquela linguagem.

### 6.5 O que o `ripwire-broker` deve aprender com o jevgrep

O jevgrep reforça cinco decisões úteis para o broker:

1. **Pergunta como entrada principal.** O usuário descreve comportamento, não precisa conhecer símbolos.
2. **Resumo primeiro.** O início da resposta deve continuar útil mesmo quando o restante for truncado.
3. **Localização sem excerto ainda tem valor.** Um arquivo relevante pode ser retornado como lead quando não houver confiança ou orçamento para incluir fonte.
4. **Descoberta incompleta deve ser explícita.** Diretório não visitado ou classificação falha permanece desconhecido, não irrelevante.
5. **Fonte é evidência, não instrução.** Conteúdo recuperado não ganha autoridade por ter sido selecionado.

Apesar disso, o jevgrep não deve ser dependência do MVP porque conflita com dois
requisitos centrais: operação offline e Ripwire como única fonte estrutural. Uma
fase futura poderá avaliar um adaptador semântico opcional e explicitamente
opt-in, com consentimento para envio do root selecionado. Esse adaptador nunca
deve ser fallback silencioso.

### 6.6 Benchmarks do jevgrep e interpretação

O jevgrep publica um repeat de dez tarefas SWE-bench no qual o custo do agente
caiu de US$ 7,62 para US$ 4,52, aproximadamente 40% de redução, mas a taxa de
resolução foi de 7/10 contra 8/10 no baseline. Os custos do Jev foram observados
separadamente e excluídos do total principal. O próprio projeto apresenta isso
como economia com trade-off de qualidade, não como superioridade geral.

Para o `ripwire-broker`, essa evidência reforça uma regra de produto: redução de
tokens ou custo só é sucesso quando a correção é mantida. Os testes A/B do broker
devem medir simultaneamente custo, arquivos recuperados, testes escolhidos e
resultado final da tarefa.

---

## 7. Arquitetura

### 7.1 Visão lógica

```text
┌─────────────────────────────────────────────────────────────┐
│ Host do agente: Codex, Claude Code, Cursor ou outro cliente │
└──────────────────────────────┬──────────────────────────────┘
                               │ MCP stdio
                               ▼
┌─────────────────────────────────────────────────────────────┐
│ ripwire-broker                                              │
│                                                             │
│  Tool facade → Intent router → Query planner                │
│                         ↓                                   │
│  Budgeter ← Normalizer ← Upstream MCP client                │
│      ↓                                                      │
│  Context envelope + provenance + limitations                │
└──────────────────────────────┬──────────────────────────────┘
                               │ MCP stdio
                               ▼
┌─────────────────────────────────────────────────────────────┐
│ ripwire <workspace> --mcp                                   │
│ grafo, ranking, impacto, testes, histórico e qualidade      │
└──────────────────────────────┬──────────────────────────────┘
                               │ leitura local
                               ▼
                      Repositório de código
```

### 7.2 Componentes

| Componente | Responsabilidade |
| --- | --- |
| MCP facade | Publicar as três tools e o resource de status |
| Workspace guard | Fixar e canonicalizar a raiz autorizada |
| Upstream manager | Iniciar, monitorar e reiniciar o processo MCP do Ripwire |
| Intent router | Classificar tarefa como orientação, símbolo, erro, mudança ou revisão |
| Query planner | Escolher e ordenar verbos do Ripwire |
| Result normalizer | Converter formatos upstream em um modelo comum |
| Deduplicator | Unificar símbolos, caminhos, testes e diagnósticos repetidos |
| Budgeter | Selecionar conteúdo até o limite solicitado |
| Provenance tracker | Registrar origem e justificativa de cada item |
| Renderer | Produzir conteúdo estruturado e resumo textual opcional |
| Metrics sink | Registrar métricas locais sem conteúdo sensível |

### 7.3 Transporte

O MVP usa `stdio` nos dois lados:

```text
agente ⇄ stdio ⇄ ripwire-broker ⇄ stdio ⇄ ripwire
```

Motivos:

- operação local sem porta de rede;
- menor superfície de ataque;
- compatibilidade com clientes MCP de desenvolvimento;
- baixo overhead;
- encerramento conjunto dos processos.

Streamable HTTP fica fora do MVP. Quando implementado, deve permanecer em
loopback por padrão e exigir autenticação por bearer token fornecido por variável
de ambiente para qualquer bind roteável.

### 7.4 Compatibilidade MCP

O broker deve negociar a versão MCP suportada pelo SDK escolhido e não depender
de uma única versão hardcoded. Tools devem possuir JSON Schema estrito, nomes
estáveis, descrições orientadas ao momento de uso e resultados estruturados.

Com o upstream não há negociação real: o SDK é stateless (`2026-07-28`, sem
`initialize`) e o Ripwire 0.6.4 anuncia `2025-06-18`, mas aceita `tools/list` e
`tools/call` sem handshake. A compatibilidade é garantida por versão mínima do
Ripwire, validação dos verbos obrigatórios e testes contra o binário real
([D-002](changelog.md#d-002--compatibilidade-de-protocolo-sdk--ripwire),
[D-012](changelog.md#d-012--toolslist-contra-servidor-pré-2026)).

### 7.5 Stack de implementação

O `ripwire-broker` será implementado em **Rust** com
[`rust-mcp-sdk` 2.0.0](https://crates.io/crates/rust-mcp-sdk). Essa versão
implementa o protocolo MCP `2026-07-28` stateless e fornece os dois papéis de que
o broker precisa:

- **servidor MCP** para publicar as três tools e o resource de status ao host;
- **cliente MCP** para iniciar e consultar `ripwire <workspace> --mcp` por `stdio`.

Dependência inicial:

```toml
[dependencies]
rust-mcp-sdk = { version = "=2.0.0", default-features = false, features = [
  "server",
  "client",
  "macros",
  "stdio"
] }
```

Decisões de uso do SDK:

- usar `ServerHandler` e `server_runtime::create_server` na interface voltada ao agente;
- usar `ClientHandler` e `client_runtime::create_client` na conexão upstream com o Ripwire;
- habilitar somente o transporte `stdio` no MVP;
- manter default features desabilitadas para reduzir superfície e dependências;
- usar os tipos do SDK para requests e resultados; os JSON Schemas das tools são
  escritos à mão (estritos, com `enum` e `additionalProperties: false`), porque o
  derive `JsonSchema` do SDK 2.0.0 não suporta enums
  ([D-017](changelog.md#d-017--schemas-estritos-das-tools));
- coletar métricas locais no próprio núcleo do broker, não por `McpObserver`: só o
  núcleo sabe a qual tool call pertence cada chamada upstream
  ([D-015](changelog.md#d-015--status-e-métricas));
- manter o adaptador do SDK isolado do roteador, normalizador e budgeter do produto.
  A única exceção é a constante da versão de protocolo lida pelo status.

O pin exato em `2.0.0` torna builds iniciais reproduzíveis. Atualizações de versão
exigem teste de contrato com os clientes suportados e com o MCP do Ripwire antes
de alterar o lockfile e o PRD.

---

## 8. Ciclo de contexto

### 8.1 Início da tarefa

```text
prompt do usuário
  → context_for_task
  → classificar intenção
  → explore / from_trace / find_symbol / memory_recall
  → buscar corpos somente quando úteis
  → combinar símbolos + testes + riscos
  → aplicar orçamento
  → entregar contexto ao agente
```

### 8.2 Depois de uma edição

```text
arquivos/símbolos alterados
  → context_after_edit
  → situational_awareness
  → edit_check quando houver símbolo-alvo
  → impacto + co-change + testes
  → entregar apenas o delta relevante
```

### 8.3 Antes de concluir

```text
estado atual da working tree
  → context_before_finish
  → situational_awareness (fornece os arquivos alterados)
  → quality_delta
  → affected (semeado com os arquivos alterados)
  → regressões + testes obrigatórios + lacunas
  → decisão: ready | attention_required | unknown
```

### 8.4 Automação no host

MCP, isoladamente, disponibiliza tools; ele não obriga o host a chamá-las. O
produto terá três níveis de integração:

1. **MVP — skill/instrução:** orienta o agente a chamar a tool correspondente em cada momento.
2. **Adaptador — hook:** chama o broker antes do prompt ou depois de uma edição quando o host oferece esse evento.
3. **Wrapper:** intercepta o prompt, consulta o broker e entrega prompt + contexto a clientes sem hook adequado.

O modo automático sempre deve permitir opt-out por sessão e mostrar que contexto
foi injetado.

**Estado (Fase 2):** os níveis 2 e 3 estão implementados.
- O comando `ripwire-broker hook` atende o Claude Code e o Codex, que compartilham o
  contrato de hook.
- O contexto é injetado no primeiro prompt e depois de edições. O gate de `Stop` é
  opt-in (`--gate`).
- `#ripwire-off` / `#ripwire-on` fazem o opt-out por sessão.
- Cada injeção mostra um `systemMessage`, e o `hook-log` mostra contagens.
- `ripwire-broker prompt` é o wrapper.
- Detalhes em [D-030](changelog.md#d-030--hooks-nos-dois-hosts-proposta),
  [D-031](changelog.md#d-031--granularidade-da-automação-proposta),
  [D-041](changelog.md#d-041--contratos-reais-dos-hooks-e-limite-de-saída) e
  [D-042](changelog.md#d-042--ponto-de-parada-2-cli-e-hooks).

---

## 9. Superfície MCP do MVP

### 9.1 Tool `context_for_task`

**Objetivo:** preparar o menor pacote suficiente para iniciar ou retomar uma tarefa.

#### Entrada

| Campo | Tipo | Obrigatório | Padrão | Descrição |
| --- | --- | --- | --- | --- |
| `task` | string | sim | — | Pedido atual em linguagem natural, símbolo ou stack trace |
| `budget_tokens` | integer | não | `2500` | Limite estimado para o conteúdo retornado |
| `mode` | enum | não | `auto` | `auto`, `orient`, `debug`, `change`, `review` |
| `include_docs` | boolean | não | `true` | Permite recuperar documentação relacionada |
| `include_bodies` | boolean | não | `true` | Permite buscar corpos dentro do orçamento |
| `include_seen` | boolean | não | `false` | Reenvia itens que a sessão já recebeu; só tem efeito com contexto incremental ([D-029](changelog.md#d-029--contexto-incremental-por-sessão-proposta)) |

#### Planejamento upstream esperado

| Sinal de entrada | Operação principal |
| --- | --- |
| Stack trace ou erro estruturado | `from_trace` |
| Nome claro de símbolo | `find_symbol`; `impact` se a tarefa indicar mudança. Se o símbolo não existir, `explore` com a limitação `symbol_not_found` ([D-044](changelog.md#d-044--símbolo-inexistente-cai-para-explore)) |
| Tarefa conceitual ou multifile | `explore` |
| Pedido sobre documentação/decisão | `memory_recall` |
| Pedido de revisão (`review`, `revise`, `revisar`, `revisão`) | `situational_awareness` da working tree |
| Caso incerto | `explore` com orçamento conservador |

O caso incerto é o `orient` a que o modo `auto` chega por falta de qualquer sinal
(trace, revisão, documentação, mudança ou símbolo). O `explore` recebe metade do
orçamento, com piso de 256, e a resposta traz a limitação `route_uncertain`
(inferência do broker) sugerindo `mode` ou um símbolo. Com `mode=orient` explícito,
o `explore` recebe o orçamento inteiro
([D-024](changelog.md#d-024--revisão-reconhecida-no-modo-auto),
[D-025](changelog.md#d-025--orçamento-conservador-no-caso-incerto)).

#### Saída

```json
{
  "schema_version": "ripwire-broker.context/v1",
  "tool": "context_for_task",
  "status": "ready",
  "intent": "change",
  "summary": "change: focus validateToken (src/auth.ts:42); 7 items, 2 tests, 1 limitation",
  "items": [
    {
      "kind": "symbol",
      "role": "primary",
      "path": "src/auth.ts",
      "line": 42,
      "symbol": "validateToken",
      "signature": "function validateToken(token: string): Claims",
      "why_included": "símbolo central para a tarefa",
      "source": { "verb": "find_symbol", "basis": "ripwire" },
      "content": { "untrusted_repository_data": "..." }
    }
  ],
  "tests": [
    {
      "path": "test/auth.test.ts",
      "run": "npm test -- test/auth.test.ts",
      "why_included": "alcança validateToken",
      "source": { "verb": "impact", "basis": "ripwire" }
    }
  ],
  "risks": [],
  "limitations": [
    { "kind": "counts_floor", "detail": "...", "source": { "verb": "impact", "basis": "ripwire" } }
  ],
  "provenance": {
    "request_id": 7,
    "upstream_tools": ["find_symbol", "fetch_body", "impact"],
    "workspace": "/repo",
    "ripwire_version": "0.6.4",
    "broker_version": "0.1.0"
  },
  "budget": {
    "requested_tokens": 2500,
    "estimated_tokens": 2310,
    "truncated": false,
    "shown": 9,
    "omitted": 0
  }
}
```

Notas sobre o envelope implementado:

- `role` segue o §10.1. `source.basis` separa fatos do Ripwire (`ripwire`) de
  inferências do broker (`broker_inference`), conforme RF-08.
- Todo texto do repositório fica em `content.untrusted_repository_data` (§15.2).
- Com `truncated: true`, `budget` traz `next_step` (RF-05).
- Com um modelo local configurado (§10.3), `notes[]` traz até três notas
  arquiteturais geradas, uma por módulo dos itens incluídos. Cada nota traz `scope`,
  o `text` em `untrusted_repository_data`, `generated`, `model`, `derived_from`,
  `cached` e `source.basis: local_model`. O campo é aditivo ao v1 e é omitido sem
  modelo.
- Com contexto incremental, um item já entregue e inalterado volta como referência enxuta
  (sem `content` nem `signature`). `budget.already_delivered` conta os testes e riscos
  não repetidos. Esse campo é aditivo ao v1 e é omitido quando vale 0
  ([D-037](changelog.md#d-037--extensões-aditivas-ao-envelope-v1-proposta)).
- `summary` é inferência determinística feita só com nomes, caminhos e contagens
  ([D-016](changelog.md#d-016--resumo-limite-por-item-e-orçamento-mínimo)).
- A estimativa é `ceil(bytes do JSON serializado / 4)` sobre o envelope inteiro.
  O orçamento mínimo é 256
  ([D-007](changelog.md#d-007--orçamento-e-estimativa-de-tokens)).

### 9.2 Tool `context_after_edit`

**Objetivo:** mostrar o que a edição pode ter afetado sem reler todo o contexto inicial.

#### Entrada

| Campo | Tipo | Obrigatório | Padrão | Descrição |
| --- | --- | --- | --- | --- |
| `files` | array de string | não | working tree | Arquivos alterados conhecidos pelo host |
| `symbols` | array de string | não | `[]` | Símbolos deliberadamente modificados |
| `budget_tokens` | integer | não | `1500` | Limite do delta de contexto |
| `include_seen` | boolean | não | `false` | Reenvia itens que a sessão já recebeu |

#### Operações upstream

- `situational_awareness` para raio de impacto, testes e co-change;
- `edit_check` para cada símbolo informado, até limite configurável;
- `impact` apenas quando necessário para esclarecer uma alteração de alto risco.

#### Saída principal

- contratos alterados;
- callers potencialmente incompatíveis;
- arquivos alcançados;
- parceiros de co-change ausentes;
- testes relacionados;
- hotspots e limitações.

### 9.3 Tool `context_before_finish`

**Objetivo:** produzir um gate informativo antes de o agente declarar a tarefa concluída.

#### Entrada

| Campo | Tipo | Obrigatório | Padrão | Descrição |
| --- | --- | --- | --- | --- |
| `budget_tokens` | integer | não | `1800` | Limite da resposta |
| `include_test_commands` | boolean | não | `true` | Inclui comandos quando conhecidos |
| `strict` | boolean | não | `false` | Também trata achados menores (`sev=minor`) como bloqueantes |

#### Operações upstream

- `situational_awareness` para consolidar impacto, testes e co-change, e obter os
  arquivos alterados;
- `quality_delta`;
- `affected`, semeado com os arquivos alterados, quando houver algum.

#### Estados de saída

| Estado | Significado |
| --- | --- |
| `ready` | Nenhuma regressão conhecida; obrigações satisfeitas ou informadas |
| `attention_required` | Regressão introduzida, contrato quebrado ou co-change ausente; com `strict=true`, também achados menores |
| `unknown` | Ripwire não conseguiu estabelecer evidência suficiente, e nenhum dos motivos acima foi detectado |

Toda regressão conhecida leva a `attention_required`, independentemente de
`strict`, como exige o CA-05. Testes a rodar são obrigações **informadas** em
`tests[]` e não bloqueiam o gate. Uma regressão conhecida prevalece sobre a falta de
evidência ([D-013](changelog.md#d-013--context_before_finish-e-semântica-de-strict)).

O estado `ready` nunca significa “código correto”; significa apenas “nenhuma
obrigação detectada pelo broker permanece aberta”.

### 9.4 Resource `ripwire-broker://status`

Retorna somente dados operacionais não sensíveis:

- versão do broker;
- versão e disponibilidade do Ripwire;
- versão MCP negociada;
- workspace fixado, com opção de redigir o caminho;
- quantidade de chamadas e reinícios;
- último erro upstream, só o tipo (as mensagens podem citar símbolos e caminhos);
- configuração de orçamento;
- modo offline e política de telemetria;
- métricas locais (§16.1) e as últimas 32 requisições (`recent_requests`), cada uma
  com `request_id`, tool, resultado, duração e chamadas upstream (verbo, duração e
  resultado). O `request_id` é o mesmo de `provenance.request_id` (§14.2).

---

## 10. Estratégia de enriquecimento

### 10.1 Enriquecimento determinístico do MVP

O MVP não usa LLM. Ele enriquece por composição de evidências:

1. normaliza entidades equivalentes retornadas por verbos diferentes;
2. atribui papéis: `primary`, `caller`, `callee`, `test`, `config`, `doc`, `risk`.
   No MVP, testes vão para `tests[]` e riscos para `risks[]`. Os papéis `config` e
   `risk` ficam reservados no schema v1, porque nenhum verbo usado hoje distingue
   arquivos de configuração;
3. calcula prioridade a partir do ranking e da função do item na tarefa;
4. registra `why_included` com justificativa determinística;
5. agrupa relações em linguagem curta;
6. associa testes aos símbolos/arquivos que alcançam;
7. promove ambiguidades e limitações para campos de primeira classe;
8. remove repetições de assinaturas, corpos e caminhos;
9. preserva a evidência original como proveniência.

### 10.2 Ordem de seleção sob orçamento

1. limitações que alteram a interpretação da resposta;
2. símbolo ou arquivo central;
3. assinatura e trecho central, quando necessário;
4. callers/callees diretamente relevantes;
5. contratos e usos que podem quebrar;
6. testes alcançáveis;
7. arquivos de co-change;
8. documentação relacionada;
9. contexto periférico.

### 10.3 Enriquecimento semântico opcional futuro

Uma versão posterior pode gerar resumos com modelo local, desabilitado por
padrão. Requisitos:

- execução inteiramente local ou endpoint explicitamente configurado;
- resumo sempre derivado de evidências incluídas;
- links de proveniência preservados;
- cache separado do cache interno do Ripwire;
- invalidação por hash de origem e versão do modelo;
- marcação clara de conteúdo gerado;
- fallback determinístico integral quando o modelo não estiver disponível.

**Estado (Fase 3):** implementado.
- **Modelo:** CLI local por subprocesso (`--summarizer-cmd`), sem shell e sem crate de
  rede, o que preserva o CA-10. Validado com `ollama run --nowordwrap phi4`.
- **Evidência:** as notas saem só dos itens incluídos, e `derived_from` lista esses
  itens.
- **Cache:** fica em memória, endereçado por
  `sha256(versão do prompt, modelo, módulo, evidência)`. A versão do modelo entra na chave
  pelo hash da saída de `--summarizer-version-cmd`. Sem ele, o requisito fica
  parcialmente atendido, e o `doctor` avisa.
- **Espera:** a resposta espera no máximo `--summarizer-wait-ms`. Depois disso, a nota
  continua em segundo plano, com no máximo uma geração por vez.
- **Fallback:** as limitações `note_pending`, `summarizer_unavailable` e `notes_omitted`
  mantêm a resposta determinística intacta.
- **Pendência:** o cache em disco foi adiado até a medição do §21.3.
- Detalhes: [D-034](changelog.md#d-034--modelo-local-por-subprocesso-proposta) a
  [D-036](changelog.md#d-036--espera-limitada-e-fallback-proposta),
  [D-046](changelog.md#d-046--fase-3-com-cache-em-memória) e
  [D-048](changelog.md#d-048--ponto-de-parada-4-fase-3).

---

## 11. Estratégia de redução de tokens

### 11.1 Mecanismos

- expor três tools em vez de toda a superfície upstream;
- carregar corpos apenas por necessidade;
- usar contexto incremental depois da primeira orientação;
- evitar repetição de símbolos e explicações;
- impor limites diferentes por fase;
- omitir contexto já entregue na mesma sessão quando não mudou;
- preferir assinaturas e relações a arquivos completos;
- encerrar seleção quando o ganho marginal de relevância cair;
- relatar truncamento em vez de ultrapassar silenciosamente o orçamento.

### 11.2 Redução da superfície de tools

Na documentação atual, o MCP do Ripwire expõe dezenas de verbos. Dependendo do
cliente, schemas de tools podem ocupar contexto do modelo. O broker reduz a
superfície estável para três tools e um resource.

Exemplo meramente ilustrativo, a ser medido no cliente real:

```text
33 schemas × 150 tokens médios = 4.950 tokens de descrição
 3 schemas × 220 tokens médios =   660 tokens de descrição
economia lógica aproximada       = 4.290 tokens
```

Essa conta não é uma promessa de cobrança, pois clientes podem cachear,
comprimir ou fazer descoberta progressiva. A métrica real deverá ser coletada
por host.

### 11.3 Referências externas de eficiência

Os números publicados pelo Ripwire indicam, em seu próprio repositório, que um
pacote de tarefa pode ocupar cerca de 2,1 mil tokens contra 16–80 mil de leituras
integrais candidatas; consultas de callers e situação de diff mostram reduções
ainda maiores em cenários medidos. Esses números são evidência do potencial do
mecanismo, não metas automaticamente herdadas pelo broker.

O Graft também publicou um experimento em que contexto empurrado ao agente
reduziu a média de tokens de 8.070 para 4.650 em seu corpus controlado. Isso
apoia a hipótese de contexto antecipado, mas não é diretamente comparável ao
`ripwire-broker`.

### 11.4 Metas iniciais do broker

| Métrica | Meta MVP |
| --- | --- |
| Respeito ao orçamento solicitado | 100% das respostas |
| Orçamento padrão de início | ≤ 2.500 tokens estimados |
| Orçamento padrão pós-edição | ≤ 1.500 tokens estimados |
| Orçamento padrão pré-conclusão | ≤ 1.800 tokens estimados |
| Redução de chamadas exploratórias | ≥ 30% no corpus de avaliação |
| Redução de tokens de exploração | ≥ 35% sem reduzir correção |
| Repetição de item idêntico na mesma resposta | 0 |

---

## 12. Exemplos de uso

### 12.1 Adicionar autenticação a uma nova rota

**Pedido:** “Adicione autenticação à rota de exportação.”

#### Sem broker

O agente pode procurar `auth`, abrir middleware, router, controller e testes,
mas ainda deixar de encontrar uma rota irmã ou uma política aplicada por
configuração. Um fluxo típico pode consumir várias buscas e arquivos completos.

#### Com broker

`context_for_task` chama `explore`, identifica o middleware usado pelas rotas
equivalentes, inclui um exemplar testado, mostra os callers e lista os testes que
alcançam o caminho.

**Melhoria de qualidade esperada:** implementação consistente com o padrão do
repositório e menor risco de proteger apenas parte do fluxo.

**Redução de tokens esperada:** assinaturas, dois corpos centrais e testes
substituem a leitura integral de router, middleware, controllers e suíte de
testes. Hipótese de avaliação: 2–4 mil tokens em vez de 10–20 mil.

### 12.2 Alterar a assinatura de uma função compartilhada

**Pedido:** “Faça `calculatePrice` receber a moeda explicitamente.”

#### Sem broker

O agente encontra a definição, altera os callers visíveis e executa o teste mais
próximo pelo nome. Calls indiretas ou outro módulo consumidor podem ser
ignorados.

#### Com broker

No início, `context_for_task` inclui contrato, usos e impacto. Depois da edição,
`context_after_edit` chama `edit_check` e destaca callers incompatíveis. Antes de
concluir, `context_before_finish` lista os testes que alcançam a mudança.

**Melhoria de qualidade esperada:** menos quebras de contrato e maior cobertura
dos consumidores reais.

**Redução de tokens esperada:** o agente recebe o delta de impacto, sem reler
todos os arquivos que já viu.

### 12.3 Corrigir bug a partir de uma stack trace

**Pedido:** stack trace com sete frames internos e externos.

#### Sem broker

O agente pesquisa cada nome, abre arquivos e tenta decidir manualmente qual
frame pertence ao repositório e qual é a causa provável.

#### Com broker

O roteador detecta a forma de stack trace, chama `from_trace`, mantém os frames
internos na ordem adequada e inclui o corpo do símbolo interno mais relevante.

**Melhoria de qualidade esperada:** investigação começa no frame correto e evita
alteração em wrapper sintomático.

**Redução de tokens esperada:** um pacote delimitado substitui várias buscas por
frame e leituras de arquivos inteiros.

### 12.4 Evitar nova dívida antes de concluir

**Situação:** a funcionalidade funciona, mas a edição duplicou uma regra e
adicionou um bloco de tratamento de erro que mascara falhas.

#### Sem broker

O agente executa testes e declara sucesso, porque a suíte não mede a piora
estrutural.

#### Com broker

`context_before_finish` chama `quality_delta`, retorna apenas regressões criadas
pela working tree e marca `attention_required`.

**Melhoria de qualidade esperada:** correção acontece enquanto o contexto da
mudança ainda está ativo, em vez de virar dívida para revisão posterior.

**Redução de tokens esperada:** a análise focaliza o delta introduzido, não um
relatório amplo de toda a dívida preexistente.

### 12.5 Encontrar testes relevantes em mudança multifile

**Situação:** a alteração toca serviço, adaptador e serializador; os testes não
possuem os mesmos nomes dos arquivos de produção.

`context_after_edit` usa alcance do grafo para apresentar testes relacionados e
comandos conhecidos. Isso reduz a dependência de heurísticas de nome e diminui
o risco de executar apenas o teste mais óbvio.

---

## 13. Requisitos funcionais

### RF-01 — Inicialização upstream

O broker deve iniciar ou conectar-se a um servidor MCP do Ripwire, negociar
capacidades e validar os verbos necessários.

### RF-02 — Workspace fixado

O broker deve operar somente dentro da raiz canonicalizada definida na
inicialização. Paths externos, symlinks de escape e roots ambíguos devem ser
recusados.

### RF-03 — Descoberta compatível

O broker deve descobrir capacidades upstream e falhar com diagnóstico claro
quando a versão instalada do Ripwire não fornecer um verbo obrigatório.

### RF-04 — Roteamento de intenção

`context_for_task` deve distinguir, no mínimo:

- orientação conceitual;
- símbolo conhecido;
- tarefa de alteração;
- stack trace/erro;
- documentação/decisão;
- revisão.

### RF-05 — Orçamento obrigatório

Nenhuma tool deve retornar mais conteúdo do que o orçamento solicitado. Toda
redução deve declarar `truncated`, itens mostrados/omitidos e próximo passo.

### RF-06 — Deduplicação

Um mesmo símbolo, corpo, teste ou diagnóstico não pode aparecer mais de uma vez
na resposta final.

### RF-07 — Corpos sob demanda

O broker deve preferir handles e assinaturas e usar `fetch_body` somente quando
o corpo melhorar materialmente a resposta e couber no orçamento.

### RF-08 — Proveniência

Cada item deve conter o verbo upstream e a localização de origem. Inferências do
broker devem ser identificadas separadamente de fatos do Ripwire.

### RF-09 — Preservação de limitações

Ambiguidade, truncamento, contagens parciais e arquivos não indexados não podem
ser convertidos silenciosamente em certeza ou ausência.

### RF-10 — Saída estável

A saída estruturada deve possuir versionamento de schema e manter compatibilidade
retroativa durante a mesma versão major.

### RF-11 — Integração orientada ao momento

O pacote deve incluir uma skill ou instrução de instalação que ensine o agente a
chamar:

- `context_for_task` antes da exploração;
- `context_after_edit` depois de uma mudança relevante;
- `context_before_finish` antes de declarar conclusão.

### RF-12 — Degradação segura

Se o Ripwire estiver indisponível, a tool deve retornar erro estruturado e não
inventar contexto. O agente pode seguir sem broker, desde que a indisponibilidade
fique explícita.

### RF-13 — Status operacional

O resource de status deve permitir diagnóstico sem expor prompt, código ou
conteúdo das respostas.

### RF-14 — Cancelamento e timeout

Chamadas devem respeitar cancelamento do cliente e timeouts configuráveis. O
processo upstream travado deve ser encerrado e reiniciado de forma controlada.

Estado no MVP: timeout (`--timeout-ms`) e reinício controlado estão implementados e
testados. O cancelamento vindo do cliente ainda não tem teste automatizado e não é
repassado ao Ripwire, que não expõe essa operação. A pendência está registrada em
[D-021](changelog.md#d-021--pendências-conhecidas).

---

## 14. Requisitos não funcionais

### 14.1 Performance

- overhead próprio do broker, excluindo o tempo upstream: p95 menor que 50 ms para respostas de até 5 mil tokens;
- processo Ripwire persistente para evitar cold start por tool;
- no máximo uma reconstrução upstream para o mesmo estado do workspace;
- respostas incrementais após edição devem evitar recomputar contexto inicial sem necessidade.

### 14.2 Confiabilidade

- reinício automático do processo upstream uma vez por falha recuperável;
- nenhuma repetição automática para erro de entrada ou recusa semântica;
- correlação entre chamada do cliente e chamadas upstream: `provenance.request_id` no
  envelope e `recent_requests` no status (§9.4), mantida por requisição mesmo com
  chamadas concorrentes ([D-026](changelog.md#d-026--correlação-entre-tool-call-e-chamadas-upstream));
- testes de contrato contra versões suportadas do Ripwire.

### 14.3 Portabilidade

- macOS e Linux no MVP;
- distribuição como binário Rust produzido por `cargo build --release`;
- toolchain Rust fixada no repositório e dependências registradas em `Cargo.lock`;
- caminhos tratados de forma independente de shell;
- execução de processo por array de argumentos, nunca por concatenação com `sh -c`;
- Windows avaliado em fase posterior.

### 14.4 Manutenibilidade

- integração MCP concentrada em um adapter baseado em `rust-mcp-sdk` 2.0.0;
- adaptador upstream isolado do modelo de saída;
- regras de roteamento explícitas e testáveis. No MVP, as listas de palavras são
  constantes em `src/router.rs`, cobertas por testes, e `mode` permite contornar o
  roteador. Tornar as regras configuráveis por arquivo fica para quando a avaliação
  A/B mostrar a necessidade;
- fixtures de respostas do Ripwire;
- testes de propriedade para orçamento e renderização: o orçamento nunca é
  excedido, nenhum item se repete, limitações são preservadas e nenhum conteúdo
  sensível vaza no status. Esses testes rodam sobre fixtures gravadas e substituem
  golden files byte a byte, que quebrariam a cada ajuste de texto;
- nenhum acoplamento ao formato binário do cache Ripwire.

---

## 15. Segurança e privacidade

### 15.1 Postura do MVP

- local por `stdio`;
- sem listener de rede;
- sem telemetria remota;
- sem LLM remoto;
- sem tools de escrita;
- workspace único e fixado;
- logs sem código, prompt ou corpo de resposta por padrão.

### 15.2 Conteúdo não confiável

Código, comentários, documentação e mensagens de erro podem conter texto que
pareça instrução ao agente. O broker deve:

- delimitar conteúdo recuperado como `untrusted_repository_data`;
- nunca promover texto do repositório a instrução de sistema;
- escapar formatos de saída;
- separar fatos, inferências e recomendações;
- limitar tamanho por item;
- não executar comandos encontrados no repositório.

### 15.3 Execução segura do Ripwire

- binário configurado por caminho explícito ou resolução validada;
- argumentos passados sem shell intermediário;
- versão mínima e allowlist de verbos. A mínima é o Ripwire 0.6.4, recusado na
  conexão com `incompatible_upstream`; uma versão ilegível não recusa, e os verbos
  obrigatórios decidem ([D-023](changelog.md#d-023--versão-mínima-do-ripwire));
- ausência de acesso aos três verbos upstream de edição;
- timeouts e limite de memória configuráveis. No MVP só o timeout está
  implementado; o limite de memória está pendente
  ([D-021](changelog.md#d-021--pendências-conhecidas));
- stderr tratado como diagnóstico, não como instrução.

### 15.4 Transporte remoto futuro

Se Streamable HTTP for implementado:

- loopback permanece padrão;
- bind roteável exige configuração explícita;
- autenticação usa bearer token vindo de variável de ambiente;
- TLS é obrigatório diretamente ou por reverse proxy;
- edição remota permanece indisponível;
- logs e métricas não armazenam payloads;
- rate limit e limites de tamanho são obrigatórios.

---

## 16. Observabilidade e avaliação

### 16.1 Métricas locais

| Métrica | Conteúdo sensível? | Finalidade |
| --- | --- | --- |
| Duração total e upstream | não | Identificar latência do broker e Ripwire |
| Quantidade de chamadas upstream | não | Medir eficiência da orquestração |
| Verbo upstream | baixo | Entender roteamento; não é desabilitável no MVP, porque as métricas ficam só em memória e só saem pelo resource de status local |
| Tokens solicitados/estimados | não | Validar orçamento |
| Itens mostrados/omitidos | não | Calibrar truncamento |
| Cache hit lógico do broker | não | Medir reaproveitamento: `metrics.session_hits`, os itens, testes e riscos não reenviados porque a sessão já os tinha ([D-040](changelog.md#d-040--ponto-de-parada-1-contexto-incremental-no-núcleo)) |
| Status final | não | Medir `ready`, atenção e desconhecido |
| Notas do modelo local | não | `summarizer.generated`, `cache_hits`, `pending`, `failures` e `cached_notes`: só contagens, nunca texto de nota ou prompt |

Prompt, código, caminhos, símbolos e respostas não são registrados por padrão.

### 16.2 Avaliação A/B

Cada corpus deve executar a mesma tarefa em três braços:

1. agente sem Ripwire;
2. agente com Ripwire MCP direto;
3. agente com `ripwire-broker`.

### 16.3 Métricas de qualidade

- arquivos corretos encontrados em top-k;
- completude dos arquivos modificados contra patch de referência;
- contratos quebrados detectados;
- testes relevantes identificados;
- regressões introduzidas após testes;
- número de buscas e leituras adicionais;
- correção final da tarefa;
- falsos alertas que causaram exploração desnecessária.

### 16.4 Métricas de eficiência

- tokens de entrada totais;
- tokens de tool schemas;
- tokens de resultados de tools;
- quantidade de tool calls;
- tempo até a primeira edição correta;
- tempo total até testes aprovados;
- latência acrescentada pelo broker.

---

## 17. Critérios de sucesso

O MVP é considerado bem-sucedido quando, em um corpus com pelo menos 30 tarefas
reais e multifile de três repositórios:

1. reduz em pelo menos 35% os tokens gastos em exploração contra o agente sem contexto;
2. reduz em pelo menos 30% as chamadas exploratórias;
3. mantém ou melhora a taxa de conclusão correta;
4. melhora a recuperação de todos os arquivos necessários em tarefas multifile;
5. identifica pelo menos 80% dos testes presentes no patch ou processo de referência, quando alcançáveis pelo grafo;
6. respeita o orçamento em 100% das respostas;
7. não entrega contexto stale conhecido;
8. não realiza nenhuma chamada de rede no modo padrão;
9. não modifica arquivos do workspace;
10. acrescenta menos de 50 ms p95 de processamento próprio em respostas de até 5 mil tokens.

As metas devem ser reportadas com corpus, versões, hardware, prompts e
contraprovas. Uma economia que diminua correção não é sucesso.

---

## 18. Critérios de aceite do MVP

### CA-01 — Inicialização

**Dado** um workspace válido e um Ripwire compatível, **quando** o broker inicia,
**então** conecta-se ao upstream, valida capabilities e publica três tools e um
resource.

### CA-02 — Orientação sob orçamento

**Dado** um pedido conceitual e orçamento de 2.500 tokens, **quando**
`context_for_task` é chamado, **então** a resposta apresenta símbolos, contexto e
testes relevantes sem ultrapassar o limite e informa qualquer corte.

### CA-03 — Stack trace

**Dada** uma stack trace, **quando** `context_for_task` é chamado em modo `auto`,
**então** o broker usa a rota de erro, preserva a ordem útil dos frames internos e
inclui o corpo mais relevante quando couber.

### CA-04 — Pós-edição

**Dada** uma working tree alterada, **quando** `context_after_edit` é chamado,
**então** retorna impacto, contratos, co-change e testes sem repetir o contexto
inicial completo.

### CA-05 — Gate de conclusão

**Dada** uma regressão detectada por `quality_delta`, **quando**
`context_before_finish` é chamado, **então** retorna `attention_required`, descreve
a evidência e não declara o código incorreto como pronto.

### CA-06 — Limitação preservada

**Dado** um resultado upstream ambíguo ou parcial, **quando** for normalizado,
**então** a resposta mantém a limitação em campo estruturado.

### CA-07 — Upstream indisponível

**Dado** um processo Ripwire indisponível, **quando** uma tool é chamada, **então**
o broker retorna erro estruturado, tenta no máximo um reinício recuperável e não
fabrica contexto.

### CA-08 — Isolamento de workspace

**Dado** um path fora da raiz, **quando** uma tool tenta acessá-lo, **então** a
requisição é recusada antes da chamada upstream.

### CA-09 — Read-only

**Quando** o cliente lista as tools, **então** nenhuma operação capaz de editar o
workspace ou criar baseline aparece na superfície pública.

### CA-10 — Offline

**Dado** o modo padrão, **quando** a suíte de integração executa, **então** nenhuma
conexão de rede é necessária ou iniciada.

---

## 19. Roadmap

### Fase 0 — Spike técnico

- criar o workspace Rust e fixar `rust-mcp-sdk` 2.0.0;
- validar no mesmo processo os recursos `server`, `client`, `macros` e `stdio`;
- iniciar Ripwire MCP como child process;
- executar `explore`, `situational_awareness` e `quality_delta`;
- medir schemas, payloads e latência;
- validar cancelamento e reinício. O reinício foi validado; o cancelamento ficou
  pendente ([D-021](changelog.md#d-021--pendências-conhecidas)).

### Fase 1 — MVP

- três tools e resource de status;
- workspace único;
- roteamento determinístico;
- orçamento, deduplicação e proveniência;
- skill de uso com Codex e Claude Code;
- métricas locais;
- testes de contrato e segurança.

### Fase 2 — Automação por host

- hooks/wrappers para clientes prioritários;
- contexto incremental por sessão;
- opt-out e visualização do conteúdo injetado;
- instalação e diagnóstico automatizados.

Estado: implementada ([plano](plan-fases-2-3.md), D-040 a D-045). O contexto
incremental vem desligado no `serve` (`--incremental`) e ligado nos hooks. `install` é
dry-run por padrão, e `doctor` verifica a instalação.

### Fase 3 — Enriquecimento local opcional

- sumarização por modelo local;
- cache semântico próprio;
- notas arquiteturais derivadas com proveniência;
- fallback determinístico.

Estado: implementada, com o cache em memória (D-046 a D-048). O cache em disco depende
da medição do §21.3.

### Fase 4 — Times e CI

- Streamable HTTP autenticado;
- múltiplos workspaces isolados por processo;
- políticas por organização;
- artefatos de avaliação e dashboards agregados sem conteúdo sensível.

---

## 20. Riscos e mitigações

| Risco | Impacto | Mitigação |
| --- | --- | --- |
| Contexto automático irrelevante | Mais tokens e distração | Threshold de relevância, orçamento curto e opt-out |
| Mudança no contrato MCP do Ripwire | Quebra de integração | Capability discovery, adapter isolado e testes por versão |
| Contexto stale após edição | Decisão baseada em código antigo | Handles content-addressed e nova consulta após mudança |
| Broker esconder limitações | Confiança indevida | `limitations[]` obrigatório e golden tests |
| Roteador escolher verbo errado | Resposta incompleta | `mode` manual, fallback para `explore` e métricas de rota |
| Tool schemas ainda consumirem tokens | Economia menor que a esperada | Três schemas pequenos e medição por host |
| Prompt injection em comentários/docs | Agente segue instrução maliciosa | Delimitação como dados não confiáveis e separação de papéis |
| Processo Ripwire travar | Bloqueio da sessão | Timeout, cancelamento e um reinício controlado |
| Relatório de qualidade com falso positivo | Trabalho desnecessário | Evidência explícita, estado `unknown` e não editar automaticamente |
| Escopo crescer para outro analisador | Atraso e duplicação | Não objetivo explícito: Ripwire permanece fonte estrutural |

---

## 21. Decisões em aberto

### 21.1 Host prioritário para automação

Escolher o primeiro alvo entre:

- Codex;
- Claude Code;
- ambos desde o MVP.

Essa decisão altera o formato da skill, os hooks disponíveis e o mecanismo de
instalação, mas não muda o núcleo MCP.

**Resolvido no MVP:** ambos. Há uma skill para o Claude Code e instruções em
`AGENTS.md` com `config.toml` para o Codex
([D-020](changelog.md#d-020--integração-com-agentes-e-toolchain)).

**Resolvido na Fase 2:** hooks nos dois hosts ao mesmo tempo. O Codex 0.157 tem hooks
com o mesmo contrato do Claude Code
([D-030](changelog.md#d-030--hooks-nos-dois-hosts-proposta),
[D-039](changelog.md#d-039--aprovação-do-plano)).

### 21.2 Granularidade da automação

Definir se o modo padrão será:

- tool chamada pelo agente por skill;
- injeção automática apenas no primeiro prompt;
- injeção automática no primeiro prompt e depois de edições;
- wrapper explícito opt-in.

**Resolvido na Fase 2:**
- O padrão com hooks é injetar no primeiro prompt e depois de edições.
- A skill continua valendo para quem não instala hooks.
- O wrapper `prompt` fica disponível para outros clientes.
- `--every-prompt` e o gate de `Stop` (`--gate`) são opt-in
  ([D-031](changelog.md#d-031--granularidade-da-automação-proposta)).

### 21.3 Cache próprio

Validar se a deduplicação por sessão é suficiente antes de criar cache
persistente. O MVP deve começar sem cache semântico persistente.

**Estado:** a deduplicação por sessão está implementada, e `session_hits` a mede. A
Fase 3 começou com o cache de notas só em memória
([D-046](changelog.md#d-046--fase-3-com-cache-em-memória)). O cache persistente
continua pendente dessa medição em uso real.

### 21.4 Política de falha

Definir se falha do broker:

- permite que o agente continue com aviso; ou
- bloqueia tarefas marcadas como `strict`.

Recomendação inicial: continuar com aviso no uso interativo e falhar em CI
somente quando `strict=true`.

**Estado no MVP:** o broker nunca bloqueia sozinho. Uma falha vira erro
estruturado (`upstream_unavailable`, `incompatible_upstream`...) e o gate responde
`unknown` quando falta evidência. A skill orienta o agente a continuar avisando o
usuário. Falhar em CI com `strict=true` depende do runner de CI da Fase 4, e por
isso a decisão continua em aberto.

---

## 22. Referências

- [Ripwire — README e referência de comandos](https://github.com/redhat-et/ripwire)
- [Ripwire — MCP skill/reference](https://github.com/redhat-et/ripwire/tree/main/skills/ripwire-mcp)
- [`rust-mcp-sdk` 2.0.0 — crates.io](https://crates.io/crates/rust-mcp-sdk)
- [`rust-mcp-sdk` — código, exemplos e guia de upgrade](https://github.com/rust-mcp-stack/rust-mcp-sdk)
- [Graft — contexto automático e integração com agentes](https://github.com/trailhq/Graft)
- [jevgrep — README, instalação, privacidade e resultados](https://github.com/dzhng/jevgrep)
- [jevgrep — arquitetura de recuperação](https://github.com/dzhng/jevgrep/blob/main/docs/architecture.md)
- [Model Context Protocol — Architecture Overview](https://modelcontextprotocol.io/docs/2026-07-28/learn/architecture)
- [Model Context Protocol — Specification](https://modelcontextprotocol.io/specification/2026-07-28)

### 22.1 Nota sobre benchmarks

Benchmarks citados neste PRD foram publicados pelos próprios projetos e usam
corpora, versões e metodologias diferentes. Eles justificam a hipótese do
produto, mas não substituem a avaliação A/B específica do `ripwire-broker`.
