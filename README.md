# ripwire-broker

A local MCP server that turns [Ripwire](https://github.com/redhat-et/ripwire)'s wide surface
(33 verbs) into three task-moment tools with a token budget, deduplication, provenance and
preserved limitations. It is local, offline and read-only. Specification:
[`spec/ripwire-broker-prd.md`](spec/ripwire-broker-prd.md). Decision log:
[`spec/changelog.md`](spec/changelog.md).

```text
agent ⇄ stdio ⇄ ripwire-broker ⇄ stdio ⇄ ripwire <workspace> --mcp
```

## Requirements

- Rust 1.98.1 (pinned in `rust-toolchain.toml`)
- `ripwire` ≥ 0.6.4 on `PATH` (or pass `--ripwire BIN`)

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
| `prompt --workspace DIR TASK...` | Prints the task followed by its context, for clients without hooks |
| `doctor --workspace DIR` | Checks ripwire, its version and verbs, git history, the state dir and a smoke call |
| `install <claude-code\|codex> --workspace DIR [--hooks] [--write]` | Wires the broker into a host (dry run unless `--write`) |

If ripwire is unavailable at startup, the server still comes up in degraded mode. Tools then
return a structured error (`upstream_unavailable` / `incompatible_upstream`), and the next
call tries to reconnect.

## Configuration

There is **no `.env` file** and none is needed: the broker is local and offline and uses no
secrets. All configuration comes from command-line arguments:

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

### Secrets (future)

When Streamable HTTP is added (roadmap phase 4), its bearer token must come from an
environment variable set in the host entry (PRD §7.3). It must never come from an
argument or a committed file.

## MCP surface

| Tool | When to call it | Default budget |
| --- | --- | --- |
| `context_for_task` | before exploring (`task`, `mode`, `include_docs`, `include_bodies`) | 2500 |
| `context_after_edit` | after a relevant edit (`files`, `symbols`) | 1500 |
| `context_before_finish` | before declaring done (`strict`, `include_test_commands`) | 1800 |

The resource `ripwire-broker://status` carries versions, upstream availability, restarts,
default budgets and local metrics. It never includes prompts, code, symbols or responses.
It answers within about a second even when ripwire is occupied (`upstream.busy: true`).

A client's `notifications/cancelled` stops the tool call: it answers `cancelled`, makes no
further ripwire calls, and shows up in `recent_requests`. The ripwire call already running
is not interrupted, because ripwire does not support that.

Every answer uses the `ripwire-broker.context/v1` envelope: `status`, `intent`, `summary`,
`items[]` (with `role`, `why_included` and `source.verb`), `tests[]`, `risks[]`,
`limitations[]`, `provenance` and `budget`. Repository text always sits under
`content.untrusted_repository_data`.

**Token estimate:** `ceil(bytes of the serialized JSON envelope / 4)`, applied to the
whole answer. The minimum budget is 256.

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

Optional and off by default (PRD §10.3). With a local model CLI, `context_for_task` adds up to
three `notes`, one per module of the items it returns:

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

## Automatic mode (hooks)

MCP alone only offers tools; the agent still has to call them. Hooks make it automatic
(PRD §8.4). Claude Code and Codex share the hook contract, so the same command serves both:

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
  followed by its context inside `<ripwire-broker-context untrusted="true">`.

## Tests

```sh
cargo test                 # core, hooks, notes and CLI (fixtures), MCP e2e and upstream against the real ripwire
RIPWIRE_BROKER_TEST_MODEL="ollama run --nowordwrap phi4" cargo test -- --ignored   # a real local model
cargo run --release --example spike -- /path/to/repo "task"   # Phase 0 measurements
```

The e2e and upstream tests are skipped when `ripwire` is not on `PATH`. Fixtures in
`tests/fixtures/ripwire/` were recorded from ripwire 0.6.4. Fixtures in `tests/fixtures/hooks/` are
real hook payloads from Claude Code 2.1.283 and Codex 0.157.1.
