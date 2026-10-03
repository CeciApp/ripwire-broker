# PRD: memória persistente com Jev no ripwire-broker

**Versão:** 0.2 · **Data:** 2026-10-03 · **Status:** proposta para revisão, não implementada.  
**Base do broker:** `3b60a1b29252485bc43a3e2b0ee914203dde9211`, pacote `0.1.0`.  
**Implementação:** Rust; System One remoto Jev `jev-1.13.0`; System Two é o agente do host, com sumarizador local opcional para notas derivadas.

## 1. Escopo e condição documental

Adicionar memória de trabalho entre sessões, isolada por workspace/worktree, para recuperar observações anteriores relevantes à tarefa. Ripwire continua responsável pelos fatos estruturais atuais; Jev classifica e relaciona observações; o host raciocina sobre o contexto entregue.

Este documento é uma **adaptação de produto inspirada em Jev-Mem**, não uma reprodução do experimento. Os parâmetros novos abaixo são decisões propostas e metas a medir, não resultados observados.

**Pendência da revisão solicitada:** neste checkout não foram encontrados `docs/2026-10-03_Jev-Mem.pdf` nem `docs/jev-mem.md`, inclusive na busca por esses nomes nos projetos e worktrees locais. Foi lido o PDF encontrado em `/Users/aquental/Downloads/Jev-Mem.pdf`, arXiv `2609.23986v1`, 16 páginas, SHA-256 `413c592431ce55f8372c1561b7dd572e3603d0e66b447f6e4097e4678bfc5e87`. Não foi possível confirmar que é a mesma cópia solicitada nem certificar o estudo linha a linha. A auditoria abaixo confronta os trechos e problemas transcritos pelo solicitante com esse PDF, as fontes dos autores e o broker. O PRD deve passar por uma última reconciliação quando o estudo estiver disponível; não atribuir a ele uma revisão integral já concluída.

O [PRD principal](../spec/ripwire-broker-mcp.md) permanece inalterado. Conflitos normativos estão enumerados no §16 para eventual incorporação somente após revisão do mantenedor. Esta entrega não altera código, dependências, configuração do host nem ativa chamadas remotas.

## 2. Auditoria de consistência e limites da evidência

### 2.1 O que os resultados publicados significam

Dongming Jiang, Yi Li e Bingzhe Li apresentam Jev-Mem como separação entre controle tipado, memória multirrelacional e raciocínio generativo. O PDF, Tabelas 1–2, relata LoCoMo com GPT-4o-mini: qualidade global `0,777`, construção em `158 s` e latência média de consulta `0,93 s`. Esta latência inclui recuperação **e geração da resposta**. O ganho de construção `6,6×` compara com Nemori (`1.044 s`); a redução de latência `36,7%` compara com MAGMA (`1,47 s`). Nenhum desses baselines é o ripwire-broker.

Conversas LoCoMo não representam diffs, paths, renames, hashes ou obrigações de testes. Filtrar entradas, retirar embeddings, mudar o agente, limitar recuperação e usar outro domínio invalida a transferência desses números. As métricas do produto serão produzidas pelo §14.

O próprio PDF contém divergências: §4.2 cita Multi-Hop `0,625`, Open-Domain `0,610`, Single-Hop `0,797` e empate temporal `0,650`; a Tabela 1 registra respectivamente `0,623`, `0,618`, `0,802` e `0,637`. MAGMA tem `0,650` temporal. A frase sobre vencer cinco de seis categorias também não corresponde às cinco categorias mais o agregado da tabela. Para referências quantitativas, usar as tabelas, registrando a divergência. O texto menciona dois benchmarks, mas as tabelas apresentadas são de LoCoMo; não atribuir resultados de LongMemEval a esse PDF.

### 2.2 Parecer sobre os nove pontos apresentados

| Ponto | Verificação | Consequência neste PRD |
|---|---|---|
| System One local “não existe” | Incorreto como afirmação sobre Jev-Mem. O README dos autores anuncia Laya local desde 27/09/2026, com PyTorch/MLX e sem chave TypeSafe | Jev é a escolha deste produto; Laya não será implementada nesta entrega |
| Admissão filtrada como algoritmo original | Appendix B.2 usa `admission_enabled=false`; typing não elimina observações | Filtro de segurança é desvio explícito, separado da classificação |
| Entidade e tempo | IDs exatos produzem ligação determinística; tempo de observação não é data do evento | Identidade composta e papéis temporais no §5; nenhum `mtime` vira narrativa |
| Custo e latência | Quatro Nouls de typing, relações por candidato e controle iterativo não são uma única decisão | Fila durável, SLO, limites de perguntas/tentativas e contabilidade no §8 |
| “Jev nunca autoriza merge” | Correto para código, incorreto se abranger nota derivada | `representation_merge`/`representation_promote` separados de merge Git |
| Cadência de 20 writes | Appendix B.3 usa 20 escritas Jev bem-sucedidas; isso não resolve crash nem baixa frequência | Contador e cursor duráveis, gatilho por idade e retomada no §9 |
| Schema ausente | Nomes informais de entradas não especificam `o_t` nem `x_t` | Contrato serializável, limites e renderização canônica no §5 |
| `role: memory` | `Role` atual não possui essa variante; hook serializa envelope em `additionalContext` | Campo explícito e apresentação textual; validação real por host no §11 |
| Comparação depreciativa com Mem0 | Não há comparação correspondente demonstrada nas fontes examinadas | Remover essa tese; comparar braços mensuráveis |

A revisão integral de `docs/jev-mem.md`, inclusive referências, fórmulas e eventuais problemas além dos itens transcritos, permanece pendente (§1).

### 2.3 Desvios obrigatoriamente declarados

| Eixo | Paper/perfil descrito | Broker proposto | Garantia/claim que não transfere |
|---|---|---|---|
| Admissão | Toda observação válida não vazia | Somente registro estruturado e elegível; rejeita segredos, PII identificável, prompt e diff brutos | Retenção integral da conversa e recall publicado |
| Domínio | Diálogo e eventos narrativos | Observações de análise do workspace | Qualidade `0,777` e generalização para código |
| Entidades | Identificadores exatos, alias inferido quando necessário | Identidade composta de arquivo/símbolo/revisão, sem identidade por nome isolado | Equivalência semântica de entidades do benchmark |
| Tempo | Timestamp de observação e referências temporais fundamentadas | Sequência de ingestão; commit/mtime apenas metadados; tempo de evento só explícito | Recall e precisão temporal publicados |
| Índices | Vetorial + lexical, âncoras híbridas | Primeira versão lexical + entidade + tempo, sem embedding novo | Reprodução da Eq. 16 e recall vetorial |
| System Two | Modelo separado gera resposta | Envelope entregue ao agente do host; sumarizador apenas para representação derivada | Latência `0,93 s` e qualidade do GPT-4o-mini |
| Recuperação | Budget 80, até 15 s, 16 tentativas | Até 12 expansões, 750 ms e 4 tentativas remotas | Cobertura do leitor original |
| Consolidação | 20 writes Jev bem-sucedidos | 20 enriquecimentos ou 24 h de pendências; execução retomável | Equivalência da cadência LoCoMo |
| Modelo | Modelo/perfil do experimento | Jev fixado na versão já integrada; sem fine-tune específico para código | Calibração e qualidade equivalentes no novo domínio |
| Falhas | Limites do experimento | Nó persistido antes do enriquecimento; respostas parciais declaradas | Grafo completo imediatamente após write |

Ausência de fine-tune é uma restrição deste produto, não evidência de que o paper exigiu fine-tune. Uma futura etapa vetorial requer PRD complementar com modelo, licença, pesos fixados, runtime Rust e orçamento; não instalar Python, PyTorch, MLX, Laya ou banco vetorial implicitamente.

## 3. Plataforma e ferramentas existentes

Versões resolvidas vêm de [Cargo.lock](../Cargo.lock), não de suposição sobre a versão mais recente. Preservar o lock e executar Cargo com `--locked`.

| Componente | Versão/base | Uso na implementação |
|---|---|---|
| Rust / edition | `1.98.1` / `2024` | Tipos, persistência, algoritmos e CLI; clippy/rustfmt do toolchain |
| ripwire-broker | `0.1.0`, commit do cabeçalho | Envelope, hooks, budget e avaliação |
| Ripwire MCP | mínimo `0.6.4`; binário encontrado `0.6.4`, `built_from=b343b9883` | Fatos, referências, análise estrutural e revalidação; guardar versão efetiva |
| rust-mcp-sdk | `=2.0.0` | Cliente/servidor stdio; manter as três tools públicas |
| tokio / tokio-util | `1.53.1` / `0.7.19` | Worker, fila, deadlines, cancelamento; não esperar HTTP sob lock |
| async-trait / futures-util | `0.1.92` / `0.3.34` | Fronteiras de transporte e execução assíncrona |
| serde / serde_json | `1.0.229` / `1.0.151` | Schemas versionados e JSON canônico controlado |
| sha2 | `0.10.9` | IDs, snapshots, integridade e chaves de cache |
| ignore | `0.4.33` | Reutilizar elegibilidade e exclusões, inclusive rechecagem antes de envio |
| libc | `0.2.189` | Flags de abertura segura já usadas no projeto |
| unicode-width | `0.2.2` | Statusline existente; não armazenar memória no snapshot visual |
| reqwest / secrecy | `0.12.28` / `0.10.3` | HTTP e credencial, exclusivamente na feature `online` |
| Jev / API | `jev-1.13.0`; `/v1/systemone`; OpenAPI consultado `0.2.0` | Nouls e Choice; preservar pin, sem alias móvel |
| tempfile / proptest / h2 | `3.27.0` / `1.11.0` / `0.4.19` | Fixtures, propriedades e testes locais do protocolo |
| ripwire-eval | binário do mesmo pacote/commit | Estender os braços e a instrumentação existentes |
| Sumarizador | `--summarizer-cmd`, versão capturada por `--summarizer-version-cmd` | Reutilizar execução sem shell; nenhum modelo gerativo obrigatório |
| CI | checkout `v7.0.1` SHA `3d3c42e5aac5ba805825da76410c181273ba90b1`; install-action `v2.87.21` SHA `4cef1412cce204788f482e778a0b9187f9626a29` | Preservar jobs obrigatórios `default` e `online` |
| nextest / cargo-deny | instalados pela CI sem pin próprio; ausentes no PATH desta revisão | Registrar versões efetivas na avaliação; não inventar versões fixadas |

Não adicionar dependência na primeira versão. `BTreeMap`, `BTreeSet`, `VecDeque`, arquivos privados e índices reconstruíveis atendem ao corpus limitado abaixo. O cliente HTTP existente deve ser ampliado; não instalar outro SDK Jev. Graft serve à navegação do desenvolvedor, não é dependência de runtime nem fonte substituta do Ripwire.

## 4. Ativação e fronteira de consentimento

Três control planes conceitualmente distintos: Jev remoto; Laya local existente no projeto dos autores, não implementada no broker; recuperação determinística sem System One, usada como controle experimental. O terceiro não é Jev-Mem equivalente.

**Decisão de ativação para este incremento:** `--memory` implica `--online`: ativa descoberta semântica de código e memória persistente controlada pelo Jev, sem exigir que o operador escreva as duas flags. Esta hierarquia foi aprovada pelo mantenedor para o PRD; é uma escolha do produto, não requisito do paper. Sem `--memory`, comportamento atual idêntico. `--online` sozinho não autoriza reter histórico nem processar jobs de memória antigos.

| Flags de execução | Comportamento |
|---|---|
| Nenhuma | Contexto estrutural offline, sem memória persistente |
| `--online` | Contexto estrutural e descoberta semântica Jev, sem memória persistente |
| `--memory` | Tudo de `--online`, mais coleta, recuperação e worker de memória |
| `--online --memory` | Mesmo comportamento de `--memory`; sem duplicar clientes, workers ou chamadas |

Na configuração efetiva do processo, resolver `online_enabled = online_flag || memory_flag` e `memory_enabled = memory_flag` antes de validar credencial e construir serviços. Preservar também a origem da ativação para diagnóstico: online explícito ou implicado por memória. Não alterar as regras atuais que determinam quando a descoberta semântica participa de uma consulta.

Ambos os modos exigem binário compilado com a feature Cargo `online` e credencial válida. A flag de execução não habilita uma feature ausente do binário: `--memory` em build default falha claramente, assim como `--online`. Credencial ausente ou inválida não causa downgrade silencioso. No modo normal de execução, ausência de ambas as flags significa zero chamada ao Jev; probes e subcomandos remotos explicitamente solicitados mantêm seus contratos próprios.

Descoberta e memória compartilham cliente HTTP, autenticação e controle global de concorrência. Mantêm budgets de operação, caches e métricas separados, respeitando também os tetos agregados do §8. Ativar memória não cria uma segunda cota de concorrência nem duplica a descoberta.

| Superfície proposta | Default / regra |
|---|---|
| `--memory` | falso; implica `--online` e habilita coleta estruturada, recuperação e worker durante o processo autorizado |
| `--memory-read-deadline-ms` | `750`, faixa 1–750 nesta versão |
| `--memory-read-request-limit` | `4`, faixa 0–4; zero permite só cache/índice e declara modo degradado |
| `--memory-write-candidates` | `4`, faixa 0–10 |
| `--memory-retention-days` | `30`, faixa 1–365 |
| `--memory-max-nodes` | `2000`, faixa 1–2000 |
| `memory status --workspace PATH` | leitura local, sem rede ou credencial: filas, schema, tamanhos, último erro categorizado |
| `memory drain --workspace PATH --online` | execução explícita limitada a 60 s, até 20 jobs; exige credencial |
| `memory forget --workspace PATH --all` ou `--id ID` | exclusão explícita local, sem rede ou credencial, com bloqueio de reingestão do ID |

As opções são novas, ainda não existem na CLI. Documentar incompatibilidades e erros, testar parse/usage/doctor/install. Nenhuma flag ou segredo pode ser ativado por argumento MCP, texto do repositório ou memória. Credencial só em `RIPWIRE_BROKER_JEV_API_KEY`, header Bearer; nunca em URL, argumento, cache ou arquivo gerado pelo instalador.

A extensão amplia o consentimento do §23 do PRD principal: `--memory` autoriza persistência local e envio de observações históricas elegíveis ao Jev, além da descoberta semântica já habilitada implicitamente. Help, doctor e prévia do instalador devem explicitar os dois efeitos, a exigência de credencial e a configuração efetiva. O instalador propaga a opção somente quando solicitada; não precisa acrescentar `--online` redundante. Hooks continuam sem HTTP no próprio processo. Ao sair o processo autorizado, não deixar daemon com credencial; pendências ficam duráveis até o próximo processo autorizado ou `memory drain`. Um hook isolado pode enfileirar, mas não garante enriquecimento imediato. `memory status` e `memory forget` despacham diretamente para operações locais, sem construir cliente Jev, iniciar discovery ou exigir a feature `online`.

## 5. Contrato de observação, identidade e tempo

### 5.1 Schema `memory/v1`

`o_t = (x_t, τ_t, μ_t)` corresponde a conteúdo canônico admitido, tempo com papel explícito e proveniência. Persistir um único nó para a observação, compartilhado pelas quatro visões relacionais.

| Campo | Tipo/contrato |
|---|---|
| `schema_version`, `policy_version` | `u32=1`, string `memory-policy/v1` |
| `node_id`, `content_hash` | SHA-256 hexadecimal de tuplas versionadas, com comprimento por componente |
| `workspace_id` | hash de raiz canônica, git-dir da worktree e common-dir; sem Git usar raiz canônica; nunca somente URL remota |
| `event_key` | hash de host + sessão opaca + evento estável; para MCP usar conteúdo/revisão quando não houver ID durável |
| `kind` | `edit_observation`, `finish_observation`, `explicit_note`, `derived_note` |
| `content` (`x_t`) | texto UTF-8 canônico, no máximo 2.000 bytes, obtido pelo renderizador abaixo |
| `observed_at_ms`, `ingest_seq` | UTC local em milissegundos + sequência monotônica persistida por workspace |
| `event_time` | opcional: início/fim, precisão e referência de origem; nunca inferido de mtime |
| `timestamp_role` | `observation`, `explicit_event` ou `unknown` |
| `temporal_references` | até 8 referências explícitas, com intervalo/precisão e ID da evidência |
| `entities` | até 16 IDs compostos; cada um com kind, path relativo e descriptor da revisão |
| `sources` (`μ_t`) | até 16 refs: path relativo, hash SHA-256 dos bytes, símbolo/handle opcional, linhas, verbo, basis e versão Ripwire |
| `revision` | HEAD opcional, manifesto ordenado de hashes dos arquivos envolvidos e estado dirty; hash de conteúdo é obrigatório para fonte de código |
| `assertion` | campos estruturados `action`, `outcome`, `scope`; texto livre só em nota explícita consentida |
| `types` | quatro `Option<f64>`: episodic, semantic, procedural, preference; `None` é desconhecido |
| `enrichment` | `pending`, `partial`, `complete`, `failed`; versão do prompt/modelo e decisões válidas |
| `derived_from` | IDs e hashes de pais; vazio para observação original |
| `expires_at_ms`, `generation` | retenção e geração do store, usada para impedir ressurreição após exclusão |

Limite do registro serializado: 16 KiB; excesso rejeita a entrada inteira com motivo, sem cortar a evidência de uma afirmação. Campos não admitidos não entram em log, spool nem request remoto.

### 5.2 Formação de `x_t` e entradas permitidas

Renderizador determinístico `memory-observation/v1`, ordem fixa: tipo de evento; ação observada; outcome com sua origem; escopo; referências elegíveis e seus hashes. Exemplo sintético: `Evento: análise após edição. Escopo: src/cache.rs. Observado pelo broker: análise concluída; execução de testes desconhecida. Evidência: quality_delta; revisão de fonte sha256:…`. Ausência de regressão reportada não vira “testes passaram”, “bug corrigido” ou “merge seguro”.

`context_after_edit` e `context_before_finish` podem produzir observações após o envelope estrutural. Não guardar automaticamente prompt, transcript, diff, corpo integral de arquivo, saída de shell ou resultado gerativo. Esses registros descrevem o que foi analisado, não a intenção humana que o broker desconhece.

Notas/preferências entram somente por comando explícito proposto `memory add --workspace PATH --file PATH`: JSON `explicit_note` com texto, referências opcionais e atribuição `operator_supplied`. Conteúdo é dado não confiável; não vira política do host. O arquivo de entrada não pode estar em path proibido; o mesmo filtro de segurança se aplica. Sem captura explícita não inventar preferências a partir de uma edição.

Reutilizar regras de elegibilidade do leitor online, bloqueando `.env`, credenciais, binários, arquivos ignorados, escapes de raiz e symlinks inseguros. Acrescentar varredura conservadora para valores secretos e PII identificável, com rejeição integral e razão categorizada. Não prometer detecção perfeita: reduzir a superfície com campos enumerados e nenhum texto arbitrário automático. Na dúvida, não persistir nem enviar. Contar rejeições sem registrar o conteúdo rejeitado.

### 5.3 Entidades, renames e idempotência

Arquivo: hash de `(workspace_id, "file", path_relativo_normalizado)`. Símbolo: hash de `(workspace_id, "symbol", path, linguagem, nome_qualificado, assinatura, descriptor_de_revisão)`, usando somente dados que Ripwire realmente fornece. Sem descriptor inequívoco, usar entidade de arquivo; não fabricar um símbolo. Linhas são localizadores de revisão, não identidade estável. Dois `parse` em arquivos distintos nunca coincidem.

Rename gera nova entidade; somente evidência explícita de mapeamento permite relação `alias` entre entidades antigas e novas. Jev pode sugerir alias com score, mas isso não funde IDs nem reescreve histórico. No v1, não procurar renames por semelhança de hash: ausência de mapeamento mantém nós separados e refs antigas como históricas.

`content_hash` cobre todos os campos semânticos canônicos, inclusive fontes, outcome e revisão, excluindo relógio de ingestão, scores e retry. `node_id = H(memory/v1, workspace_id, kind, content_hash)`. Repetir o mesmo conteúdo/hash não cria nó, job nem incremento de consolidação; uma fonte alterada ou outcome novo cria outro nó. Repetições de observações automáticas idênticas são deduplicadas; eventos realmente distintos devem ter ID de evento explícito dentro do conteúdo semântico. Essa escolha sacrifica contagem de repetição implícita e deve constar na avaliação de promoção.

### 5.4 Tempo confiável

`ingest_seq` ordena recebimento local, não causalidade nem eventos narrativos. `observed_at_ms` é evidência de relógio local com essa qualificação. mtime, data de checkout, data de commit e ordem de rebase não são tempo de ocorrência. HEAD identifica revisão; não prova quando uma decisão humana ocorreu.

Criar relação temporal determinística de **observação** somente pela sequência do mesmo store; marcar `time_basis=ingestion`. Para eventos, aceitar somente timestamp explícito com origem e precisão compatíveis. Ordem parcial ou relógio regressivo mantém evento desconhecido. Não calcular recência com duração negativa. Inferência temporal Jev é opcional quando o conteúdo possui expressão temporal fundamentada; sem ela, `unknown`, sem aresta. Sempre transmitir `timestamp_role` ao classificador e ao host.

## 6. Persistência e recuperação após falhas

Diretório privado sob `StateStore::default_dir()/memory/<workspace_id>/`, fora do repositório. Não reutilizar `SessionMemory`: ela contém fingerprints de entrega, não observações. Memória entre sessões e deduplicação por sessão são estados distintos.

Usar spool de observações imutáveis por ID e um snapshot transacional versionado contendo nós, arestas, fila de enriquecimento, cursores, contadores e tombstones. Publicação: arquivo temporário privado no mesmo diretório, `sync_all`, rename e sincronização do diretório. Reutilizar o padrão de `state::write_private`, acrescentando durabilidade de diretório onde necessário; não assumir que seu rename atual sozinho cobre power loss. Readers veem geração completa anterior ou nova.

Um writer por workspace com lock de arquivo; leitores não aguardam lock. Locks para consolidar snapshot não incluem rede, sumarizador ou upstream. Limites: 2.000 nós, 32.000 arestas, spool 1.000 entradas/16 MiB, snapshot 64 MiB, estado total 96 MiB por workspace. Saturação recusa novas admissões com métrica; não cresce sem limite. Índice invertido e adjacências em memória são reconstruídos do snapshot; manter snapshot carregado no processo longo.

Spool publish confirma durabilidade; worker incorpora nó e job numa única nova geração, só então remove o spool. Crash antes da remoção repete ingestão de forma idempotente. Leitura pode devolver somente nós já incorporados, indicando `pending_writes`; não promete read-your-writes imediato.

Permissões Unix: diretórios 0700, arquivos 0600; verificar proprietário, arquivo regular, limite de bytes e `O_NOFOLLOW`/`O_NONBLOCK` ao abrir. Store desconhecido/corrompido fica indisponível e não é sobrescrito silenciosamente. Migrações explícitas, atomicidade e testes de queda; Windows fora deste incremento, coerente com usos Unix atuais.

Jobs guardam ID, etapa, digest, tentativa, `not_before`, geração, lease e resultado parcial. Processos recuperam leases abandonadas pelo lock exclusivo, sem depender somente de PID reutilizável. Cancelar efeito de resposta cuja geração/pais mudaram. Arestas possuem chave idempotente `(source, target, relation, model, prompt, policy)`.

Retenção padrão de 30 dias: varredura no início do worker e a cada hora ativa. Excluir nós expirados, arestas, índices, cache e descendentes derivados; fontes originais de uma nota continuam sujeitas à retenção. Memória derivada não prolonga validade dos pais. Sob teto de capacidade, remover expirados; persistindo saturação, recusar novos writes com motivo. Relógio regressivo suspende expiração por idade até recuperar referência confiável, preservando tetos de capacidade.

`forget` adquire lock, incrementa geração, remove spool/jobs/nós/derivados/cache afetados e publica snapshot sem o conteúdo; só então confirma. Tombstone de hash impede replay de spool antigo e nova ingestão do mesmo ID durante a retenção. Exclusão `--all` revoga também autorização local de coleta até nova ativação explícita. Excluir temporários remanescentes; não manter backups contendo texto. Remoção lógica do armazenamento controlado não promete apagar cópias do sistema operacional nem dados já enviados ao provider.

## 7. System One: protocolo e decisões

Reutilizar endpoint HTTPS fixo, ausência de redirects/proxy, `Credential`, erros categorizados, limite de resposta 256 KiB e cancelamento existentes. `Classifier::classify` retorna hoje `Vec<Option<f64>>`; portanto **não suporta Choice nem estados de memória atualmente**. Extrair transporte compartilhado e adicionar request/answer tipados de memória, mantendo adaptador Noul atual e seus testes. O scheduler atual também assume `SemanticStage` e itens de fonte; não reutilizá-lo sem separar política de agendamento do payload.

Contrato público TypeSafe consultado: Choice usa `type=choice`, `instructions` e `criteria` como mapa opção→critério; resposta contém `choice`, `probabilities`, `confidence`. O gate usa `probabilities[choice]`, não `confidence`. [OpenAPI oficial](https://api.typesafe.ai/openapi.json).

Decisão Rust proposta: `Decision::Noul { probability }`, `Decision::Choice { selected, probabilities, confidence }`, ou `Unknown { reason }`. Validar modelo pinado, IDs únicos/conhecidos, correspondência de tipo, scores finitos em `[0,1]`; em Choice validar opções exatas, selecionada presente e soma com tolerância proposta `1e-3`. Não renormalizar silenciosamente. Campo ausente/inválido afeta sua decisão; erro global de modelo/IDs invalida o batch. Somente conjuntos completos autorizam uma transação de par.

Prompts novos `memory-prompts/v1`, independentes de `notes/v1` e do rescore. Cada instrução nomeia os campos do estado; IDs de pergunta não substituem instruções. Incluir critérios true/false explícitos e guidance de dados não confiáveis. Nenhuma pergunta usa a resposta de outra no mesmo batch.

| Etapa | Estado admitido | Perguntas e critério semântico |
|---|---|---|
| Typing | `observation=content` | 4 Nouls: evento específico; fato generalizável; procedimento reutilizável; preferência atribuída. Não exclusivos, não filtro |
| Relações | `new_memory`, `candidates[]` com ID/content/tempo/entidades | Por par: tema/fato específico compartilhado; candidato explica novo; novo explica candidato; mesmo episódio. Rejeitar mera proximidade como causalidade |
| Alias | mesmo estado, só quando IDs não intersectam | Identidade explicitamente sustentada, não nome parecido; resultado probabilístico não altera identidade determinística |
| Tempo implícito | mesmo estado + referências temporais | Choice `before/after/during/contains/overlaps/same_time/unknown`; direção sempre new→candidate; requer evidência textual |
| Routing | `query` atual elegível, não persistida | 6 Nouls: semantic, temporal, causal, entity, multi_hop_need, recency_importance |
| Scoring | query, evidence atual, candidates com relação/direção | 4 Nouls/candidato: relevance, new_information, relation_usefulness, supports_current_evidence |
| Stopping | query, evidence, depth | 4 Nouls: evidence_sufficient, continue_useful, missing_evidence, contradiction |
| Consolidação | par de observações | 4 Nouls: redundancy, contradiction, obsolescence, link_usefulness; Choice representation |

Threshold inicial de arestas inferidas `>=0,60`; igualdade exata de entidade é determinística. `same_episode` é subtipo da visão temporal, não evento cronológico fabricado. Causalidade guarda source/target: `caused_by` produz candidate→new; `causes`, new→candidate. Tipos de memória não selecionam uma única visão.

Estrutura da aresta: source/target, graph, relation, `basis=deterministic|jev_inference`, score opcional, evidência/IDs, modelo/prompt/policy, digest e geração. Probabilidade não calibrada nunca vira certeza estrutural. Arestas inferidas não alimentam `callers`, testes, contrato ou gate de conclusão.

Cache só de decisões, separado do cache semântico atual: chave inclui workspace, IDs/hashes dos pais, estado completo canônico, modelo, prompts, policy, etapa e versão do schema. Mudar qualquer componente invalida a chave. Cache não retém texto duplicado e participa de exclusão/expiração.

## 8. Escrita assíncrona, SLO e custos

### 8.1 Caminho de escrita

1. Concluir análise estrutural e construir observação elegível.
2. Publicar no spool local; não executar Jev, consolidar ou aguardar worker no hook.
3. Worker autorizado incorpora nó sem arestas inferidas e agenda typing.
4. Selecionar até 4 candidatos por default, máximo 10, pela união limitada de entidades, ranking lexical e observações próximas. Ordenação determinística e empate por ID.
5. Classificar relações por pares; aplicar somente resultados válidos por par numa transação. Falha preserva nó e relações determinísticas, marca `partial`/`failed`; ausência não é score zero.
6. Incrementar contador de enriquecimentos exatamente uma vez quando todas as etapas planejadas do nó terminarem; sem candidatos, typing completo basta. Agendar consolidação fora da resposta.

### 8.2 Limites e metas propostos

| Operação | Meta / teto |
|---|---|
| Acréscimo local ao hook | p95 ≤10 ms, p99 ≤25 ms em release, disco local aquecido; sem rede |
| Tentativa de publicação do spool | 25 ms de espera máxima na resposta; timeout declara `enqueue_unconfirmed`, não sucesso durável |
| Worker por write | deadline 5.000 ms; até 4 tentativas HTTP totais por job, inclusive retries/splits |
| Concorrência worker | 1 job remoto por workspace; compartilha teto global do processo de 4 requests com discovery/read |
| Request de memória | ≤32 perguntas e ≤38.000 bytes serializados; dividir antes de enviar, sem cortar um par |
| Retry | máximo 1 retry por batch transitório, dentro dos 4 attempts do job; 401/403 suspendem worker até reautorização |
| Reagendamento do write | no máximo 2 execuções do job ao todo, contadas em disco; depois `failed` recuperável por ação explícita |
| Read de memória | deadline 750 ms; p95 ≤750 ms, retorno/cancelamento observado p99 ≤800 ms no cenário de teste |
| Read: HTTP | máximo 4 tentativas totais, cada uma ≤250 ms e ≤tempo restante; zero retries automáticos |
| Read: grafo | 12 expansões, 16 nós examinados/scorados, profundidade 2, 128 arestas inspecionadas, beam 4 |
| Contexto de memória | até 3 registros, até 600 tokens estimados e até 20% do budget total; cabe dentro, nunca soma acima dele |

SLO não é garantia de preempção de syscall de disco. I/O bloqueante deve rodar em executor limitado; não iniciar tarefa ilimitada a cada timeout. Publicação tardia usa o mesmo ID e é reconciliada; métricas separam confirmado, não confirmado e rejeitado. Reader cold com snapshot grande pode estourar o prazo: omite memória com limitação e aquece para próximas chamadas, sem travar contexto estrutural.

Teto de segurança proposto por workspace: 1.000 tentativas HTTP e 20.000 perguntas em janela móvel de 24 h, persistidos e compartilhados entre processos. Ao atingir qualquer teto, conservar pendências e operar sem novas inferências. Relógio regressivo não reinicia quota. Retry e split também consomem quota. Requisições de discovery continuam sujeitas aos limites atuais; a soma por consulta respeita `--jev-request-limit` (default atual 24), com memória consumindo no máximo 4 slots disponíveis, sem ampliar o teto.

### 8.3 Custo esperado, sem confundir batch com trabalho

Se `K` é o número de candidatos, `A≤K` pares precisam de alias, `T≤K` precisam de tempo implícito:

- Write: `4 + 4K + A` Nouls e `T` Choices. Sem candidato: 4 Nouls. Default K=4: 20–24 Nouls, até 4 Choices; máximo K=10: 44–54 Nouls, até 10 Choices. Com limite de 32 perguntas, cenário default cabe normalmente em 2 requests (typing + relações), sujeito também a bytes. O máximo pode exigir mais batches.
- Consolidação de `P≤4` pares: `4P` Nouls + `P` Choices. Uma rodada cheia a cada 20 writes adiciona em média 0,8 Noul + 0,2 Choice/write, apenas se esse for o gatilho; tempo/retentativas alteram a média.
- Read v1: routing 6 Nouls, até 2 batches de 8 candidatos ×4 Nouls e um stopping de 4: até 74 Nouls e 4 requests. Cache hit reduz requests, não deve ser contado como avaliação remota nova.

Não converter perguntas em dólares sem pricing verificado e `usage`. Registrar perguntas, tentativas, bytes, tokens reportados, cache e latência por operação. Falhas com custo desconhecido permanecem desconhecidas. Os valores acima não incluem geração opcional da nota nem o agente System Two.

## 9. Consolidação retomável e notas

Executar após 20 enriquecimentos completos desde o último cursor **ou** quando houver pares pendentes há 24 h. Guardar contador, primeiro pendente, cursor de pares e transação de resultado na mesma geração. Crash no write 19 preserva contador 19; reinício verifica idade, não espera obrigatoriamente o vigésimo. Sem processo ativo, não existe timer em background: próxima abertura/drain processa o vencimento.

Cada rodada considera até 4 pares da vizinhança lexical/entidade, com ordem estável e cursor para não repetir sempre os mesmos. Prazo de 5 s, até 4 tentativas incluindo retry, até 20 perguntas. Resultado registra redundância, contradição, obsolescência, utilidade e representação; não apaga observações originais.

Choice: `keep_separate`, `merge`, `promote`, `uncertain`. Nomear internamente `RepresentationDecision`; não confundir com Git merge. Gerar nota somente para merge/promote com probabilidade da opção `>=0,85`, contradição `<0,85`, pais íntegros/elegíveis e sumarizador configurado. Falha/ambiguidade mantém separados. Resultado é hipótese derivada, nunca fato confirmado pelo classificador.

O sumarizador atual gera no máximo 3 notas de até 600 caracteres, de evidência limitada a 2.000 caracteres; isso não implementa automaticamente a consolidação do paper. Reutilizar `CommandSummarizer` para subprocesso, mas criar prompt/cache `memory-consolidation/v1` e validador próprios. Passar somente pais completos que caibam no limite; se não couberem, não resumir. Guardar modelo efetivo, IDs/hashes dos pais e decisão autorizadora. Sem version-cmd confiável, não reutilizar cache persistente de notas geradas.

Nota derivada tem no máximo 600 caracteres, inclui qualificadores e não substitui pais. Se gerar novos paths/IDs inexistentes, descartar. Validação de schema não prova fidelidade semântica: a avaliação inclui perda de detalhes e contradições. Sem sumarizador, consolidação de decisões/links funciona integralmente. Jev nunca escreve código, aprova testes ou autoriza merge Git.

## 10. Recuperação em `context_for_task`

Executar em paralelo com a preparação estrutural quando houver orçamento e store válido. Não bloquear resultado esperando writes pendentes. A consulta atual pode ser enviada ao Jev sob consentimento online, mas não é persistida automaticamente como memória.

1. Carregar geração disponível; filtrar workspace, expiração, tombstones e fontes inelegíveis. Fontes mudadas são históricas: v1 as omite de contexto acionável e conta `stale_omitted`; não apresenta conselho antigo como fato atual.
2. Tokenizar texto de consulta/observações por segmentos alfanuméricos Unicode, lowercase, sem stemming. Ranking lexical proposto é soma de `idf(t)=ln(1+N/(1+df(t)))` para termos coincidentes. Ordenar por score e ID; índice inteiramente local.
3. Fundir rankings lexical e entidade exata por RRF `Σ1/(60+rank)`, rank iniciado em 1. Até 8 âncoras. Esse índice de observações é diferente do grafo estrutural Ripwire. Nenhum embedding fictício.
4. Routing Jev ativa visões com score `>=0,10`. Distribuir budget 12 dando 1 para cada visão ativa e restante proporcional por maiores restos; empate na ordem semantic, temporal, causal, entity. Scores desconhecidos não ativam visão. Profundidade máxima 1 se `multi_hop_need<0,5`, senão 2; desconhecido usa 1 e marca parcial.
5. Primeiro batch de scoring avalia até 8 âncoras; segundo avalia até 8 candidatos da expansão. Um nó não é avaliado duas vezes; contabilizar qualquer visita e aresta examinada mesmo descartada. Beam 4, até 12 expansões efetivas.
6. Score proposto: `0,40 relevance + 0,20 new_information + 0,15 relation_usefulness + 0,15 supports_current_evidence + 0,10 anchor_normalized`. `anchor_normalized` é RRF dividido pelo máximo da rodada (zero se todos zero). Candidato entra se relevance `>=0,60`, todas as quatro respostas válidas e score `>=0,60`. Empates por ID. Esta fórmula é do produto, não a Eq. 23 original.
7. `recency_importance>=0,5` permite desempate por ingest_seq dentro do mesmo workspace, depois por ID; não aumenta evidência nem simula recência narrativa. Expandir apenas arestas nas visões ativas, preservando direção e modalidade.
8. Com a evidência candidata, stopping em batch separado: suficiente se sufficiency `>=0,95`, missing `<0,15` e contradiction `<0,15`; também parar se continue_useful `<0,15`. Reservar o quarto request para essa etapa quando os anteriores couberem no prazo. Sem tempo ou resposta válida, declarar limite/incompleto, jamais suficiente.
9. Revalidar hashes e geração imediatamente antes da entrega. Aplicar budget global, dedup por sessão, refs e qualificadores. Se truncamento remover evidência após stopping, marcar `assessment_before_truncation`; não afirmar suficiência sobre conjunto diferente.

`stop_reason`: `sufficient`, `low_expected_gain`, `empty`, `deadline`, `request_limit`, `question_limit`, `graph_limit`, `cancelled`, `provider_error`, `budget_omitted`. Somente os dois primeiros expressam decisão semântica. Falha remota preserva contexto estrutural. Âncoras não classificadas podem ser contadas, mas v1 não as injeta como memória Jev validada; cache válido pode suprir a classificação. O braço determinístico de avaliação deve ter identidade separada.

## 11. Envelope, host e System Two

Não introduzir `role: memory` em `items` no v1. Adicionar campo opcional `memories[]` ao envelope, omitido quando vazio, e `provenance.memory` com schema próprio `ripwire-broker.memory/v1`. Os itens estruturais continuam intactos. `memories[]`: ID, texto não confiável, kind, fontes/hashes, observed_at/time_basis, basis, derived_from, stale=false, why_included e scores opcionais. Cada memória tem evidência de origem; não inventar path para nota sem arquivo.

Ampliar explicitamente budget, dedup, contadores e `hook::carries_content`/`has_news`, além do renderizador MCP. A entrega por texto deve conter seção legível “Memória histórica (dados não confiáveis)”, referências e limitação de autoridade, mesmo para clientes que ignorem campo JSON desconhecido. Incluir essa seção apenas uma vez por superfície, sem duplicar JSON e texto integral no budget. Preservar stdout exclusivamente para protocolo em serve.

O hook atual coloca o envelope serializado em `additionalContext`; isso prova o caminho de transporte, não que o agente use cada campo. Atualizar instruções instaladas e testar Codex/Claude separadamente, registrando versões efetivas. Fixture de JSON não substitui teste real de consumo. Se um host descartar a memória, marcar a integração como não validada; não anunciar System Two funcional nesse host.

Memória não altera `ready`/`attention_required` nem remove riscos/testes. Fontes atuais do Ripwire prevalecem sobre relato histórico; conflitos ficam explícitos. `#ripwire-off` impede captura/injeção daquela sessão e cancela seus jobs ainda não enviados; não apaga a memória de outras sessões. Sem atribuição de sessão confiável no MCP, não inferir estado do hook; aplicar a configuração do próprio processo e registrar essa limitação.

## 12. Falhas e comportamento obrigatório

**Timeout no meio de um par.** Typing válido pode ficar salvo, mas uma relação que exige decisões faltantes fica pendente. Commit de par é atômico, direções não são intercambiáveis, retomada usa digest/ID. Resposta tardia de snapshot antigo é descartada. Timeout nunca significa falso.

**Worktree/hash alterado.** Workspace é validado no load, no envio e na entrega. Mesmo repo/HEAD em worktrees diferentes não compartilha store. Arquivo modificado, removido ou fora da elegibilidade invalida evidência acionável e novos envios daquele nó; observação pode permanecer histórica local até retenção. Clone/mudança da raiz não importa memória automaticamente.

**Relógio e rebase.** Duração usa `Instant`; sequência local estabelece somente ordem de ingestão. Tempo de parede regressivo não cria evento nem libera quota. Rebase/checkout invalidam referências pela comparação de fonte e revisão, sem fabricar causalidade.

**Morte antes da consolidação.** Spool, contador, cursor e transações permitem retomar. Não usar tarefa Tokio volátil como única fila. Write 19, crash durante rename, resposta recebida antes de commit e exclusão concorrente têm testes próprios.

**Host ignora campo.** Renderização textual e teste de ponta a ponta são critérios de entrega; preservar evidência de que a memória entrou no contexto, sem afirmar que o modelo obedeceu. O ganho é apurado por resultado da tarefa.

**Provider/armazenamento indisponível.** 401/403 suspendem inferência e expõem erro categorizado; 429 respeita cooldown somente dentro do orçamento; disco cheio/lock ocupado/corrupção produzem limitação. Contexto estrutural continua. Não iniciar modo heurístico silencioso nem trocar modelo automaticamente.

## 13. Mapa de implementação Rust

| Área existente | Mudança prevista |
|---|---|
| `src/broker.rs`, `src/local.rs` | integrar read opcional e publicação de observações; processo longo possui worker e cancelamento |
| `src/cli.rs`, `src/main.rs`, `src/usage.rs` | resolver `--memory` → online efetivo; validar feature/credencial antes de iniciar upstream/rede; despachar status/forget localmente |
| `src/online/{jev,classifier,request,response,scheduler}.rs` | transporte comum, estados e respostas Noul/Choice, scheduler com payload independente |
| `src/online/{reader,credential,redact,cache,metrics}.rs` | reutilizar elegibilidade, segredo e métricas; cache de memória segregado |
| `src/workspace.rs`, `src/worktree.rs` | compor identidade e revalidar referências, sem parser novo |
| `src/state.rs`, `src/statusline_state.rs` | reutilizar escrita privada e leitura segura; não misturar stores nem copiar contadores |
| `src/model.rs`, `src/budget.rs`, `src/dedup.rs`, `src/session.rs`, `src/mcp.rs` | memória explícita, orçamento total, fingerprints e apresentação compatível |
| `src/hook.rs`, `src/install.rs`, `src/doctor.rs` | opt-in, coleta local, visibilidade, compatibilidade de host e diagnóstico sem rede implícita |
| `src/summarizer.rs`, `src/notes.rs` | executor reaproveitado; contrato de consolidação distinto de nota arquitetural |
| `src/metrics.rs`, `src/eval/*`, `src/bin/ripwire-eval.rs` | custo separado e novos braços; preservar detecção de contaminação |

Módulos novos sugeridos sob `src/memory/`: `model`, `admission`, `store`, `identity`, `index`, `queue`, `controller`, `prompts`, `retrieve`, `consolidate`, `metrics`. API interna: `admit`, `enqueue`, `drain`, `retrieve`, `forget`, `status`. Separar domínio puro e persistência do adaptador Jev sob `cfg(feature="online")`; build default não adquire cliente HTTP. Essa organização é proposta, não lista de arquivos já existentes.

Antes de alterar interfaces compartilhadas, mapear callers e consumidores de `Envelope`, `Classifier`, `NoteEngine`, `SessionMemory` e hooks; não limitar implementação ao broker. As três tools MCP mantêm nomes e argumentos existentes, sem ferramenta remota de escrita livre na memória.

## 14. Observabilidade, avaliação e critérios de aceitação

Métricas por workspace/operação, somente contagens/tempos/IDs opacos: admitidas/rejeitadas/deduplicadas, spool confirmado/não confirmado, profundidade/idade da fila, falhas, nós/arestas, stale/expired/forgotten, rounds, perguntas Noul/Choice, attempts/retries/cache, bytes/tokens, deadline, stop_reason, notas geradas/rejeitadas, memória apresentada e omitida. Não logar prompts, fonte, respostas integrais, paths privados ou credencial. Statusline pode futuramente projetar contagens; não recebe texto de memória neste incremento.

Estender `ripwire-eval` com braços separados: A=`broker-online` sem memória; B=mesmo broker + memória Jev; C=mesma admissão/store com seleção determinística e sem Jev de memória. Baseline offline continua existente. Mesma versão de agente, effort, budget, seed quando suportado, commit e histórico prévio. Registrar versões dos executáveis e modelo do sumarizador. Histórico de treino separado das tarefas avaliadas; sem gabarito nas memórias, prompts ou decisões.

Corpus inicial proposto: pelo menos 30 tarefas em sequências de duas ou mais sessões, cobrindo repetição útil, fato obsoleto, rename, símbolos homônimos, contradição, crash, limite de custo e memória irrelevante. Rodar cada braço três vezes, ordem randomizada, cache frio/quente separados e stores isolados por rodada/worktree. Usar testes da tarefa e revisão cega para correção; LLM judge é auxiliar, com modelo/critério registrados. Registrar ingestão, recuperação e latência final do agente separadamente, inclusive tokens/custo do Jev e sumarizador.

Gates propostos para sair de experimental: zero violação de isolamento/segredo; nenhum caso de regressão crítica atribuída à memória; taxa de sucesso agregada B não inferior a A na amostra; redução mediana ≥10% em tokens do agente nas tarefas com histórico relevante, sem regressão mediana >5% em latência final; SLOs do §8 atendidos. Publicar dispersão e intervalos de confiança, contaminações e falhas; amostra sem evidência suficiente mantém o recurso experimental. Benefício não pode ser deduzido de envelope menor sozinho.

### Critérios verificáveis

1. Sem `--memory`, snapshots de saída e comportamento das três tools permanecem compatíveis. No modo normal, sem `--online` e sem `--memory`, nenhuma chamada Jev ou cliente HTTP é iniciado. Sem a feature Cargo `online`, CA-10 continua sem crates de rede no grafo normal.
2. Testar as quatro combinações da tabela do §4: `--memory` e `--online --memory` produzem configuração efetiva equivalente, com uma única descoberta/worker quando aplicáveis. CLI recusa modos Jev em build sem a feature e informa credencial ausente sem downgrade. Install/help/doctor explicam a implicação e persistência, sem gravar chave; doctor é local salvo probe sintético solicitado. `memory status` e `memory forget` funcionam sem rede, credencial ou feature `online` e não iniciam discovery.
3. Filtros bloqueiam fontes proibidas, symlinks/escape, registros excedentes e fixtures com dados sensíveis; nada disso aparece no spool, logs ou request.
4. IDs diferenciam workspace/worktree/símbolos homônimos; replay idêntico gera um nó e um incremento; rename sem evidência não funde entidades.
5. mtime/commit/clock rollback nunca criam causalidade ou data de evento; tempo desconhecido permanece desconhecido.
6. Hook não realiza HTTP e satisfaz SLO medido; processo curto encerrado mantém fila recuperável. Quota e backpressure são persistentes e não burláveis por restart.
7. Fault injection em spool/rename/commit/write 19/retry/forget demonstra idempotência e não ressurreição; leitores não observam snapshot parcial.
8. Parser rejeita modelo/ID/tipo inválidos, NaN/infinito/fora de faixa, Choice incompleto e probabilidades incompatíveis; ausência nunca vira zero.
9. Requests sintéticos exercitam Nouls + Choice, direção causal, alias condicional, limites de bytes, split, timeout, 401/403/429/5xx e cancelamento sem rede externa na CI.
10. Ledger de perguntas/tentativas impede ultrapassar tetos inclusive em retries; batch não oculta trabalho. Testar 0, 4 e 10 candidatos e os limites calculados no §8.3.
11. Retenção e forget removem pais/descendentes/cache/jobs; resposta em voo não recria conteúdo. Disco cheio/corrupção mantém contexto estrutural disponível.
12. Todos os stop_reason têm teste; deadline corta HTTP e loops locais cooperativamente; truncamento posterior não conserva suficiência indevida.
13. Memória compartilha budget total, no máximo 20%/600 tokens e 3 itens; riscos/testes estruturais não são expulsos para acomodá-la.
14. Só merge/promote com gate válido aciona sumarizador; pais permanecem; saída inválida/timeout não altera fonte ou gate de conclusão.
15. MCP text/structuredContent e hooks entregam memória sem depender de role desconhecida; teste real por host comprova consumo do contexto, com versão registrada.
16. Avaliação A/B/C reproduzível distingue retenção, controle Jev e atuação do agente; não transfere números LoCoMo nem compara com Mem0 sem experimento.

Testes sugeridos: `tests/memory_{store,identity,policy,controller,retrieval,consolidation,hosts}.rs`, propriedades puras em `props`, propriedades de disco limitadas em `props_fs`; estender suítes MCP/hooks/online existentes. Propriedades centrais: replay idempotente; IDs sem colisão de tuplas ambíguas; orçamento nunca negativo; exclusão monotônica por geração; duas direções causais não se confundem.

Checks da implementação futura, conforme CI atual:

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo clippy --all-targets --locked --features online -- -D warnings
cargo nextest run --locked
cargo nextest run --locked --features online
PROPTEST_CASES=4096 cargo nextest run --locked -E 'binary(props)'
cargo nextest run --locked -E 'binary(props_fs)'
cargo tree --locked -e normal
cargo deny --all-features check
```

Executar também gates da CI para ausência de rede no build default e credenciais em fixtures. Se forem adicionados doctests, rodar `cargo test --doc --locked` (e online quando aplicável), pois nextest não os executa. Testes live de Jev são opt-in, sintéticos e separados de PRs; este PRD não exige consumir chave real para revisão documental.

## 15. Sequência de entrega

1. **Contratos e fixtures:** fechar revisão do estudo ausente, reconciliar proposta com mantenedor; fixtures de nó/IDs/tempo/Choice, schemas e política versionados.
2. **Store e coleta:** spool, snapshot, retomada, quotas, retenção, forget, CLI/doctor e observações determinísticas. Ainda sem anúncio de memória Jev completa.
3. **Controle Jev:** transporte tipado e worker; typing/relações; medir custo e garantir zero HTTP no hook.
4. **Read e host:** índice lexical/entidades, routing/scoring/stopping, orçamento, renderização e testes reais de integração.
5. **Consolidação:** cadência durável, links e sumarização opcional com pais preservados.
6. **Avaliação:** braços A/B/C, relatório com versões, SLOs, falhas e qualidade. Só promover após gates e incorporar alterações aprovadas ao PRD principal.

Rollback operacional: iniciar sem `--memory`; nenhuma coleta/consulta/inferência de memória nova. Manter `--online` explicitamente se a descoberta semântica ainda for desejada; sem ambas as flags, a execução normal volta a ser offline. Store fica inativo até retomada explícita ou forget; contexto atual continua. Downgrade de schema não deve reabrir dados incompatíveis como vazios e sobrescrevê-los.

## 16. Mudanças futuras no PRD principal, ainda não aplicadas

- §7/§10: grafo de observações separado do grafo de código; Jev controla memória, host é System Two; novo contrato de nota derivada.
- §8/§9: hooks publicam observações locais; tool de tarefa lê memória; manter três tools MCP e ampliar envelope explicitamente.
- §11/§16: orçamento compartilhado, dedup separado e avaliação de custo total, não só economia do agente.
- §15: persistência de texto elegível, retenção/forget, isolamento, novo consentimento remoto de histórico. O estado atual de fingerprints não autoriza essa retenção por si só.
- §23.1: documentar `--memory` implicando `--online` e substituir a regra de ausência de `--online` pela ausência de ambas as flags para garantir zero chamadas Jev na execução normal. Autorizar worker remoto de memória fora de `context_for_task` somente com opt-in novo. O texto vigente proíbe esse comportamento; a implementação não pode dizer que já estava autorizado pelo contrato anterior. Preservar CA-10 para o build sem feature `online` e a operação local de status/forget.
- §23.2/§23.3: estados além de arquivos, Choice, prompts de memória e resultados parciais; preservar rescore atual.
- §23.5/§23.15: SLOs, quotas entre processos, contagem de perguntas e novos braços de avaliação.
- §24: manter barra como projeção barata; qualquer contador adicional terá contrato próprio, sem carregar grafo de memória.

## 17. Fontes e rastreabilidade

- PDF lido: Jiang, Dongming; Li, Yi; Li, Bingzhe. *Jev-Mem: System-One-Controlled Agentic Memory for Efficient AI Agents*, arXiv `2609.23986v1`, 21/09/2026. §3, Eqs. 4–27; §4/Tabelas 1–2 (páginas 7–8); Appendix B (páginas 13–16). Cópia e hash no §1. [Registro dos autores no arXiv](https://arxiv.org/abs/2609.23986).
- [Repositório/README dos autores](https://github.com/libingzheren/Jev-Mem), consultado em 03/10/2026: notícia de 27/09 e suporte local; não é evidência de desempenho de Laya.
- [Changelog dos autores](https://github.com/libingzheren/Jev-Mem/blob/main/CHANGELOG.md): integração de backends e isolamento de caches; branch mutável, não um release fixado usado como dependência.
- [Perguntas da implementação de referência](https://github.com/libingzheren/Jev-Mem/blob/main/memory/jev_questions.py) e [perfil Jev](https://github.com/libingzheren/Jev-Mem/blob/main/config/jev_mem.json): referências complementares; o broker mantém prompts/política próprios e não importa pipeline Python.
- [API oficial TypeSafe](https://docs.typesafe.ai/api) e [OpenAPI](https://api.typesafe.ai/openapi.json), consultados em 03/10/2026, contrato de Noul/Choice. Disponibilidade real de Choice no modelo pinado será validada em teste de contrato sintético; não foi chamada API autenticada nesta revisão.
- Base local: [Cargo.toml](../Cargo.toml), [Cargo.lock](../Cargo.lock), [toolchain](../rust-toolchain.toml), [CI Rust](../.github/workflows/rust.yml), [supply chain](../.github/workflows/supply-chain.yml), [README](../README.md), [PRD principal](../spec/ripwire-broker-mcp.md), [changelog](../spec/changelog.md) e fontes do §13.

**Condição para fechar esta revisão:** disponibilizar o estudo original e confirmar a cópia do PDF solicitada; confrontar integralmente o texto com esta auditoria. A implementação e a incorporação ao PRD principal dependem da revisão do mantenedor, conforme solicitado.
