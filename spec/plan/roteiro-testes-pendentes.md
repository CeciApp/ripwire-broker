# Roteiro — testes pendentes

Estado em 2026-10-10, `master` em `c7d4a4d` (D-168). **A1, A3 e A4 já foram rodados nesse commit
(D-169) e passam**; o A3 foi medido de novo pelo cronômetro do próprio hook (D-170) e o B foi
reproduzido e diagnosticado (D-171); ficam aqui para a próxima vez. A suíte automática está verde (840 no build
padrão, 877 com `online`, 46 do plugin). O que falta são testes que pedem algo de fora: uma chave,
um modelo local, uma sessão real de um host, o corpus privado ou dinheiro.

Ordem sugerida: do mais barato ao mais caro. Cada bloco diz o que é preciso, os comandos, o que
conta como aprovado e onde registrar.

## Resumo

| # | Teste | Precisa de | Custo | Quem |
|---|---|---|---|---|
| A1 | Testes ao vivo contra o Jev (3) | chave do Jev | centavos | qualquer um |
| A2 | Nota por um modelo local real (1) | `ollama` e um modelo | nada | qualquer um |
| A3 | SLO do `hook --memory` | ripwire, build release | nada | qualquer um |
| A4 | Overhead de batching e merge | build release | nada | qualquer um |
| A5 | `--summarizer-cache` em uso real | modelo local | nada | qualquer um |
| B | **Leitura de memória não entrega nada** (bloqueia C) | chave do Jev | ~US$ 0,30 por sessão | investigação |
| C | T3.11: memória no Claude Code e no Codex | B resolvido | ~US$ 1 | mantenedor |
| D | Plugin: P1, P2, P3, P6, P7, P8 | sessão interativa | pouco | mantenedor |
| E | Barra de status: o campo `agent` | sessão com `--agent` | pouco | mantenedor |
| F | `session_hits`: fechar as 20 sessões | 3 sessões de trabalho real | nada | mantenedor |
| G | A/B (PRD §16.2) | corpus privado, outra máquina | piloto ≤ US$ 3; rodada: medir no piloto | mantenedor |
| H | T5.3: rodada da memória | corpus em sequências | pago | mantenedor |

Não entram: os dois testes auxiliares marcados `#[ignore]`
(`the_model_runs_with_the_key_in_this_process`, `the_fingerprint_runs_with_the_key_in_this_process`),
que outros testes disparam como subprocesso; e os testes ao vivo contra a Cloudflare, porque a
Cloudflare não será o provider (D-167).

## Antes de tudo

```sh
cd ~/projects/ai/ripwire-broker
git switch master && git pull --ff-only
cargo build --release --features online
echo ${#RIPWIRE_BROKER_JEV_API_KEY}     # 107 = chave do Jev (TypeSafe). 53 = token da Cloudflare: não rode nada ao vivo
which ripwire claude
```

A variável guarda a credencial de um provider por vez. Se o tamanho não for 107, pare: os testes
ao vivo mandariam a credencial ao provider errado.

## A1 · Testes ao vivo contra o Jev

Três testes que só rodam com `--ignored`. Mandam só conteúdo sintético.

```sh
cargo test --features online --test online_live -- --ignored --nocapture
```

**Aprovado:** `3 passed`. São `a_real_provider_classifies_the_synthetic_corpus`,
`a_real_provider_enriches_a_synthetic_repository` e `the_pinned_model_answers_a_choice`.
**Se falhar:** sem chave eles falham de propósito; com 401, a chave está errada ou vencida
(`ripwire-broker doctor --workspace . --jev-probe` confirma).

## A2 · Nota por um modelo local real

```sh
ollama pull phi4                         # ou outro modelo que você já tenha
RIPWIRE_BROKER_TEST_MODEL="ollama run --nowordwrap phi4" \
  cargo test --test summarizer -- --ignored a_real_local_model_writes_a_note --nocapture
```

**Aprovado:** `1 passed`, e a nota impressa é texto legível, sem códigos de terminal.
O `--nowordwrap` importa: sem ele o `ollama` redesenha a linha mesmo por um pipe.

## A3 · SLO do `hook --memory`

O que `--memory` acrescenta a um hook: p95 ≤ 10 ms e p99 ≤ 25 ms (PRD jev-mem §8.2).

```sh
cargo test --release --locked --test cli -- --ignored the_hook_overhead_meets_the_slo --nocapture
```

**Aprovado:** o teste passa e imprime os percentis. Rode com a máquina ociosa; anote p95 e p99.
Desde o D-170 os percentis são do tempo que o hook mede em si mesmo (as linhas `collect` do
`debug.log`); a diferença de relógio entre dois hooks inteiros sai junto, só como conferência.
**Registrar:** no changelog, com a data e a máquina.

## A4 · Overhead de batching e merge

```sh
cargo test --release --test online overhead -- --ignored --nocapture
```

**Aprovado:** é uma medição, não tem limiar no código. Compare com o número anterior no changelog
(procure por `overhead_of_batching_and_merge`) e anote se piorou.

## A5 · `--summarizer-cache` em uso real

O S3.15 só foi exercitado com modelo falso e com um script `sh`. Falta um modelo de verdade.

```sh
W=$(mktemp -d) && cd "$W" && git init -q && mkdir src && printf 'def login(u):\n    return bool(u)\n' > src/auth.py \
  && git add -A && git -c user.email=t@t -c user.name=t commit -qm x
S=$(mktemp -d)                            # o state dir do teste
```

A nota só aparece pelo servidor (o comando `prompt` não roda o modelo), então use um cliente MCP:

1. Nesse repositório, crie `.mcp.json` apontando para o binário release, com os argumentos
   `--workspace <repo> --state-dir <S> --summarizer-cmd "ollama run --nowordwrap phi4" --summarizer-cache`.
2. `claude --mcp-config .mcp.json --strict-mcp-config`, e peça algo que chame `context_for_task`
   em modo `orient` (por exemplo: "me oriente sobre como o login funciona").
3. Repita o pedido até a resposta trazer `notes` (a primeira pode vir com `note_pending`).
4. `ls -l "$S/notes/"`: um arquivo `<hash>.json`, modo `-rw-------`.
5. Feche o Claude Code, abra de novo e repita o pedido.

**Aprovado:** na segunda sessão a nota vem com `cached: true` na primeira resposta, sem esperar o
modelo; o recurso de status mostra `summarizer.persistent: true` e `generated: 0`.

## B · A leitura de memória não entrega nada (bloqueia o C)

**Diagnosticado no D-171:** a leitura funciona; o que ela recusa é o texto das observações
automáticas, que não diz sobre o que a análise foi (relevância 0,20 e 0,16 contra a barra de 0,60;
uma nota explícita no mesmo store teve 0,91 e foi entregue). O conserto é a T3.12 do plano
(`memory-observation/v2`, PRD v0.4 §5.2). Este bloco passa a ser o aceite dela.

Achado do D-168: num Claude Code real, a memória é coletada, ingerida e enriquecida, mas a leitura
visita as memórias, pergunta ao Jev e não mantém nenhuma (`kept=0`, `stop=empty`). Enquanto isso
valer, não há memória no envelope e o C não tem o que observar.

Para reproduzir:

```sh
W=$(mktemp -d)/repo && mkdir -p "$W/src" "$W/tests" && cd "$W" && git init -q
cat > src/auth.py <<'EOF'
TOKEN_TTL_SECONDS = 3600

def validate_token(token, now, issued_at):
    """True when the token is non-empty and younger than the TTL."""
    if not token:
        return False
    return now - issued_at < TOKEN_TTL_SECONDS
EOF
printf 'from src.auth import validate_token\n\ndef test_valid():\n    assert validate_token("a", now=1, issued_at=0)\n' > tests/test_auth.py
git add -A && git -c user.email=t@t -c user.name=t commit -qm sample
B=~/projects/ai/ripwire-broker/target/release/ripwire-broker
$B install claude-code --workspace "$W" --online --memory --memory-debug-log --write
```

O `install` imprime o caminho do `debug.log`; guarde-o em `$LOG`.

```sh
T="mcp__ripwire-broker__context_for_task mcp__ripwire-broker__context_after_edit mcp__ripwire-broker__context_before_finish Read Edit Grep Glob"
# sessão 1: semeia
claude -p "In src/auth.py change TOKEN_TTL_SECONDS to 900. Use the ripwire-broker tools as their instructions say." \
  --mcp-config .mcp.json --strict-mcp-config --allowedTools $T --permission-mode acceptEdits \
  --max-budget-usd 1 --output-format stream-json --verbose > ../s1.jsonl
$B memory status --workspace "$W"          # espera-se: 2 memories, 0 pending
# sessão 2: lê
claude -p "Add a test that a token older than the TTL is rejected. Use the ripwire-broker tools as their instructions say." \
  --mcp-config .mcp.json --strict-mcp-config --allowedTools $T --permission-mode acceptEdits \
  --max-budget-usd 1 --output-format stream-json --verbose > ../s2.jsonl
grep ' read ' "$LOG" | tail -2
```

**O que olhar:** a linha `read … kept=N visited=M requests=R stop=…`.

- `visited=0`: a leitura não achou candidatos (índice ou recuperação).
- `visited>0`, `kept=0`, `stop=empty`: os candidatos chegaram ao Jev e ele não aceitou nenhum.
  É o estado de hoje, e a causa é conhecida (D-171). Para ver as notas do Jev, acrescente
  `--log` aos `args` do `.mcp.json` (o `install` não aceita a flag) e leia o `jev.log` do state
  dir. Depois da T3.12, se ainda der `kept=0`, o que se revê é o texto da v2, não a barra de 0,60.
- `kept≥1`: resolvido; siga para o C.

**Aprovado:** `kept≥1` numa tarefa ligada a uma edição anterior, e `memories` no resultado de
`context_for_task` (`grep -c '"memories"' ../s2.jsonl` maior que zero).

## C · T3.11: os hosts usam a memória

Usa o mesmo repositório do B. Enquanto a T3.12 não existir, nenhuma observação automática é
entregue: semeie com uma nota explícita antes da sessão que lê, e registre que foi assim.

```sh
echo '{"text": "In src/auth.py, TOKEN_TTL_SECONDS was lowered from 3600 to 900 seconds."}' > ../note.json
$B memory add --workspace "$W" --file ../note.json
```

### Claude Code

1. Rode as duas sessões do bloco B. Anote `claude --version`.
2. Em `s2.jsonl`, ache o resultado de `context_for_task` e confirme:
   - o campo `memories[]` com pelo menos uma entrada;
   - a seção legível de memória no bloco de texto.
3. Leia a resposta final do agente e os `tool_use` que vieram depois do `context_for_task`.

**Aprovado:** o agente age sobre o que a memória diz. Sinais aceitáveis: cita a edição anterior,
vai direto ao arquivo que a memória aponta sem procurar, ou evita refazer o que ela diz que foi
feito. **Reprovado:** a memória está no envelope e nada no comportamento muda; marque o host como
"não usa" no README, que também é um resultado.

### Codex

```sh
$B install codex --workspace "$W" --online --memory --memory-debug-log --write
```

Repita as duas sessões com o Codex em modo não interativo, gravando a saída em JSON. Não conferi
as flags do Codex nesta rodada: confirme em `codex --help` antes (o repositório tem a
configuração em `integrations/codex/`).

### Registrar

- a versão de cada host;
- o trecho do transcript com o resultado de `context_for_task`, reduzido e sem nada privado, como
  fixture em `tests/memory_hosts.rs` (o arquivo ainda não existe);
- o README, que hoje diz "Hosts: not validated";
- a T3.11 em `spec/plan/jev-mem-plan.md`.

## D · Plugin do Claude Code

As pendências do §5a de `spec/plan/mod-plan.md`. Tudo numa sessão interativa, num repositório de
teste, com o Claude Code 2.1.292 ou mais novo.

```sh
cd <repositório de teste>
claude --plugin-dir ~/projects/ai/ripwire-broker/integrations/claude-code
```

| # | Passo | Aprovado |
|---|---|---|
| P1 | `/mcp`; depois peça um `context_for_task` | `plugin:ripwire-broker:broker` conectado, e `provenance.workspace` igual ao diretório do projeto |
| P2 | Um prompt; uma edição; encerre. Depois `ripwire-broker hook-stats` | o prompt injeta contexto, a edição injeta `context_after_edit`, o `Stop` roda, e a sessão conta no `hook-stats`. Com `.mcp.json` de projeto **e** o plugin: anote se há aviso de servidor duplicado |
| P6 | Na mesma sessão, com o mod ativo | (b) um hook clássico vê `RIPWIRE_BROKER_MOD_ACTIVE=1` e fica calado; (c) anote a ordem de `tool.call` e `PostToolUse`; (e) um valor em `$.state` sobrevive a editar `register.ts` com a sessão aberta; (f) o resultado de um Bash traz `bashEditDiff.changedFiles` |
| P7 | Olhe acima do prompt; rode `/ripwire-status` | a faixa `rw-brkr · …` aparece e o comando responde |
| P3 | `claude plugin marketplace add .` e `claude plugin install ripwire-broker@aquental --scope user`, noutro repositório de teste | instala; anote se a cópia é in-place ou no cache |
| P8 | Meça a latência de um hook pelo resolvedor (`scripts/broker`) contra o binário direto, e a do `prompt.submit` do mod contra `context_for_task` | números anotados; fecham a T3.4 e a T6.1 |

**Registrar:** cada linha no §5a do `mod-plan.md` e, se algo divergir, uma decisão no changelog.

## E · Barra de status: o campo `agent`

Nenhum payload real trouxe o campo `agent` (§24.8); a validação do D-128 rodou sem `--agent`.

1. Use o roteiro de `~/projects/ai/CECI/statusline-manual/` (pasta local, não versionada), com o
   `capture.sh` como comando de status.
2. Abra uma sessão com `claude --agent <nome de um agente seu>`.
3. Confira num payload gravado se o campo `agent` veio, e com que forma.

**Aprovado:** um payload real com `agent`, e a barra mostrando o que o §24.8 prevê. Se a forma for
outra, é um defeito a corrigir com o payload como fixture.

## F · `session_hits`: fechar as 20 sessões

A decisão do S3.15 já foi tomada com 17 sessões (D-168); isto só fecha a regra do §21.3.

```sh
ripwire-broker hook-stats
```

**Aprovado:** `sessions` ≥ 20. Anote a repetição entre sessões e compare com os 44,6% de hoje.
Se cair abaixo de 30%, registre: o S3.15 é opt-in, então nada precisa ser desfeito.

## G · A/B (PRD §16.2)

O corpus (32 tarefas) mora na outra máquina, em `~/projects/ai/CECI/ab-eval/`, e nunca entra
neste repositório. Nesta ordem:

1. **Revisar os enunciados.** As ressalvas estão no D-117: alguns nomeiam interfaces, um entrega o
   diagnóstico. Reescreva esses.
2. **Porta e relógio.** Dê a cada tarefa do repositório A uma porta própria e o relógio congelado
   no `env` (D-121).
3. **Validar.**
   ```sh
   ./target/release/ripwire-eval check    --corpus corpus.json
   ./target/release/ripwire-eval validate --corpus corpus.json
   ```
   **Aprovado:** as 30 tarefas com `check` falham no `base` e passam no `fix`.
4. **Piloto pago.** Um corpus com 3 tarefas, 3 braços, teto de US$ 3 por execução (o
   `--max-budget-usd` vai no `--agent-cmd`; `ripwire-eval --help` mostra o padrão).
   ```sh
   ./target/release/ripwire-eval run --corpus piloto.json --out piloto/ --arms none,ripwire,broker --repeats 1
   ./target/release/ripwire-eval report --out piloto/
   ```
   **Aprovado:** as 9 execuções terminam válidas. Anote o custo por execução e guarde um
   transcript real (a fixture de hoje é sintética).
5. **A rodada.** 32 tarefas × 3 braços × 3 repetições. Estime o custo pelo piloto antes.
   ```sh
   ./target/release/ripwire-eval run --corpus corpus.json --out ab/ --arms none,ripwire,broker --repeats 3
   ./target/release/ripwire-eval report --out ab/
   ```
   O braço `broker-online` só com a chave e com consentimento para mandar trechos dos três
   repositórios ao provider.

**Aprovado:** as barras do §17 e do §23.15 no relatório. Se passarem, o `--online` deixa de ser
experimental e o §23 do PRD muda.

## H · T5.3: rodada da memória

Depende da T3.12 e do B (sem observação automática entregue, os braços de memória medem só o
custo). O corpus em sequências ainda não existe.

- **Corpus:** ≥ 30 tarefas em sequências, fora deste repositório.
- **Braços:** `broker`, `broker-memory`, `broker-memory-deterministic`.
- **Limitações do instrumento que a rodada tem de tratar** (D-142): cada braço tem o próprio
  histórico; os braços rodam na ordem de `--arms`, sem aleatorização; a ingestão de uma sessão é
  paga pela seguinte.

```sh
./target/release/ripwire-eval run --corpus sequencias.json --out mem/ \
  --arms broker,broker-memory,broker-memory-deterministic --repeats 3
./target/release/ripwire-eval report --out mem/
```

**Aprovado:** os gates do PRD jev-mem §14. Sem evidência suficiente, `--memory` continua
experimental, e isso também se registra.

## Onde registrar cada resultado

| Resultado | Lugar |
|---|---|
| Qualquer medição ou decisão | `spec/changelog.md`, uma entrada nova |
| Estado geral | `handoff.md` ("Em andamento" e "Pendências conhecidas") |
| T3.11, T5.3 | `spec/plan/jev-mem-plan.md` e o README |
| P1 a P8 | `spec/plan/mod-plan.md` §5a |
| A/B | `spec/plan/plano-ab-e-session-hits.md` e o §23 do PRD |
