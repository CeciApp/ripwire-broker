# Proposta — o processo ripwire por evento de hook (item 4 da revisão de arquitetura)

Status: **fechado — opção B feita em [D-105](../changelog.md#d-105--a-versão-do-ripwire-deixa-de-custar-um-processo-por-evento-de-hook);
C e D recusadas pela medição com o ripwire real; E feita em
[D-106](../changelog.md#d-106--uma-rajada-de-edições-é-uma-pergunta-não-uma-por-edição) — item 4 fechado** ·
2026-09-28 · item 4 de
[D-096](../changelog.md#d-096--gargalos-de-arquitetura-medidos-e-os-dois-primeiros-corrigidos),
único em aberto depois de [D-100](../changelog.md#d-100--itens-7-e-10-medidos-e-recusados-e-os-números-do-d-096-ao-d-099-refeitos-em-release)

Fonte: PRD 8.4 (níveis de automação), 15.3 (superfície de processo e limite de memória),
RF-14, e as decisões [D-033](../changelog.md#d-033--instalação-e-diagnóstico-proposta),
[D-050](../changelog.md#d-050--limite-de-memória-do-ripwire-por-supervisor),
[D-052](../changelog.md#d-052--code-review-das-fases-2-e-3) e
[D-064](../changelog.md#d-064--cache-diagnóstico-e-integração-proposta).

Este documento **não decide nada**. Ele mede o custo, nomeia o que cada mitigação quebra, e
separa o que é barato e isolado do que é mudança de arquitetura com superfície de segurança nova.
Nada disso vira código antes de aprovação.

## 0-bis. ADENDO (2026-09-28 22:28) — o ripwire real foi medido, e derruba a tese central

Tudo abaixo desta seção foi escrito com um **dublê** de `ripwire --mcp` em Python, porque o ripwire
não estava no PATH. Ele foi instalado (`~/.local/bin`, versão **0.6.5**) e medido. O que muda:

| | dublê Python | ripwire real |
| --- | --- | --- |
| `ripwire --version` | 19,4 ms | **3,6 ms** |
| `user-prompt-submit` | 43,0 ms | **19,4 ms** |
| `post-tool-use` | 43,0 ms | **34,0 ms** |
| `stop` | 42,6 ms | **222,0 ms** |

**A tese central da proposta não sobrevive.** Ela dizia: "os três custam o mesmo, e é isso que
importa — os 43 ms são overhead fixo de subida". Com o ripwire real os três custam coisas muito
diferentes, e o `stop` custa **5x** o mais barato. O que domina é o **trabalho do ripwire**, não a
subida do processo: o gate de conclusão faz várias chamadas upstream e o ripwire trabalha de verdade
em cada uma.

O que isso faz com cada opção:

- **B — feita.** Ganho entregue de 1 a 3 ms por evento: `user-prompt-submit` 19,4 → 17,0 ms (12%),
  `post-tool-use` 34,0 → 33,0 (3%), `stop` 222,0 → 218,9 (1,4%). A proposta prometia **45% do
  total**, o que era artefato do startup do Python. Continua valendo por remover um processo
  inteiro por evento, com código pequeno e testado, mas o valor é modesto e vale dizer isso.
- **C e D — recusadas.** A justificativa das duas era "a subida de processo domina". Com o ripwire
  real a subida é ~3 a 5 ms de 17 a 219 ms. Elas comprariam pouco e custariam superfície nova de
  segurança — socket autenticado, escopo por workspace, ciclo de vida sob
  [D-050](../changelog.md#d-050--limite-de-memória-do-ripwire-por-supervisor) — além de reabrir o
  [D-064](../changelog.md#d-064--cache-diagnóstico-e-integração-proposta). **A medição que era o
  bloqueio resolveu o caso, e resolveu contra elas.**
- **E — segue aberta**, e agora é a única com ganho grande. O `stop` a 219 ms e o `post-tool-use`
  a 33 ms **por edição** são custo real de trabalho do ripwire, não de subida. Reduzir eventos ataca
  isso; nada mais nesta proposta ataca. Continua sendo decisão de produto sobre PRD 8.4, não
  otimização.

Uma rota que a proposta sugeria para B **não existe**: ler a versão do handshake MCP. O
`rust-mcp-sdk` 2.0.0 não expõe o `Implementation` do servidor ao cliente — `server_details()` é do
lado servidor. Mesmo obstáculo que o [D-049](../changelog.md#d-049--cancelamento-pelo-cliente-e-status-que-não-trava)
encontrou. B foi feita pela outra rota, cache no estado de sessão com stamp do binário.

E os testes que exigem ripwire, que até agora "passavam" retornando cedo por ausência dele, passaram
a **executar de verdade** contra o 0.6.5: zero pulados, todos verdes. Os fixtures gravados batem com
a saída real.

## 0. O que foi medido, e o que não foi

Medido em **build release** (a lição do [D-100](../changelog.md#d-100--itens-7-e-10-medidos-e-recusados-e-os-números-do-d-096-ao-d-099-refeitos-em-release):
os números de debug inflam de 10 a 20x em trabalho ligado a CPU), com um dublê de `ripwire --mcp`
em Python que responde **instantaneamente**, mediana de 15 execuções, depois de aquecimento.

| evento de hook | custo por evento |
| --- | --- |
| `user-prompt-submit` | 43,0 ms |
| `post-tool-use` | 43,0 ms |
| `stop` | 42,6 ms |

**Os três custam o mesmo, e é isso que importa.** O dublê responde na hora, então não há trabalho
de ripwire nessa conta: os 43 ms são overhead fixo de subida. Decomposto:

| parcela | custo |
| --- | --- |
| subir o binário `ripwire-broker` | 2,6 ms |
| `ripwire --version`, um processo inteiro só para ler a versão | 19,4 ms |
| `ripwire --mcp` mais o handshake MCP | ~19 ms |
| trabalho do broker | ~2 ms |

**O que não foi medido, e por isso não é afirmado aqui:** o custo real do ripwire. O binário não
está no PATH desta máquina. Os 19,4 ms de cada subida são o startup do **Python**; um ripwire em
Rust subiria em poucos milissegundos. Em troca, o ripwire real faz trabalho de indexação a cada
subida, que o dublê não faz e que pode dominar o número real em qualquer direção. **O número real
por evento é desconhecido; o que está estabelecido é que ~38 dos 43 ms são as duas subidas de
processo, e que nenhuma fração disso depende do evento.**

Uma primeira medição deu 146,9 ms para `user-prompt-submit` e ~50 ms para os outros dois. Era
artefato de cache frio — foi o primeiro do laço. Fica registrado porque a diferença de 3x parecia
um achado sobre o evento e não era.

## 1. Quantos eventos um turno paga

O `install --hooks` liga três eventos ([`src/install.rs`](../../src/install.rs), `events()`):

| evento | quando dispara |
| --- | --- |
| `UserPromptSubmit` | uma vez por prompt |
| `PostToolUse` | **uma vez por chamada** de `Edit`, `Write`, `MultiEdit` ou `NotebookEdit` |
| `Stop` | uma vez por turno |

Um turno com `N` edições paga `N + 2` eventos. Com 10 edições, `12 x 43 ms = 516 ms` de overhead
fixo por turno, no dublê instantâneo — e o `PostToolUse` é o que escala, porque é por edição.

## 2. A árvore de processos por evento

[`src/local.rs`](../../src/local.rs) `launch()` → [`src/upstream.rs`](../../src/upstream.rs):

1. o host sobe `ripwire-broker hook …`
2. `ripwire_version(&binary)` sobe **`ripwire --version`** e o descarta
3. `RipwireUpstream::spawn` sobe **`ripwire --mcp`**
4. `Broker::connect` faz `list_tools()` sobre stdio

Com `--ripwire-max-rss-mb` ([`src/main.rs`](../../src/main.rs)) entram dois processos a mais por
evento: `ripwire-broker __supervise` e `ripwire-broker __watch`. O supervisor é `None` por padrão
em `UpstreamConfig::new`, então o caminho padrão são **3 processos por evento**, e **5** com o
limite de memória ligado.

## 3. O recurso que já existe e fica ocioso

Este é o fato que reenquadra o item.

Um `install` padrão escreve **as duas coisas**: o servidor MCP em `.mcp.json` e os hooks em
`.claude/settings.json`. E o servidor MCP guarda `broker: Mutex<Option<Arc<Broker>>>`
([`src/mcp.rs`](../../src/mcp.rs)), conectado sob demanda e **mantido pela sessão inteira**, com um
ripwire filho vivo e aquecido.

Ou seja: numa instalação padrão já existe um broker de vida longa com ripwire quente, ocioso entre
chamadas de tool, **enquanto cada evento de hook sobe um broker e um ripwire novos ao lado dele**.
O item 4 não é necessariamente "construir um daemon" — é "deixar de duplicar um processo que já
está lá".

O que impede hoje: o hook é um processo separado, sem handle para o servidor MCP, e o servidor MCP
não escuta nada além do stdio que o host lhe deu.

## 4. O que qualquer mitigação tem de preservar

Nomeados aqui para que nenhuma opção seja avaliada sem eles.

| invariante | origem | por que colide |
| --- | --- | --- |
| Nenhum ripwire fica rodando sem vigia | [D-050](../changelog.md#d-050--limite-de-memória-do-ripwire-por-supervisor), [D-052](../changelog.md#d-052--code-review-das-fases-2-e-3) | o `__watch` mata o ripwire assim que o supervisor morre. Um ripwire de vida longa compartilhado muda quem é o pai e quando é legítimo matá-lo |
| O limite de memória do ripwire (PRD 15.3) | [D-050](../changelog.md#d-050--limite-de-memória-do-ripwire-por-supervisor) | hoje o limite é por processo de vida curta. Um ripwire que atravessa a sessão acumula, e o teto passa a valer contra um processo que ninguém reinicia |
| Hooks são offline | [D-064](../changelog.md#d-064--cache-diagnóstico-e-integração-proposta) | "processo curto, timeout do host e consentimento por processo não combinam com envio remoto a cada prompt". Um daemon de vida longa **remove as três premissas dessa decisão**, o que reabre o assunto em vez de resolvê-lo |
| Estado de sessão por `session_id`, com lock | [D-032](../changelog.md#d-032--estado-de-sessão-em-disco-proposta) | `StateStore::lock` hoje serializa hooks paralelos de uma sessão. Um daemon passa a ter o estado em memória e o arquivo vira cache, ou dois donos disputam a verdade |
| Sem telemetria, sem rede no build padrão | CA-10 | um socket local não é rede externa, mas é superfície nova e precisa dizer isso explicitamente |

## 5. Opções

### A — Não fazer nada

**Custo:** ~43 ms medidos × (`N` + 2) por turno, com o número real por evento desconhecido.
**Risco:** zero. **Superfície nova:** nenhuma.

Vale como linha de base honesta: se o ripwire real subir em poucos milissegundos e indexar rápido,
o item 4 pode simplesmente não valer o que custa. **Medir o ripwire real é pré-requisito de
qualquer outra opção**, e não foi possível aqui.

### B — Parar de subir um processo só para ler a versão

`ripwire_version()` sobe um processo inteiro por evento e joga fora. Duas saídas: ler a versão do
handshake MCP que já acontece, ou cachear por caminho e mtime do binário no estado de sessão.

**Ganho medido:** 19,4 ms por evento no dublê — **45% do total**. Com ripwire em Rust o ganho
absoluto cai junto com o startup, mas continua sendo **um processo a menos por evento, sempre.**
**Risco:** baixo e contido. **Superfície nova:** nenhuma. **Não depende de nenhuma das outras
opções.**

Isto é barato, isolado e mensurável. Independente de qual opção seja escolhida para o resto,
**esta merece ser tratada como item próprio**, não como parte de uma mudança de arquitetura.

### C — Reusar o ripwire do servidor MCP por um socket local

O servidor MCP publica um socket de domínio Unix no diretório de estado; o hook conecta, manda o
evento, recebe o envelope.

**Ganho:** o hook deixa de subir qualquer ripwire. Sobra o custo do próprio processo de hook
(2,6 ms) mais uma ida e volta local.

**O que isto cria, e é o ponto que precisa de decisão:**

- **Permissão e autenticação.** Um socket em disco é alcançável por qualquer processo do usuário.
  Sem autenticação, qualquer coisa na máquina pede contexto do workspace — e com `--online`
  ligado no servidor, **pede envio remoto de conteúdo do workspace**. Precisa de permissão `0600`,
  diretório com dono verificado, e um segredo por sessão que o hook prove conhecer.
- **Ciclo de vida.** O servidor MCP morre quando o host encerra. Se um hook disparar depois disso,
  precisa de fallback para o caminho de hoje — então o código antigo **não sai**, ele ganha um
  caminho paralelo. Duas rotas para o mesmo resultado é mais superfície de teste, não menos.
- **Confusão de workspace.** Dois hosts em workspaces diferentes, dois sockets. Errar qual é
  **vaza contexto de um repositório para outro**. O socket tem de ser nomeado pelo workspace
  canônico, e o hook tem de verificar, não confiar no nome.
- **Quem é o dono do estado de sessão.** Servidor e hook passam a disputar; o lock de hoje não
  cobre processos que não usam o arquivo.
- **Reabre o D-064.** Com um servidor de vida longa que pode estar `--online`, "hooks são offline"
  deixa de se sustentar por construção e passa a precisar de imposição explícita.

**Risco:** alto, e é risco de **segurança**, não de performance.

### D — Um daemon próprio do broker

Um processo dedicado, subido pelo primeiro hook e mantido por inatividade.

Tem **todos** os problemas da opção C, mais: quem sobe, quem mata, o que acontece se morrer no meio
de um turno, como o `__watch` do D-050 passa a vigiar algo que sobrevive a quem o criou, e um
processo a mais na conta do PRD 15.3. Ganha só uma coisa sobre C: funciona sem o servidor MCP
instalado.

**Risco:** o mais alto das cinco. **Recomendação:** não, a menos que se conclua que hooks sem
servidor MCP são o caso principal — e o `install` escreve os dois.

### E — Reduzir o número de eventos

Não toca em processo nenhum: faz o `PostToolUse` não disparar contexto a cada edição. Um debounce
por tempo, ou só edições que mudam elegibilidade, ou agrupar as edições de um turno e resolver no
`Stop`.

**Ganho:** ataca o `N` em vez do custo por evento, e é o `N` que escala. **Risco:** médio, e é de
**produto**, não de segurança — o contexto chega mais tarde ou menos vezes, o que muda o que o
agente vê e quando. Isso é matéria de PRD 8.4, não de otimização.

**Superfície nova:** nenhuma. Nenhum processo novo, nenhum socket, nenhuma autenticação.

## 6. Como as opções se comparam

| opção | ganho | superfície nova | risco | independente |
| --- | --- | --- | --- | --- |
| B — não subir processo pela versão | 19,4 ms/evento no dublê; um processo a menos sempre | nenhuma | baixo | **sim** |
| E — menos eventos | ataca o `N`, que é o que escala | nenhuma | médio, de produto | **sim** |
| A — não fazer nada | — | nenhuma | zero | — |
| C — socket para o servidor MCP | quase todo o overhead | socket, autenticação, dois caminhos | **alto, de segurança** | não |
| D — daemon próprio | igual a C | tudo de C mais ciclo de vida | **o mais alto** | não |

## 7. Recomendação

**Fazer B agora, como item próprio.** É um processo a menos por evento, sem superfície nova,
mensurável, e não depende de decidir nada sobre daemon.

**Medir o ripwire real antes de considerar C ou E.** Todo o resto desta proposta repousa num
número que não tenho: quanto o ripwire real custa por subida. Se for pequeno, C e D não se pagam
pelo risco, e E é decisão de produto e não de performance. **Essa medição é o próximo passo, não
o código.**

**Tratar C e D como mudança de PRD, não como refactor.** Autenticação de socket, confusão de
workspace e o ciclo de vida sob D-050 são matéria de especificação e de revisão de segurança. E
qualquer uma das duas **reabre o D-064**, que só se sustenta porque o hook é um processo curto.

## 8. Seams de teste, se B for aprovado

| seam | onde | o que afirmar |
| --- | --- | --- |
| versão sem processo extra | `tests/cli.rs` ou `tests/hooks.rs` | um evento de hook sobe **um** ripwire, não dois — contado por um dublê que registra cada invocação |
| a versão continua correta | `tests/hooks.rs` | a versão relatada é a mesma de hoje, vinda do handshake ou do cache |
| ripwire trocado é notado | `tests/hooks.rs` | se o cache for por caminho e mtime, trocar o binário invalida o cache |
| `check_version` intocado | existente | a recusa de ripwire abaixo de `MIN_RIPWIRE_VERSION` continua valendo |

Vermelho de verdade é possível aqui: o dublê contando invocações falha hoje com 2 onde deve ser 1.

## 9. O que depende do usuário

1. **Medir o ripwire real por subida** numa máquina que o tenha — sem isso, C, D e E são decisões
   sobre um número desconhecido.
2. **Aprovar B** como item isolado, ou dizer que não vale.
3. **Se C ou D forem de interesse**, dizer isso antes de qualquer código: eles exigem proposta de
   segurança própria (autenticação do socket, escopo por workspace, ciclo de vida sob D-050) e
   reabrem o D-064.
4. **Se E for de interesse**, é decisão de produto sobre PRD 8.4: com que frequência o contexto
   deve chegar depois de uma edição.

## 10. Riscos desta proposta

- **O número central é de dublê.** Os 43 ms não incluem o trabalho do ripwire real. Se a indexação
  do ripwire dominar, B resolve pouco e C fica mais atraente — ou o contrário. Não sei qual, e a
  proposta não finge saber.
- **B pode render menos do que 45%.** Os 19,4 ms são startup do Python. Com ripwire em Rust o
  ganho absoluto encolhe; o que não encolhe é um processo a menos por evento.
- **E muda comportamento observável.** Reduzir eventos não é otimização invisível: muda o que o
  agente recebe e quando. Entra como decisão de produto ou não entra.
