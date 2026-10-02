# Proposta — edições feitas pelo shell chegam ao hook de edição

Status: **design aprovado em conversa; aguardando revisão deste documento** · 2026-10-01 ·
fecha a divergência 2 de [D-128](../changelog.md#d-128--validação-manual-da-barra-e-fixture-de-payload-real)
(PR #38; o link resolve depois do merge dele).

Fonte: PRD 8.4 (níveis de automação) e §24 (barra de status), e as decisões
[D-105](../changelog.md#d-105--a-versão-do-ripwire-deixa-de-custar-um-processo-por-evento-de-hook),
[D-106](../changelog.md#d-106--uma-rajada-de-edições-é-uma-pergunta-não-uma-por-edição) e
[D-128](../changelog.md#d-128--validação-manual-da-barra-e-fixture-de-payload-real).

## 1. Problema

Na validação manual da barra (D-128), o Claude editou `src/lib.rs` com a ferramenta Bash
(`echo '…' >> src/lib.rs`), não com o Edit, sem ninguém pedir. O PostToolUse do broker só casa
`Edit|Write|MultiEdit|NotebookEdit` (`src/install.rs`), então o Claude Code nem chamou o hook: a
edição não teve contexto depois da edição nem contou em `stats.events`.

O ponto cego é **só no meio do turno**. O gate do `Stop` (`context_before_finish`) pergunta ao
ripwire `situational_awareness` e `quality_delta`, que leem os arquivos mudados na árvore de
trabalho, então a edição por shell é vista no fim do turno. O que se perde é o
`context_after_edit` logo depois da edição (chamadores, testes afetados, riscos) e a contagem.

Edição por shell não é rara: `echo >>`, `sed -i`, `cat > arquivo`, formatadores (`cargo fmt`),
geradores (`npm create`, `cargo new`) e scripts do projeto.

## 2. Objetivo e critérios de sucesso

Objetivo (escolha do mantenedor): uma edição feita pelo shell recebe o mesmo contexto depois da
edição que uma feita com o Edit, logo depois do comando, e conta em `stats.events`.

Critérios:

1. Um comando Bash que muda um arquivo do workspace leva ao `context_after_edit` com esse arquivo,
   pelo mesmo fluxo do Edit (coalescência, `has_news`, `record`).
2. Um comando Bash que não muda nada **não sobe o ripwire** e não conta evento.
3. O custo acrescentado a um Bash só de leitura fica com p95 ≤ 50 ms num repositório grande; acima
   disso o design é revisto antes do merge. O mesmo `git status` passa a rodar também em todo
   `UserPromptSubmit` e todo PostToolUse do Edit (§4.4), e entra na mesma medição.
4. Sem git, o comportamento é o de hoje (silêncio), documentado.

Fora do escopo: o Codex (a ferramenta de shell dele tem o mesmo ponto cego; o matcher do Codex
fica como está nesta rodada) e separar a edição do comando de outras mudanças simultâneas (§5.6).

## 3. Abordagens consideradas

| | Como detecta | Custo | Cobre formatador/gerador | Escolha |
| --- | --- | --- | --- | --- |
| **A** | impressão digital da árvore do git, comparada com a do hook anterior, **antes** de subir o ripwire | um `git status` por Bash | sim | **escolhida** |
| B | ler os alvos de escrita no texto do comando (`>`, `>>`, `tee`, `sed -i`, `mv`, `cp`) | nenhum | não (scripts, `cargo fmt`, geradores) | recusada: frágil |
| C | subir o ripwire a cada Bash e usar `situational_awareness` | um ripwire por Bash, inclusive `ls` | sim | recusada: lenta |

Cada hook hoje sobe o ripwire antes de fazer qualquer coisa (`hook::run`, `local::launch`; ~34 ms
num `post-tool-use` medido em D-105, mais o trabalho do ripwire). O Claude roda Bash o tempo todo, e
a maior parte é leitura (`cat`, `ls`, `cargo test`). Só a opção A decide "mudou alguma coisa?" de
forma barata antes dessa subida.

## 4. Design

### 4.1 Matcher

`install` passa a escrever, para o Claude Code, `Edit|Write|MultiEdit|NotebookEdit|Bash`. Como o
`install` reconhece os próprios hooks (`is_ours`) e os substitui, rodar `install … --write` de novo
atualiza uma instalação antiga. O Codex mantém `apply_patch|Edit|Write`.

### 4.2 Unidade nova: `src/worktree.rs`

Pequena, pura exceto por uma chamada ao `git`:

- `fingerprint(root) -> Option<Fingerprint>`: roda
  `git -C <root> status --porcelain=v1 -z --untracked-files=all` com timeout de 500 ms e guarda,
  para cada caminho listado, `(mtime_ns, size)` ou a marca "apagado". Devolve `None` quando a raiz
  não é repositório git, o `git` não existe, a chamada falha ou estoura o tempo, ou a lista passa
  de `MAX_FINGERPRINT_ENTRIES` (5000).
- `changed(before, after) -> Vec<String>`: caminhos novos, mudados (outro `mtime_ns` ou `size`) ou
  apagados em `after` em relação a `before`, mais os que estavam sujos antes e ficaram limpos
  (reverter ou commitar também é edição). Ordem de `after`, depois os que sumiram; sem repetição.

`Fingerprint` é serializável e vai para o estado da sessão.

### 4.3 Onde roda

Em `hook::run`, **antes** de `local::launch`, para `PostToolUse` com `tool_name == "Bash"`:

1. tirar a impressão digital atual e comparar com `state.worktree` (a anterior);
2. gravar a atual em `state.worktree`;
3. sem mudança (ou sem impressão anterior, §5.2): salvar o estado e sair em silêncio, **sem subir
   o ripwire e sem contar evento**;
4. com mudança: os caminhos viram os arquivos editados, limitados a `MAX_BASH_EDIT_FILES` (50), e
   daí em diante o fluxo é o do Edit — filtro `in_workspace`, coalescência com `held_edits`
   (D-106), `context_after_edit`, `has_news`, `record`.

### 4.4 Base da comparação

`UserPromptSubmit` e **todo** `PostToolUse` (o do Edit também) gravam a impressão digital. Assim
uma edição feita pelo Edit não é atribuída ao Bash seguinte, e mudanças anteriores ao prompt não
são atribuídas ao turno. O `Stop` não precisa gravar. Nos eventos que não são Bash a impressão é
tirada depois de decidir a resposta, sem mudar o caminho deles.

### 4.5 Contagem

Um evento Bash só conta em `stats.events` quando mudou algo. Comandos só de leitura continuam
invisíveis, como hoje; `events` continua significando "eventos que o broker processou".

## 5. Casos de borda e falhas

1. **Sem git** (não é repositório, `git` ausente, falha, mais de 500 ms): `fingerprint` dá `None`,
   o evento Bash é silêncio como hoje, sem mensagem de falha (nada quebrou); o gate do `Stop`
   continua cobrindo o turno. README e PRD dizem: "edições pelo shell só são vistas no meio do
   turno em workspace git".
2. **Sem base** (primeiro Bash de uma sessão começada antes da atualização, ou `None` antes):
   grava a impressão e trata como "sem mudança". Sem base não há o que atribuir, e inventar é pior.
   Na prática o `UserPromptSubmit` cria a base.
3. **Opt-out:** com `hooks off`, o PostToolUse ainda atualiza a impressão digital, sem subir o
   ripwire, para que o `#ripwire-on` não atribua ao próximo comando tudo o que mudou na pausa.
4. **Mudança grande:** depois do `in_workspace`, no máximo 50 arquivos (ordem do `git status`); o
   resto é descartado, e o gate do `Stop` vê todos. Acima de 5000 entradas a impressão vira `None`
   e o workspace se comporta como o caso 1 ("sujo demais para acompanhar"), sem inchar o arquivo
   de estado.
5. **Precisão:** `mtime` em nanossegundos mais `size`. Duas escritas do mesmo tamanho no mesmo
   nanossegundo passam despercebidas; a próxima edição real ou o `Stop` cobrem.
6. **Mudança que o comando não fez** (edição à mão no editor, watcher, servidor de
   desenvolvimento) é atribuída a esse Bash. O contexto continua correto (o arquivo mudou mesmo);
   só a atribuição erra. Documentado, sem tentativa de separar.
7. **Concorrência:** o lock por sessão de `run` já serializa os hooks; ler e gravar a impressão
   digital é seguro.

## 6. Testes

TDD como no D-127: vermelho, verde, e uma mutação por teste novo.

1. **`worktree.rs`**, em repositórios git temporários com o `git` real: arquivo novo não rastreado,
   modificado e apagado aparecem em `changed`; arquivo sujo que voltou a limpo aparece; arquivo
   ignorado pelo `.gitignore` não aparece; diretório sem git dá `None`; mais de 5000 entradas dá
   `None`.
2. **Hook** (`tests/hooks.rs` e `tests/cli.rs`):
   - **o atalho:** `PostToolUse:Bash` só de leitura pela CLI com `--ripwire` apontando para um
     binário que **não existe** sai em silêncio, sem `no context`, e `events` não muda (se o
     ripwire fosse subido, o teste veria a falha);
   - Bash que edita `src/lib.rs` leva ao `context_after_edit` com esse arquivo, `events` +1, e
     injeção quando há `has_news`;
   - Edit seguido de Bash só de leitura não atribui nada;
   - opt-out atualiza a impressão, e depois do `#ripwire-on` nada da pausa é atribuído;
   - mudança de 60 arquivos manda no máximo 50 na requisição;
   - sem git, silêncio.
3. **Fixture:** um `PostToolUse:Bash` real, capturado do Claude Code com um hook de captura
   temporário (como o `capture.sh` da barra), em
   `tests/fixtures/hooks/claude_code_post_tool_use_bash.json`, para confirmar `tool_name` e
   `tool_input.command` em vez de supô-los.
4. **Install:** o matcher do Claude Code inclui `Bash`, e reinstalar sobre uma instalação antiga
   atualiza o matcher.

## 7. Documentação e medição

- PRD: seção de hooks com a detecção pelo shell, os limites (só git, tetos, atribuição) e o custo.
- README: nota de atualização — rodar `install … --write` de novo.
- Changelog: entrada D nova, que fecha a divergência 2 do D-128.
- Roteiro manual (`statusline-manual/ROTEIRO.md`, local): variante Bash do passo 3.2.
- Medição: o tempo que o `git status` acrescenta a um Bash só de leitura, neste repositório e num
  grande, com p50/p95 registrados na entrada D. p95 acima de 50 ms reabre o design antes do merge.
