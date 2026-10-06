# ripwire-broker

A local MCP server that turns [Ripwire](https://github.com/redhat-et/ripwire)'s wide surface
(33 verbs) into three task-moment tools with a token budget, deduplication, provenance and
preserved limitations. It is local, offline and read-only by default. An optional
[online mode](#online-mode-optional) adds a remote semantic classifier to `context_for_task`,
only when the process is started with `--online`. Specification:
[`spec/ripwire-broker-mcp.md`](spec/ripwire-broker-mcp.md). Decision log:
[`spec/changelog.md`](spec/changelog.md).

```text
agent ⇄ stdio ⇄ ripwire-broker ⇄ stdio ⇄ ripwire <workspace> --mcp
```

## Requirements

- Rust 1.98.1 (pinned in `rust-toolchain.toml`)
- `ripwire` ≥ 0.6.4 on `PATH` (or pass `--ripwire BIN`)
- Online mode only: a build with `--features online` and a TypeSafe API key

## Build and run

```sh
cargo build --release
./target/release/ripwire-broker --workspace /path/to/repo \
  [--ripwire /path/to/ripwire] [--timeout-ms 60000] [--redact-workspace] [--incremental]
```

That is the MCP server (`serve`, the default command). The same binary has one-shot
commands; `ripwire-broker --help` lists them all:

| Command | Purpose |
| --- | --- |
| `hook <claude-code\|codex> <event>` | Automatic context from a host hook ([below](#automatic-mode-hooks)) |
| `hook-log --session ID` | What the hooks injected in a session (counts only) |
| `hook-stats [--json]` | Every saved hook session (one with no events and nothing remembered is skipped) reduced to counts: what the per-session dedup saved, and what a persistent cache would add ([below](#measuring-the-session-cache)) |
| `prompt --workspace DIR [--budget N] [--] TASK...` | Prints the task followed by its context, for clients without hooks (`--budget` defaults to `context_for_task`'s 2500). A task word that starts with `--` needs the `--` before the task; `-h` or `--version` inside a task is task text |
| `doctor --workspace DIR [--json] [--jev-probe [--jev-model M]]` | Checks ripwire, its version and verbs, git history, the state dir and a smoke call; `--jev-probe` also sends one synthetic question to the classifier |
| `install <claude-code\|codex> --workspace DIR [--hooks] [--statusline] [--write] [--online] [--memory]` | Wires the broker into a host (dry run unless `--write`); `--statusline` also registers the Claude Code status line |
| `statusline [--workspace DIR] [--detail] [--width N] [--color never\|always]` | One status line for Claude Code, from the host's stdin and the hooks' projection ([below](#status-line)) |
| `memory status --workspace DIR [--json]` | The workspace's memory: memories, pending observations, generation, sizes, and the category of the error if the store cannot be read |
| `memory forget --workspace DIR (--all \| --id ID)` | Forgets one memory and what derives from it, or everything (which also revokes collection) |
| `memory add --workspace DIR --file PATH` | Adds an explicit note from a JSON file, `{"text": "...", "references": ["src/a.rs"]}` |
| `memory resume --workspace DIR` | Lifts the revocation a full forget leaves on the workspace's memory |
| `memory drain --workspace DIR --online [--jev-model M] [--memory-write-candidates N]` | Incorporates pending observations and enriches ready ones with the classifier, for at most 60 s or 20 jobs; the only `memory` command that uses the network (needs `--features online` and the key). Give it the server's model and K. It fails, instead of reporting nothing to do, when a running server already holds the workspace's worker (also if the server takes it during the drain) or the provider refuses the key |
| `memory retry --workspace DIR` | Gives failed enrichment jobs their runs back, and makes jobs a long `Retry-After` set aside ready now; local |

Every command that keeps state (`serve`, `hook`, `hook-log`, `hook-stats`, `doctor`, `statusline`,
`memory …`) also takes `--state-dir DIR` (for `serve`, where `--memory` keeps its store: give it the
same directory as the hooks that collect for it); without it, `$XDG_STATE_HOME/ripwire-broker` or
`~/.local/state/ripwire-broker` (an empty or relative `XDG_STATE_HOME` counts as unset).

If ripwire is unavailable at startup, the server still comes up in degraded mode. Tools then
return a structured error (`upstream_unavailable` / `incompatible_upstream`), and the next
call tries to reconnect.

## Configuration

There is **no `.env` file** and none is needed. All configuration comes from command-line
arguments; the only secret, the online mode's API key, comes from the environment
([below](#secrets)):

| Argument | Default | Purpose |
| --- | --- | --- |
| `--workspace DIR` | required | Authorized root; canonicalized at startup |
| `--ripwire BIN` | `ripwire` on `PATH` | Path to the ripwire binary |
| `--timeout-ms N` | `60000` | Timeout per ripwire call; a hung process is restarted |
| `--redact-workspace` | off | Hides the workspace path in the status resource |
| `--incremental` | off | Sends unchanged items only once per server process ([below](#incremental-context)) |
| `--ripwire-max-rss-mb N` | no limit | Kills ripwire above N MiB of resident memory; the broker restarts it |
| `--summarizer-cmd CMD` | off | Local model CLI for architectural notes ([below](#architectural-notes-local-model)) |
| `--summarizer-version-cmd CMD` | none | Prints the model's version; its hash invalidates cached notes. It gets 10 s |
| `--summarizer-wait-ms N` | `1500` | Longest an answer waits for a note |
| `--summarizer-timeout-ms N` | `60000` | Hard limit for one generation; the process is killed after it |
| `--online` and `--jev-*` | off | The optional remote classifier ([below](#online-mode-optional)) |
| `--log` | off | With `--online`: every exchange with Jev written to `jev.log` in the state dir, readable ([below](#online-mode-optional)) |
| `--memory` | off | Persistent per-workspace memory; implies `--online`, so it needs a binary built with `--features online` (exit 2 before anything starts without it); without the credential it starts with no request to Jev ([D-155](spec/changelog.md#d-155--sem-chave-chave-recusada-e-o-log-do-jev)). **Experimental** ([PRD](docs/jev-mem-prd.md#4-ativação-e-fronteira-de-consentimento)): it collects, enriches, consolidates and reads memories back in `context_for_task`; the hooks do not deliver them yet, and no host has been validated (T3.11) |
| `--memory-read-deadline-ms N` | `850` | 1–850; longest a task waits for memory ([D-156](spec/changelog.md#d-156--prazos-da-leitura-de-memória-e-recurso-local)) |
| `--memory-read-request-limit N` | `4` | 0–4 classifier requests per read, taken out of `--jev-request-limit` (discovery keeps the rest); 0 serves only the local index |
| `--memory-write-candidates N` | `4` | 0–10 existing memories each new one is compared with |
| `--memory-retention-days N` | `30` | 1–365 |
| `--memory-max-nodes N` | `2000` | 1–2000 memories per workspace |
| `--memory-selection jev\|deterministic` | `jev` | **Experimental, for evaluation** ([D-141](spec/changelog.md#d-141--recorte-da-fase-5-do---memory-braço-determinístico-e-sequências)): `deterministic` keeps the same collection and store but enriches nothing and asks the classifier nothing about memory; a read delivers the local matches (task words and files) whose sources are unchanged, with `basis: deterministic_rank`, no `scores` and `stop_reason: deterministic`. Discovery is unchanged |

### How an MCP host passes configuration

A stdio MCP server is started as a **child process** by the host (Claude Code, Codex...),
from an entry in the host's own config. Only three things reach the process:

1. **Arguments** (`args`): what the broker uses.
2. **Environment variables** (`env` in the same entry), plus whatever the process inherits
   from the host.
3. **Working directory**: this depends on the host, so prefer an absolute `--workspace`
   over `.`.

No host loads a `.env` file automatically. A server would have to read one itself, and it
would then be ambiguous which directory's `.env` applies.

Claude Code (`.mcp.json` in the project, or
`claude mcp add ripwire-broker -e KEY=value -- /path/ripwire-broker --workspace /repo`):

```json
{
  "mcpServers": {
    "ripwire-broker": {
      "command": "/path/to/ripwire-broker",
      "args": ["--workspace", "/path/to/repo"],
      "env": {}
    }
  }
}
```

Codex (`~/.codex/config.toml`):

```toml
[mcp_servers.ripwire-broker]
command = "/path/to/ripwire-broker"
args = ["--workspace", "/path/to/repo"]
env = {}
```

Other hosts, such as Grok-based CLIs, generally use the same `command`/`args`/`env` shape;
check your host's documentation. Hosted APIs that only accept **remote** MCP servers (by
URL) cannot use the broker today: the MVP is stdio only.

### Secrets

The online mode reads its key only from `RIPWIRE_BROKER_JEV_API_KEY` in the server's
environment. It is never accepted as an argument, never written by `install` (which
references the variable by name), and never shown in errors, the status or logs. No process the
broker starts receives it: ripwire, the summarizer, its version command and `git` run without it,
in every build. Set it in the environment the host starts from, not in a committed file.

When Streamable HTTP is added
([roadmap phase 6](spec/ripwire-broker-mcp.md#fase-6--times-e-ci)), its bearer token must also
come from an environment variable ([PRD §7.3](spec/ripwire-broker-mcp.md#73-transporte)).

## MCP surface

| Tool | When to call it | Default budget |
| --- | --- | --- |
| `context_for_task` | before exploring (`task`, `mode`, `include_docs`, `include_bodies`) | 2500 |
| `context_after_edit` | after a relevant edit (`files`, `symbols`) | 1500 |
| `context_before_finish` | before declaring done (`strict`, `include_test_commands`) | 1800 |

The resource `ripwire-broker://status` carries versions, upstream availability, restarts,
default budgets and local metrics. It never includes prompts, code, symbols or responses.
It answers within about a second even when ripwire is occupied (`upstream.busy: true`) or
a reconnect is hanging (`upstream.reconnecting: true`). `inflight` counts the tool calls the
server is tracking for cancellation (counts only).

A client's `notifications/cancelled` stops the tool call: it answers `cancelled`, makes no
further ripwire calls, and shows up in `recent_requests`. The ripwire call already running
is not interrupted, because ripwire does not support that.

Every answer uses the `ripwire-broker.context/v1` envelope: `status`, `intent`, `summary`,
`items[]` (with `role`, `why_included` and `source.verb`), `tests[]`, `risks[]`,
`limitations[]`, `provenance` and `budget`. Repository text always sits under
`content.untrusted_repository_data`.

**Token estimate:** `ceil(bytes of the serialized JSON envelope / 4)`, applied to the
whole answer. The minimum budget is 256; with `--online`, `context_for_task` needs 512.

When `context_for_task` reads a word as a symbol that the repository doesn't have (for
example a host's tool name in the prompt), it explores the task instead and adds the
limitation `symbol_not_found`. An empty `task` is `invalid_input`. `context_after_edit` checks at
most five `symbols`; the rest are named in a `symbols_truncated` limitation. An `item_truncated`
limitation appears only for a body the answer actually shows.

### Incremental context

With `--incremental`, an item the session already received unchanged comes back as a
short reference (path, line and symbol; no body or signature), and repeated tests and
risks are counted in `budget.already_delivered` instead of repeated. A changed item comes
back in full. Limitations, the risks that decide a status, and everything in
`context_before_finish` are never suppressed. Pass `include_seen: true` to
`context_for_task` or `context_after_edit` to get everything again, for example after the
host compacted the conversation.

It is off by default in the server because Claude Code subagents share the parent's MCP
process and would receive references to context they never saw. Hooks always use it,
keyed by the host's `session_id`.

### Architectural notes (local model)

Optional and off by default
([PRD §10.3](spec/ripwire-broker-mcp.md#103-enriquecimento-semântico-opcional-futuro)). With a
local model CLI, `context_for_task` adds up to three `notes`, one per module of the items it
returns:

```sh
ripwire-broker --workspace /repo \
  --summarizer-cmd "ollama run --nowordwrap phi4" \
  --summarizer-version-cmd "ollama show phi4 --modelfile"
```

- **Local:** the command is split on whitespace and run without a shell. The prompt goes to stdin and the note
  comes from stdout. Any CLI works (ollama, llama.cpp, `llm`). The broker opens no network connection itself,
  but `ollama run` talks to its own server on localhost.
- **From the evidence only:** a note is written only from items included in the same answer.
  `derived_from` lists them, `generated: true` and `source.basis: local_model` mark it, and the text sits
  under `untrusted_repository_data`. Terminal escape codes and control characters are removed, and the note
  is capped at 600 characters. The model's answer is read up to 1 MiB, never whole, while the prompt is
  still being written to it.
- **Never blocking:** an answer waits at most `--summarizer-wait-ms`. If the note isn't ready, the answer
  carries a `note_pending` limitation and the note finishes in the background. At most one generation runs at
  a time. A failure becomes `summarizer_unavailable`, and everything else in the answer is unchanged.
- **Cache:** notes are cached in memory by `sha256(prompt version, model id, module, evidence)`. A code change
  gives a new note. So does a model change, once `--summarizer-version-cmd` is set; without it, new weights
  under the same tag keep old notes. The cache lives only as long as the server process, so hooks and
  `prompt` get no notes (D-046).
- **Budget:** notes come after every item; one that doesn't fit becomes `notes_omitted`.
- **ollama:** pass `--nowordwrap`. Without it, `ollama run` word-wraps with terminal redraws even through a
  pipe, which garbles notes. `doctor --summarizer-cmd ...` warns about this and checks the version command;
  it never runs the model (a cold start took 19 s here, and 1 s warm).

## Online mode (optional)

Off unless the server process starts with `--online`
([PRD §23](spec/ripwire-broker-mcp.md#23-adaptador-opcional---online)). A remote semantic classifier
(TypeSafe `jev-1.13.0`) then rates, in `context_for_task`, the files ripwire ranked and their
direct siblings, and the broker merges its probabilities with ripwire's facts. Nothing else
changes: no new tool, and `context_after_edit`, `context_before_finish`, hooks and `prompt`
stay offline.

> O modo online envia previews e trechos elegíveis do workspace ao provider Jev.
> Selecione somente uma raiz cujo conteúdo você tem autorização para enviar.

```sh
cargo build --release --features online      # the default build has no HTTP client (CA-10)
export RIPWIRE_BROKER_JEV_API_KEY=...          # in the host's environment, never in a file
ripwire-broker doctor --workspace /repo --jev-probe   # one synthetic question, no workspace bytes
ripwire-broker install claude-code --workspace /repo --online --write
```

**What leaves the machine:** the task text, paths relative to the workspace, file previews
(16 KiB for files ripwire ranked, 4 KiB for their siblings) and blocks of up to 24 KiB of the
files the classifier admitted. Never the absolute root, the key in any log, or a file the
policy excludes: hidden paths and `.git`, dependency and build directories, anything ignored
by `.gitignore`/`.ignore`, symlinks, binaries, non-UTF-8 files, likely credential files
(`.env*`, `*.env`, SSH keys, `*.pem`, `*.key`, Terraform variables and state, `kubeconfig`,
`.htpasswd`, `.git-credentials`...) and text with a private-key marker. A file is checked on what
was opened, never by name again, so one swapped for a link or a FIFO after the checks is not read. This filtering
reduces risk; it cannot guarantee that every secret is recognized. Choose the root knowingly.

**When it runs:** only on routes that end in ripwire's `explore` (orientation, a change without
a symbol, a symbol the repository lacks). Traces, known symbols, reviews and docs skip it and
say so (`semantic_skipped`). A provider that refuses the key stops the whole discovery: no
later stage sends anything.

**Without a usable key** ([D-155](spec/changelog.md#d-155--sem-chave-chave-recusada-e-o-log-do-jev)):
the server still starts. With `RIPWIRE_BROKER_JEV_API_KEY` unset (or empty) it sets the
process-wide `no_jev_api_key` switch and skips every call to Jev before anything is sent; the answers
are the offline ones, stderr says why and the status line shows `[jev: no key]`. A key with
whitespace inside is never sent either, and shows `[jev: invalid key]`, as does a key Jev refuses
(401/403, expired or badly pasted) until a call succeeds. Memory jobs wait, as for a refused key.

**`--log`** writes every exchange with Jev to `jev.log` in the state dir (the path is printed on
stderr at start): one block per call, separated by rules, with the request (`enviado`) and the
answer (`recebido`, with the HTTP status) as pretty-printed JSON, and the time between sending and
the last byte read (`duração`). The file is private (0600) and only grows; delete it when done. It
holds the source sent to the classifier. The key is never in it: the `Authorization` header is
written as `Bearer [redacted]`, and an echo of the key in an answer is replaced.

```text
════════════════════════════════════════════════════════════════════════
Jev call #1 · 2026-10-06 18:52:01 UTC · discovery
════════════════════════════════════════════════════════════════════════

── enviado ─────────────────────────────────────────────────────────────
POST https://api.typesafe.ai/v1/systemone
Authorization: Bearer [redacted]
Content-Type: application/json

{
  "model": "jev-1.13.0",
  ...
}

── recebido ────────────────────────────────────────────────────────────
HTTP 200

{
  "answers": { ... }
}

── duração ─────────────────────────────────────────────────────────────
812 ms
```

**What the answer gains** (additive to the v1 envelope, absent without `--online`):

- `provenance.online`: `provider`, `model`, `requests`, `cache_hits`, `incomplete` and
  `discovery` (`complete`, `incomplete`, `interrupted` or `skipped`).
- `semantic` on an item: the classifier's `stage`, `state` (`admitted`, `rejected`,
  `selected_source`, `reading_lead`, `excluded`), `probability`, `threshold`, `model`, `lines`,
  `content_hash`, `request_digest` and `cache_hit`. The item keeps its ripwire `source`; a
  rejection is shown, never used to drop a fact.
- `semantic_location` items (`role: semantic`, `source.basis: remote_classifier`) for evidence
  no ripwire symbol matches, including files found beside ripwire's candidates. They never
  carry callers, tests or risks. Only `selected_source` ones carry source.
- Limitations: `semantic_skipped`, `semantic_not_sent`, `semantic_incomplete`,
  `request_too_large`, `semantic_source_capped`. An incomplete discovery means an absence is
  not evidence of irrelevance.

**Limits:** `--jev-max-in-flight 4`, `--jev-request-limit 24` requests per call,
`--jev-timeout-ms 15000` per attempt, `--jev-deadline-ms 8000` for the whole discovery
(`interrupted` after it), `--jev-max-candidates 16`, `--jev-lookahead-max 32` (0 turns the
lookahead off), `--jev-max-source-bytes` (source rendered, not evaluated), `--jev-no-cache`,
`--jev-model` (pinned; `jev-latest` is never a default), `--jev-provider typesafe` (the only
provider). Transient failures are retried by stage, a 429 waits for its `Retry-After` up to 30 s (a longer one is
not waited for, and the discovery ends incomplete), and a client cancel aborts the HTTP requests.
Decisions are cached in memory, keyed by digests only.

**Status:** `online` in `ripwire-broker://status` carries the
[§23.11](spec/ripwire-broker-mcp.md#2311-observabilidade) metrics (requests, latency
percentiles, bytes, retries, 429s, candidates, gain beyond ripwire...), and each call in
`recent_requests` lists its online stages. Counts and times only.

The mode is **experimental** until the A/B evaluation of
[PRD §23.15](spec/ripwire-broker-mcp.md#2315-avaliação-e-barras-de-merge) shows it keeps or
improves correctness.

## Persistent memory (experimental)

`--memory` keeps observations of the workspace between sessions
([PRD](docs/jev-mem-prd.md), [plan](spec/plan/jev-mem-plan.md)). Phases 0 to 5 are built: collection,
enrichment, consolidation, reading in `context_for_task`, and the evaluation arms. It stays
experimental until it is validated in real hosts (T3.11) and measured (T5.3).

**What an automatic memory holds:** which analysis ran (after an edit, before finishing), its
outcome as the broker saw it, the files in scope with the SHA-256 of their bytes, and the names
of the analyses. The text is rendered from those fields (`memory-observation/v1`), for example
`Evento: análise após edição. Escopo: src/cache.rs. Observado pelo broker: análise concluída;
execução de testes desconhecida. …`. The broker never runs the tests, so it never records that
they passed, that a bug was fixed or that a merge is safe.

**When it is collected:** after `context_after_edit` and `context_before_finish` build their
answer, never before and never changing it; `context_for_task` collects nothing, and an answer
the broker could not assess (`unknown`) claims nothing. The answer waits at most 25 ms for the
observation to be durable; past that the write finishes in the background and is counted as
unconfirmed, and no more than four such writes run at once. The status resource gains a
`memory` field with these counts (confirmed, unconfirmed, rejected) only when memory is on. `serve --memory` switches this on.

**From the hooks:** `hook … --memory` collects the same observations after an edit or at
`Stop`. It only writes to the local spool: it never makes an HTTP request, needs no credential
and does not imply `--online`. The observation waits in the spool until a process with
`--memory` incorporates it, so a hook that exits leaves nothing half done. `#ripwire-off` stops
collection for that session only. Measured on a laptop in release, with the real ripwire, the
flag adds about 4–7 ms at p95 to a hook of about 80 ms (the PRD's bar is 10 ms).

**What is never kept:** the prompt, the transcript, a diff, a file's body, shell output or
anything a model generated. A source the online policy refuses (`.env` and other sensitive
names, ignored, hidden, binary, symlinked or outside the root) refuses the whole observation,
and so does a value shaped like a credential (`sk-…`, `AKIA…`, `ghp_…`, a JWT, `password=…`)
or an e-mail address. The scan is conservative, not a promise to catch every secret; a refusal
is counted by reason and never logged with its content.

**Where and how long:** `<state-dir>/memory/<workspace id>/`, outside the repository, 0700 and
0600; two worktrees of one repository never share a store. A memory expires after
`--memory-retention-days` (30); a note derived from memories goes with the first of them to
expire. If the wall clock goes back, nothing expires by age until it catches up again, while the
size caps (2,000 memories, a 1,000-entry spool, 96 MiB in all) keep holding. A store that cannot
be read (a newer schema, a corrupt file, a link, a directory open to others) is left untouched
and memory stays off for that workspace.

**Enrichment:** with `--memory`, `serve` runs a worker that incorporates pending observations,
types each memory (event, fact, procedure, preference) and relates it to at most
`--memory-write-candidates` earlier ones (shared entity, shared words, the nearest one). An
inferred relation needs a probability of at least 0.60, and a pair only counts when every one of
its answers came back; an unknown answer is never read as no. Each job gets four classifier
attempts at most, one retry for a transient failure, and waits for a 429 only inside its 5 s (a longer
`Retry-After` sets the job aside for at most an hour);
401/403 stops the worker until the server restarts. One job per workspace talks to the provider
at a time, memory and discovery share the four requests in flight, and the workspace has a
persisted budget of 1,000 attempts and 20,000 questions per 24 hours, shared with the reads and
kept in its own file (`quota.json`), that a restart or a clock set back does not reset; with it
spent, jobs stay pending and keep their runs. A job runs twice at
most and then waits for `memory retry`. The worker stops with the server; what it did not
finish waits on disk. A stage that starts failing (retention, ingest, enrichment, consolidation),
say over a snapshot that no longer parses, is said once on stderr, not on every tick; so is a
provider that refuses the key.
The status resource's `memory` field adds what the worker cost, by operation (typing,
relations, consolidation): attempts, retries, questions, bytes sent, failures by category and
quota refusals, plus the consolidation rounds run and the derived notes kept or discarded,
counts only. `memory status` shows the attempts and questions spent in the last 24 hours.

**Consolidation:** after 20 enrichments since the last round, or once a pair of memories has
waited 24 hours, the worker (or `memory drain`) runs a round. The counter, the cursor and the
decisions are on disk, so a crash at the 19th enrichment keeps 19; with no process running
nothing is scheduled, and the next `serve --memory` or `memory drain` runs what came due
(`memory drain` only when a whole round's 5 s are left). A round asks the classifier about at
most four pairs of neighbouring memories (each memory with the `--memory-write-candidates` its
write compared it with), twenty questions, within 5 s and four attempts, and starts after the
pairs the previous one asked about. A round that sent nothing stays due; one that could not be
saved is not tried again for ten minutes.
For each pair it records redundancy, contradiction, obsolescence, link usefulness and a
representation (`keep_separate`, `merge`, `promote`, `uncertain`; nothing to do with a Git
merge), only when every answer came back; a link usefulness of 0.60 or more adds a `linked`
relation. No memory is ever deleted or rewritten. With `--summarizer-cmd`, a pair the classifier
wants merged or promoted with probability 0.85 or more, contradicting itself below 0.85, gets a
derived note (`memory-consolidation/v1`): written from both whole memories (2,000 characters at
most, or no note), at most 600 characters, and thrown away when it names a file or an id they
do not (dotfiles, short hashes and abbreviated commits included). The note is a hypothesis about
its parents, keeps them, goes stale with their sources and expires with the first of them. A
summarizer that fails or answers badly leaves the pair separate; each note waits at most
`--summarizer-timeout-ms`, outside the round's 5 s. A memory forgotten during a round is neither
summarized nor recorded. A note is reused from disk only when `--summarizer-version-cmd` pins the model's
weights; `memory drain` makes decisions and links but no notes.

**Reading:** with `--memory`, `context_for_task` reads memory alongside the structural context
and adds at most three memories (600 tokens, a fifth of the budget) in a `memories` field, with
`provenance.memory` saying why the read stopped and what it cost. Memory never fails the tool and
never changes `status`. When it is missing the answer says why: `memory_cold` (a large snapshot
is still loading; the next call finds it warm), `memory_unavailable` (the store cannot be read)
or `memory_incomplete` (the provider failed, time ran out, or the store changed under the read,
in which case none goes out, or the 24-hour budget is spent, in which case no request is sent).
With `--incremental` a memory goes out once per session. The task text is sent to the classifier
and never written to disk. A read takes at most four of `--jev-request-limit`'s requests per call
and discovery the rest (the status shows discovery's share), and what it sends is charged to the
workspace's 24-hour budget. Each request to Jev gets at most 450 ms, counted from when it
leaves: waiting for one of the four requests in flight that discovery shares only spends the
read's deadline. When Jev cannot answer in time and has validated nothing yet, the read delivers
the local matches instead, with `basis: deterministic_rank`, no `scores` and
`provenance.memory.degraded: true`; a refused or broken request still delivers nothing
([D-156](spec/changelog.md#d-156--prazos-da-leitura-de-memória-e-recurso-local)). Only the memories a read is about to use have their sources checked,
off the async threads and inside the deadline, so a large store never holds the structural answer
up. With memory on, a task answer holds back about 150 tokens of its budget for that record, so
memory never pushes it past the budget nor takes the place of an item, with or without notes;
memories only get what is left over. The MCP text block adds a readable section that says the
memories are untrusted history and names each one by id and sources; their text is in the JSON
only, never twice.

**In the hooks:** a hook never reads memory, since it makes no HTTP request, so a hook's context
carries no memories today. The hooks are ready for them: an answer with only memories counts as
content, a memory the session was not told is news, and the injected context gets the same
readable section as the MCP text block when it fits the host's 9,000 characters (the memories
stay in the JSON either way). `#ripwire-off` stops injection for that session only.

**Hosts: not validated.** Whether Claude Code and Codex actually use the `memories` field or the
readable section has not been checked in a real session yet
([plan](spec/plan/jev-mem-plan.md), T3.11); until it is, neither host counts as consuming memory.

**Forgetting:** forgetting a memory removes it, every note derived from it and its pending
copies, in a new generation, and keeps its id from coming back for the retention period, even
from an old pending copy. No backup with its text is kept: forgetting and expiry also remove the
memory's job lock and any temporary snapshot a crashed writer left. It cannot reach copies the operating
system made (swap, snapshots, backups of the disk) nor anything already sent to the provider.
Forgetting everything also revokes collection for the workspace: a `revoked` marker in the store
wins over `--memory`, across restarts, until `ripwire-broker memory resume --workspace DIR`
removes it. What was forgotten stays forgotten after resuming.

**Commands:** `memory status`, `forget`, `add`, `retry` and `resume` ([table above](#build-and-run)) are
local: they never start ripwire, open a connection or need the credential or the `online`
feature. `memory drain --online` is the one that talks to the provider.
`memory add` is the only way a preference or a free-text note gets in; the broker never infers
one from an edit. The note's text is untrusted data, kept verbatim and never followed as an
instruction; the input file and the text pass the same filters as an observation. A forgotten id
stays blocked for 365 days, the longest retention allowed, since `forget` cannot know the
retention the server runs with.
`doctor` adds a `memory` line when the workspace has a store: its size, or a warning when it is revoked
or cannot be read. It reads the store locally and never uses the network.

## Automatic mode (hooks)

MCP alone only offers tools; the agent still has to call them. Hooks make it automatic
([PRD §8.4](spec/ripwire-broker-mcp.md#84-automação-no-host)). Claude Code and Codex share the
hook contract, so the same command serves both:

| Host event | What the broker does |
| --- | --- |
| `UserPromptSubmit` | First prompt of the session: `context_for_task` becomes `additionalContext`. Later prompts only with `--every-prompt` |
| `PostToolUse` (`Edit`/`Write`/`MultiEdit`/`NotebookEdit`/`Bash` on Claude Code, `apply_patch` on Codex) | `context_after_edit` for the edited files inside the workspace; nothing if there is nothing new. A `Bash` command counts only if it changed files: as Claude Code reports them, or else as a git fingerprint finds them (see below) |
| `Stop` | `context_before_finish`. With `--gate`, blocks once on `attention_required` and sends the evidence; otherwise a one-line notice |

- **Shell edits (Claude Code):** when Claude Code reports the changed files itself (`tool_response.bashEditDiff`,
  seen in 2.1.285), that list is used, in any workspace, and from the first such payload on the session runs no
  `git` at all (D-131). Otherwise, as a fallback in git workspaces, before starting ripwire the hook compares a fingerprint of the
  dirty files (`git status`, plus each file's mtime and size) with the one from the previous hook. A `Bash`
  command that changed files gets `context_after_edit` for them (at most 50); a read-only one starts nothing and
  counts no event. Only changed files inside the workspace count, even when the workspace is a subdirectory of
  the repository. Without the host's list, outside git it stays silent. `git` gets 500 ms; if it is slower, if `git status` lists more
  than 5,000 entries, or if two fingerprints in a row take over 50 ms (a Linux-sized tree takes ~240 ms),
  detection switches off for the rest of the session (no more `git` calls, and `Bash` edits are left to the
  `Stop` gate); a new session tries again. A change made by another process while the
  command ran is blamed on the command (the fallback only; the host's list is exact). Codex is unchanged.
- **Bursts of edits:** an edit within `--edit-interval-ms` (default 1000) of the previous answer asks nothing;
  its files are held and ride along with the next edit past the window, so news is delayed by one edit at most.
  Only the first 32 distinct files of a burst are held; the rest are not forwarded, and `Stop` covers them and
  the tail of the turn. `--edit-interval-ms 0` answers every edit (D-106).
- **Budgets:** the hooks ask for 1500 tokens at the prompt and 800 after an edit.
- **D-129:** edits made through the Bash tool now reach the edit hook in git workspaces. Re-run
  `ripwire-broker install claude-code --workspace DIR --hooks --write` to add `Bash` to the
  `PostToolUse` matcher; older installs keep working without it.
- **Opt-out:** type `#ripwire-off` in a prompt to silence the session, and `#ripwire-on` to resume.
  The marker counts only as the whole last (or first) word of the prompt; one quoted mid-text does nothing.
- **What was injected:** every injection shows a one-line `systemMessage`, and the full context sits in the
  host's transcript. `ripwire-broker hook-log --session ID` lists the last 5 injections with counts. Paths and
  symbols appear there only if the hook ran with `--log-refs`.
- **Never in the way:** a hook always exits 0. A broker failure becomes a notice, never a block. Injected
  context stays under 9,000 characters, because both hosts show only a preview beyond ~10,000.
- **State:** one private file per session (`0600`, named by the sha256 of the session id) in `--state-dir`,
  by default `$XDG_STATE_HOME/ripwire-broker` or `~/.local/state/ripwire-broker` (an empty or relative
  `XDG_STATE_HOME` counts as unset, so state never lands in the workspace). It holds fingerprints and
  counts, never prompts or code. In a git workspace it can also hold the working-tree fingerprint: up to 5,000
  entries, each an absolute path anywhere in the repository with its mtime and size. The first event of a
  new session removes the files of sessions nobody touched for 30 days.
- **Cost:** a hook starts ripwire only for an event that asks it something (about 0.1–0.5 s on a small
  repository): not for a prompt after the first one (without `--every-prompt`), an event of a paused session,
  an edit held back by the coalescing window, or an edit outside the workspace. `doctor` shows the timing of a
  smoke call.

### Measuring the session cache

Each hook event is a new process, so the broker's own `session_hits` dies with it. The session file keeps a
running tally instead (events, injections, items delivered whole, items not resent), and
`ripwire-broker hook-stats` adds every saved session up (a session with no events and no fingerprints, one
whose every event failed to launch ripwire, is skipped):

- **within a session:** what the per-session dedup already saves (`hit_rate`);
- **across sessions:** how many fingerprints a session received that an earlier session had already received.
  That is what a persistent cache would add, and it is the measurement
  [PRD §21.3](spec/ripwire-broker-mcp.md#213-cache-próprio) waits for before building one.

Counts only: no path, symbol, prompt, fingerprint or session id. Sessions saved before this tally existed
count as zero events but still contribute their fingerprints.

## Status line

Claude Code can run a command to draw its status bar. `ripwire-broker statusline` reads the JSON the host
sends on stdin, adds what the hooks last published for the session and what a live online `serve` of the
workspace published, and prints **one line**
([PRD §24](spec/ripwire-broker-mcp.md#24-barra-de-status-do-claude-code)). It never starts ripwire, never
connects to the broker and never creates a file.

```text
rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks on · última: atenção · inj 7 · não reenviados 18
rw-brkr · Opus 4.6 max · ctx 71% · hooks off · inj 7 · não reenviados 18
rw-brkr · Sonnet 4.6 mid · ctx 12% · hooks sem dados
rw-brkr · Sonnet 4.6 low · ctx 45% · hooks on · última: erro
rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks on · inj 7 · não reenviados 18 · [jev:3] · [mem: retr 1, stor 2] · (online)
```

| Segment | Meaning |
| --- | --- |
| `rw-brkr` | Fixed prefix (the executable is still `ripwire-broker`) |
| `Sonnet 4.6 hig` | Model name, version and effort (`low`, `mid`, `hig`, `xtr`, `max`) from the host; any other effort is omitted |
| `ctx 32%` | Context window used, from the host. With `--color always` it is grey below 40, white below 60, yellow up to 80, red above |
| `hooks on` / `hooks off` | `off` means the automatic context is paused (`#ripwire-off`); MCP is not affected. With `--color always`, `off` is red and `on` light blue |
| `última: pronta\|atenção\|incerta\|erro` | Outcome of the last analysis, shown as "last", never as current health. `pronta` does not certify the code |
| `inj 7` | Injections and blocks the hooks counted in this session |
| `não reenviados 18` | Logical items not resent because the session already had them (not tokens, not Anthropic prompt-cache hits) |
| `hooks sem dados` | No projection for this session |
| `[jev:3]` | With a live `serve --online`: Jev requests it sent in the last five seconds, refused ones included |
| `[jev: no key]` / `[jev: invalid key]` | In place of `[jev:N]`, red when colored, kept when the line is narrow: the key is unset (nothing is sent), or malformed or refused by Jev |
| `[mem: retr 1, stor 2]` | With a live `serve --memory`: memory reads by `context_for_task` and observations written by the tools, in the last five seconds |
| `(online)` | A live `serve --online` (or `--memory`) runs for this workspace; always the last segment |

- **The server segments** come from a small file each online `serve` keeps in the `statusline/` directory
  of its state dir (counts only), rewritten within a second of new activity and every 10 s otherwise, and
  removed when the server stops. A file not refreshed for 30 s is ignored, so a crashed server drops off the
  bar. Several servers of one workspace add up. The bar only redraws on host events: for a live count while
  idle, set `refreshInterval` in `statusLine` yourself. Observations written by the hooks are not in
  `stor`: hooks are separate processes. The server needs the same `--state-dir` as the bar (both default
  to the standard one).
- **`hooks sem dados` does not prove the hooks are uninstalled.** It can be a new session, a failed write or a
  session that only uses MCP.
- **`--detail`** adds, if they fit, `entregues N`, `reuso 42%`, `último contexto ~1,2k tok há 5min` (when the
  last context reached the model) and `visto há 20s` (when a hook last wrote the snapshot; `dados antigos`
  after five minutes).
- **`--width N`**, then `COLUMNS`, then 100 columns. When the line is too wide, details go first, then the
  counters (`[jev:N]` and `[mem: …]` among them), then the model, then the soft segments (`(online)` among
  them); the prefix, `ctx`, `hooks off` and an
  `atenção`/`erro` alert are kept. **`--color never`** is the default; `--color always` emits ANSI even
  without a TTY and even with `NO_COLOR`.
- **Install:** `ripwire-broker install claude-code --workspace DIR --hooks --statusline --write` writes the
  hooks and `statusLine` into the same `.claude/settings.json` change. A bar that is not ours is never
  overwritten: one in your user settings is left to win (the install prints the snippet to add by hand), one in
  `settings.local.json` is reported because it takes precedence. Without `--hooks` the bar has no counters to
  show. A bar is recognized as the broker's only when the program is named exactly `ripwire-broker` or
  `ripwire-broker-<digit>…` (a versioned copy such as `ripwire-broker-0.2`) and its first argument is
  `statusline`; `ripwire-brokerage` or a wrapper script is someone else's bar and is kept.
  The "no counters" note is skipped when the project settings already hold the broker's hooks.
- **Privacy:** the projection holds counts and a few enums only: no prompt, code, path, symbol or fingerprint.
  The host's stdin is read (up to 256 KiB) and never stored. The file is private (`0600`, in a `0700`
  `statusline/` directory of the state dir) and written atomically by the hooks, after the session state.
- **Migration:** the bar's counters start at the first projection bound to the workspace, so a session that
  began before the bar existed shows zero, while `hook-stats` keeps counting everything (except sessions that only ever failed to launch).
- **Hand-written configs** must pass the same `--workspace` to the hooks and to `statusline`: without it the
  hooks fall back to the event's `cwd` and the bar to `workspace.project_dir`, and they differ if the working
  directory changes mid-session (`install` always writes the same one to both).
- **`--agent`:** when the main session runs with `--agent`, the host sends an `agent` object; the bar shows
  the usual segments, hook data included, plus `agente: <name>` right after the model (at most 24 columns).
  Subagents have their own `subagentStatusLine`, which this bar does not configure.
- **Shadowing:** if your user settings gain a `statusLine` of their own, a re-run of
  `install --statusline` removes the broker's bar from the project settings (and says so) so it does not
  shadow yours.
- Only the Claude Code hooks publish. Cost: one process per refresh, a few milliseconds
  (measured in [D-123](spec/changelog.md#d-123--a-barra-de-status-é-implementada)).

## Agent integration

The simplest way is `install`, which is a **dry run** unless you pass `--write`:

```sh
ripwire-broker doctor --workspace /repo
ripwire-broker install claude-code --workspace /repo --hooks          # shows the plan
ripwire-broker install claude-code --workspace /repo --hooks --write  # writes it
ripwire-broker install codex --workspace /repo --hooks --write
```

- Claude Code: merges the server into `/repo/.mcp.json` and, with `--hooks`, the hooks into
  `/repo/.claude/settings.json`. These are the only files the broker ever writes inside a workspace, and only
  with `--write`.
- Codex: merges the hooks into `~/.codex/hooks.json` (`--codex-home` to change it). They are global, so they
  follow each session's `cwd` rather than a fixed workspace. It **prints** the `config.toml` lines to add
  (`[mcp_servers.ripwire-broker]`, and `[features] hooks = true`), and never edits TOML.
- `--online` adds the flag to the server and references the key by name: `${RIPWIRE_BROKER_JEV_API_KEY}` in
  `.mcp.json`, `env_vars = ["RIPWIRE_BROKER_JEV_API_KEY"]` in the Codex snippet. Hooks stay offline.
- `--memory` adds `--memory` to the server, without a redundant `--online`, and references the key the
  same way; with `--hooks`, the hook commands get `--memory` too, which only writes the local spool. The
  preview names both effects: memories kept on this machine and eligible ones sent to the provider.
  Reinstalling without it takes the flag off everywhere.
- Hook commands quote every path for the host's shell, so a directory name cannot run as code.
- Merges keep your other keys and hooks, are idempotent, and back up a changed file once as `<name>.bak`.
  JSON key order is normalized.

Manual setup, if you prefer:

- Claude Code: copy `integrations/claude-code/skills/ripwire-broker/` to `.claude/skills/`
  and register the server (example in `integrations/claude-code/mcp.json`, or
  `claude mcp add ripwire-broker -- /path/ripwire-broker --workspace .`).
- Codex: add `integrations/codex/config.toml` to `~/.codex/config.toml` and the contents of
  `integrations/codex/AGENTS.md` to the repository's `AGENTS.md`.
- Hooks: examples in `integrations/claude-code/settings.json` and `integrations/codex/hooks.json`.
- Other clients: `client "$(ripwire-broker prompt --workspace /repo "the task")"` sends the task
  followed by its context inside `<ripwire-broker-context untrusted="true">`. `<` and `>` in the
  payload are escaped (`\u003c`), so repository text cannot close that block.

## A/B evaluation

`ripwire-eval` is the instrument for [PRD §16.2](spec/ripwire-broker-mcp.md#162-avaliação-ab),
[§17](spec/ripwire-broker-mcp.md#17-critérios-de-sucesso) and
[§23.15](spec/ripwire-broker-mcp.md#2315-avaliação-e-barras-de-merge). It is a second binary and adds
nothing to the broker's own command line. Plan and decisions in
[spec/plan/plano-ab-e-session-hits.md](spec/plan/plano-ab-e-session-hits.md).

```sh
cargo build --release
./target/release/ripwire-eval check    --corpus corpus.json
./target/release/ripwire-eval validate --corpus corpus.json
./target/release/ripwire-eval run      --corpus corpus.json --out ab/ [--arms none,ripwire,broker] [--repeats 3]
./target/release/ripwire-eval report   --out ab/ [--json]
```

- **Corpus:** JSON, `{"tasks": [{"id", "repo", "base", "fix", "prompt", "vocabulary_diverges", "reference":
  {"files", "tests"}, "check", "setup", "teardown", "env", "sequence"}]}`. `id` names the run's files, so it takes letters, digits, `.`, `_` and `-`
  only, and does not start with `.`. `repo` is a local git repository (relative to
  the corpus file), `base` the commit the agent starts from, `fix` the reference commit, `reference.files` what
  it modifies, and `check` a shell command whose exit 0 means the task was solved. `setup` prepares the copy
  (dependencies, build caches) and is not counted as the agent's edit; `teardown` cleans up after it; `env`
  applies to all of them and to the agent. Commands take `{repo}`, `{fix}` and `{run}`, a per-run id safe
  for a database name, each already one shell word (quoted when the path needs it, so leave them unquoted);
  `env` values take `{run}` only, since the agent would see the others. Tasks taken from real commits get their reference for free, and their
  tests become hidden tests: `git -C {repo} show {fix}:test/x_test.exs > test/x_test.exs && mix test test/x_test.exs`.
  Tasks with the same `sequence` are sessions of one history, for the memory arms: they run in corpus order,
  each from its own `base`, in the same place and with the same memory store for a given arm and repeat; a
  sequence is recorded, and resumed, as a whole (one recorded in part stops the run, with the lines to
  remove), stays in one repository, and, in a memory arm, marks a session after an invalid one
  `history_incomplete`, which the report lists and keeps out of the averages. Each arm's history is its own earlier sessions: arms B and C differ in history as well as in selection, and a
  memory whose source the agent edited is stale in the next session unless its base has the same bytes.
- **Validation:** `validate` runs each task's check on a copy at the base, where it must fail, and on one at
  the fix, where it must pass. A check that passes at the base measures nothing. The output of every setup
  and check goes to `validate-logs/` next to the corpus; a run's check output goes next to its transcript.
- **Run on an idle machine.** A check run under heavy load can fail for reasons that have nothing to do with
  the agent (tests with short timing windows), and then counts as a wrong answer for that arm. Validate again
  just before a paid run.
- **Pin what the suite reads from the machine.** A check taken from an old commit ages: its tests may read
  today's date, or bind a fixed port that a second suite on the same machine already holds. Put a frozen clock
  (the day the change was written, not the merge date) and a port of its own in the task's `env`, when the
  suite offers them (D-121).
- **Arms:** `none`, `ripwire` (ripwire's MCP directly), `broker`, `broker-online`, and for the memory
  evaluation ([PRD](docs/jev-mem-prd.md#14-observabilidade-avaliação-e-critérios-de-aceitação)) `broker-memory`
  (`serve --memory`, arm B) and `broker-memory-deterministic` (`--memory-selection deterministic`, arm C);
  `broker-online` is arm A. Those three need `RIPWIRE_BROKER_JEV_API_KEY` and send eligible source of the
  corpus repositories to the provider. A memory arm's server keeps its store in a directory of the round's
  own (`XDG_STATE_HOME`), removed with the round, so no round sees another's memories or yours.
- **Isolation:** each run gets a fresh repository holding the base and its ancestors only. The fix, a later
  commit, cannot leak through `git log`, and the source repository is never written to. The default agent is
  Claude Code headless with `--strict-mcp-config --setting-sources local`: neither your own settings, hooks
  and plugins nor the ones a repository commits load. A run whose session shows a hook that ran, an MCP
  server the arm did not declare, or a context tool (`graft`, `ripwire`) run from the shell outside its arm,
  is recorded as invalid and left out of the averages; so is a session its API cut short
  (`terminal_reason: "api_error"`), which says nothing about the arm.
- **Output:** `results.jsonl` (counts and scores; an interrupted run resumes where it stopped),
  `versions.json` (the broker's and ripwire's versions, the pinned classifier model, the summarizer; the
  agent's version and model come from each session's transcript; a resume with other binaries is
  refused), and `transcripts/`, which holds the
  agent's full session, repository code included. Keep it local.
- **Memory cost:** a memory arm's run records, apart, what its reads sent and delivered (from
  `provenance.memory` in the answers), how long the agent waited for `context_for_task`, what the round's
  store spent of its 24-hour quota during the session besides the reads (the worker's enrichment and
  consolidation, read with `memory status`), the observations and jobs it left for the next session (the
  worker dies with the session, so the next one pays for them), and the agent's own duration. Other arms
  have none of these fields, never zeros; arm A sits beside B and C for its `context_for_task` wait. The report's "Custo da memória" table averages them; questions are not turned into
  dollars without verified pricing.
- **Bars:** each one reads `passa`, `falha` or `insuficiente`. A bar compares two arms over the (task, repeat)
  pairs valid in both, so a task one arm failed to run weighs on neither side; it stays `insuficiente` below 30
  such tasks in 3 repositories. A task whose reference names no tests has no test recall, rather than a full one.
- **Binaries and timeouts:** `--broker BIN` defaults to the `ripwire-broker` next to `ripwire-eval` (then
  `PATH`), `--ripwire BIN` to `ripwire` on `PATH`. `--timeout-s` caps one agent run (default 1800) and
  `--check-timeout-s` one setup or check (default 600, also for `validate`).
- **Cost:** every run is a paid agent session. `--agent-cmd` replaces the whole agent command (split on
  whitespace, never through a shell); to cap each Claude Code run, pass the default command with
  `--max-budget-usd N` added. `ripwire-eval --help` prints the default.

## Tests

```sh
cargo test                 # core, hooks, notes, CLI and the A/B instrument (fixtures), MCP e2e and upstream against the real ripwire
cargo test --features online   # also the HTTP client against a local fixture server
RIPWIRE_BROKER_TEST_MODEL="ollama run --nowordwrap phi4" cargo test -- --ignored   # a real local model
RIPWIRE_BROKER_JEV_API_KEY=... cargo test --features online --test online_live -- --ignored   # the real classifier, synthetic content only
cargo run --release --example spike -- /path/to/repo "task"   # Phase 0 measurements
```

The e2e and upstream tests are skipped when `ripwire` is not on `PATH`. Fixtures in
`tests/fixtures/ripwire/` were recorded from ripwire 0.6.4. Fixtures in `tests/fixtures/hooks/` are
real hook payloads from Claude Code 2.1.283 and Codex 0.157.1; the two `Bash` ones come from Claude Code
2.1.285 (D-129, D-131), as does the status line payload in `tests/fixtures/statusline/` (D-128). Paths
in them are placeholders (`__WORKSPACE__`, `__TRANSCRIPT__`, `__SCRATCHPAD__`). `tests/fixtures/eval/` holds a
synthetic `stream-json` transcript. `tests/fixtures/jev/` keeps the digests and probabilities of a live
exchange with `jev-1.13.0`, never source.

## License

[MIT](LICENSE) © 2026 Antonio Quental
