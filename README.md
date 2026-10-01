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
| `hook-stats [--json]` | Every saved hook session reduced to counts: what the per-session dedup saved, and what a persistent cache would add ([below](#measuring-the-session-cache)) |
| `prompt --workspace DIR TASK...` | Prints the task followed by its context, for clients without hooks |
| `doctor --workspace DIR [--jev-probe]` | Checks ripwire, its version and verbs, git history, the state dir and a smoke call; `--jev-probe` also sends one synthetic question to the classifier |
| `install <claude-code\|codex> --workspace DIR [--hooks] [--statusline] [--write] [--online]` | Wires the broker into a host (dry run unless `--write`); `--statusline` also registers the Claude Code status line |
| `statusline [--workspace DIR] [--detail] [--width N] [--color never\|always]` | One status line for Claude Code, from the host's stdin and the hooks' projection ([below](#status-line)) |

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
| `--summarizer-version-cmd CMD` | none | Prints the model's version; its hash invalidates cached notes |
| `--summarizer-wait-ms N` | `1500` | Longest an answer waits for a note |
| `--summarizer-timeout-ms N` | `60000` | Hard limit for one generation; the process is killed after it |
| `--online` and `--jev-*` | off | The optional remote classifier ([below](#online-mode-optional)) |

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
references the variable by name), and never shown in errors, the status or logs. Set it in the
environment the host starts from, not in a committed file.

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
limitation `symbol_not_found`.

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
  is capped at 600 characters.
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
(`.env*`, `*.env`, SSH keys, `*.pem`, `*.key`...) and text with a private-key marker. This filtering
reduces risk; it cannot guarantee that every secret is recognized. Choose the root knowingly.

**When it runs:** only on routes that end in ripwire's `explore` (orientation, a change without
a symbol, a symbol the repository lacks). Traces, known symbols, reviews and docs skip it and
say so (`semantic_skipped`).

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
`--jev-model` (pinned; `jev-latest` is never a default). Transient failures are retried by
stage, a 429 waits for its `Retry-After`, and a client cancel aborts the HTTP requests.
Decisions are cached in memory, keyed by digests only.

**Status:** `online` in `ripwire-broker://status` carries the
[§23.11](spec/ripwire-broker-mcp.md#2311-observabilidade) metrics (requests, latency
percentiles, bytes, retries, 429s, candidates, gain beyond ripwire...), and each call in
`recent_requests` lists its online stages. Counts and times only.

The mode is **experimental** until the A/B evaluation of
[PRD §23.15](spec/ripwire-broker-mcp.md#2315-avaliação-e-barras-de-merge) shows it keeps or
improves correctness.

## Automatic mode (hooks)

MCP alone only offers tools; the agent still has to call them. Hooks make it automatic
([PRD §8.4](spec/ripwire-broker-mcp.md#84-automação-no-host)). Claude Code and Codex share the
hook contract, so the same command serves both:

| Host event | What the broker does |
| --- | --- |
| `UserPromptSubmit` | First prompt of the session: `context_for_task` becomes `additionalContext`. Later prompts only with `--every-prompt` |
| `PostToolUse` (`Edit`/`Write`/`MultiEdit`, Codex `apply_patch`) | `context_after_edit` for the edited files inside the workspace; nothing if there is nothing new |
| `Stop` | `context_before_finish`. With `--gate`, blocks once on `attention_required` and sends the evidence; otherwise a one-line notice |

- **Opt-out:** type `#ripwire-off` in a prompt to silence the session, and `#ripwire-on` to resume.
- **What was injected:** every injection shows a one-line `systemMessage`, and the full context sits in the
  host's transcript. `ripwire-broker hook-log --session ID` lists the last 5 injections with counts. Paths and
  symbols appear there only if the hook ran with `--log-refs`.
- **Never in the way:** a hook always exits 0. A broker failure becomes a notice, never a block. Injected
  context stays under 9,000 characters, because both hosts show only a preview beyond ~10,000.
- **State:** one private file per session (`0600`, named by the sha256 of the session id) in `--state-dir`,
  by default `$XDG_STATE_HOME/ripwire-broker` or `~/.local/state/ripwire-broker`. It holds fingerprints and
  counts, never prompts or code.
- **Cost:** each hook starts ripwire for one event (about 0.1–0.5 s on a small repository). `doctor` shows the
  timing of a smoke call.

### Measuring the session cache

Each hook event is a new process, so the broker's own `session_hits` dies with it. The session file keeps a
running tally instead (events, injections, items delivered whole, items not resent), and
`ripwire-broker hook-stats` adds every saved session up:

- **within a session:** what the per-session dedup already saves (`hit_rate`);
- **across sessions:** how many fingerprints a session received that an earlier session had already received.
  That is what a persistent cache would add, and it is the measurement
  [PRD §21.3](spec/ripwire-broker-mcp.md#213-cache-próprio) waits for before building one.

Counts only: no path, symbol, prompt, fingerprint or session id. Sessions saved before this tally existed
count as zero events but still contribute their fingerprints.

## Status line

Claude Code can run a command to draw its status bar. `ripwire-broker statusline` reads the JSON the host
sends on stdin, adds what the hooks last published for the session, and prints **one line**
([PRD §24](spec/ripwire-broker-mcp.md#24-barra-de-status-do-claude-code)). It never starts ripwire, never
connects to the broker and never creates a file.

```text
rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks on · última: atenção · inj 7 · não reenviados 18
rw-brkr · Opus 4.6 max · ctx 71% · hooks off · inj 7 · não reenviados 18
rw-brkr · Sonnet 4.6 mid · ctx 12% · hooks sem dados
rw-brkr · Sonnet 4.6 low · ctx 45% · hooks on · última: erro
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

- **`hooks sem dados` does not prove the hooks are uninstalled.** It can be a new session, a failed write or a
  session that only uses MCP.
- **`--detail`** adds, if they fit, `entregues N`, `reuso 42%`, `último contexto ~1,2k tok` and `há 20s`
  (`dados antigos` after five minutes).
- **`--width N`**, then `COLUMNS`, then 100 columns. When the line is too wide, details go first, then the
  counters, then the model, then the soft hook segments; the prefix, `ctx`, `hooks off` and an
  `atenção`/`erro` alert are kept. **`--color never`** is the default; `--color always` emits ANSI even
  without a TTY and even with `NO_COLOR`.
- **Install:** `ripwire-broker install claude-code --workspace DIR --hooks --statusline --write` writes the
  hooks and `statusLine` into the same `.claude/settings.json` change. A bar that is not ours is never
  overwritten: one in your user settings is left to win (the install prints the snippet to add by hand), one in
  `settings.local.json` is reported because it takes precedence. Without `--hooks` the bar has no counters to
  show.
- **Privacy:** the projection holds counts and a few enums only: no prompt, code, path, symbol or fingerprint.
  The host's stdin is read (up to 256 KiB) and never stored. The file is private (`0600`, in a `0700`
  `statusline/` directory of the state dir) and written atomically by the hooks, after the session state.
- **Migration:** the bar's counters start at the first projection bound to the workspace, so a session that
  began before the bar existed shows zero, while `hook-stats` keeps counting everything.
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
  {"files", "tests"}, "check", "setup", "teardown", "env"}]}`. `repo` is a local git repository (relative to
  the corpus file), `base` the commit the agent starts from, `fix` the reference commit, `reference.files` what
  it modifies, and `check` a shell command whose exit 0 means the task was solved. `setup` prepares the copy
  (dependencies, build caches) and is not counted as the agent's edit; `teardown` cleans up after it; `env`
  applies to all of them and to the agent. Commands and `env` values take `{repo}`, `{fix}` and `{run}`, a
  per-run id safe for a database name. Tasks taken from real commits get their reference for free, and their
  tests become hidden tests: `git -C {repo} show {fix}:test/x_test.exs > test/x_test.exs && mix test test/x_test.exs`.
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
- **Arms:** `none`, `ripwire` (ripwire's MCP directly), `broker`, `broker-online`. The online arm needs
  `RIPWIRE_BROKER_JEV_API_KEY` and sends eligible source of the corpus repositories to the provider.
- **Isolation:** each run gets a fresh repository holding the base and its ancestors only. The fix, a later
  commit, cannot leak through `git log`, and the source repository is never written to. The default agent is
  Claude Code headless with `--strict-mcp-config --setting-sources local`: neither your own settings, hooks
  and plugins nor the ones a repository commits load. A run whose session shows a hook that ran, an MCP
  server the arm did not declare, or a context tool (`graft`, `ripwire`) run from the shell outside its arm,
  is recorded as invalid and left out of the averages.
- **Output:** `results.jsonl` (counts and scores; an interrupted run resumes where it stopped), and
  `transcripts/`, which holds the agent's full session, repository code included. Keep it local.
- **Bars:** each one reads `passa`, `falha` or `insuficiente`. They stay `insuficiente` below 30 tasks in 3
  repositories.
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
real hook payloads from Claude Code 2.1.283 and Codex 0.157.1. `tests/fixtures/jev/` keeps the digests and
probabilities of a live exchange with `jev-1.13.0`, never source.

## License

[MIT](LICENSE) © 2026 Antonio Quental
