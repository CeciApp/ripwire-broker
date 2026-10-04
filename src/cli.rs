//! Command line (D-028): `serve` (the default, also without a subcommand), `hook`, `hook-log`,
//! `hook-stats`, `prompt`, `doctor`, `install` and `statusline`. Parsing is pure; nothing here
//! touches the disk.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Never,
    Always,
}

pub const USAGE: &str = "\
usage: ripwire-broker [serve] --workspace DIR [--ripwire BIN] [--timeout-ms N] [--redact-workspace] [--incremental]
                      [--ripwire-max-rss-mb N]
                      [--online [--jev-provider typesafe] [--jev-model MODEL] [--jev-max-in-flight N]
                                [--jev-request-limit N] [--jev-timeout-ms N] [--jev-no-cache]
                                [--jev-max-source-bytes N] [--jev-max-candidates N] [--jev-deadline-ms N]
                                [--jev-lookahead-max N]]
                      [--memory [--memory-read-deadline-ms N] [--memory-read-request-limit N]
                                [--memory-write-candidates N] [--memory-retention-days N] [--memory-max-nodes N]
                                [--memory-selection jev|deterministic]]
                      [--summarizer-cmd CMD [--summarizer-version-cmd CMD] [--summarizer-wait-ms N] [--summarizer-timeout-ms N]]
       ripwire-broker hook <claude-code|codex> <user-prompt-submit|post-tool-use|stop> [--workspace DIR]
                      [--ripwire BIN] [--timeout-ms N] [--state-dir DIR] [--every-prompt] [--gate] [--log-refs]
                      [--edit-interval-ms N] [--memory]
       ripwire-broker hook-log --session ID [--state-dir DIR]
       ripwire-broker hook-stats [--state-dir DIR] [--json]
       ripwire-broker prompt --workspace DIR [--ripwire BIN] [--timeout-ms N] [--budget N] TASK...
       ripwire-broker doctor --workspace DIR [--ripwire BIN] [--timeout-ms N] [--state-dir DIR] [--json]
                      [--jev-probe [--jev-model MODEL]]
                      [--summarizer-cmd CMD [--summarizer-version-cmd CMD]]
       ripwire-broker install <claude-code|codex> --workspace DIR [--hooks] [--statusline] [--write] [--codex-home DIR] [--online] [--memory]
       ripwire-broker statusline [--workspace DIR] [--state-dir DIR] [--detail] [--width N] [--color never|always]
       ripwire-broker memory status --workspace DIR [--state-dir DIR] [--json]
       ripwire-broker memory forget --workspace DIR [--state-dir DIR] (--all | --id ID)
       ripwire-broker memory add --workspace DIR [--state-dir DIR] --file PATH
       ripwire-broker memory drain --workspace DIR [--state-dir DIR] --online [--jev-model MODEL] [--memory-write-candidates N]
       ripwire-broker memory retry --workspace DIR [--state-dir DIR]
       ripwire-broker memory resume --workspace DIR [--state-dir DIR]

--online: O modo online envia previews e trechos elegíveis do workspace ao provider Jev.
Selecione somente uma raiz cujo conteúdo você tem autorização para enviar.
--memory: implica --online; guarda observações do workspace localmente e envia as elegíveis ao Jev.
--memory-selection deterministic: experimental, para avaliação; a mesma coleta, sem enriquecer nem perguntar ao Jev sobre memória.
The credential comes only from RIPWIRE_BROKER_JEV_API_KEY in the server's environment.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    ClaudeCode,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    UserPromptSubmit,
    PostToolUse,
    Stop,
}

/// How to reach ripwire; shared by every command that starts it.
#[derive(Debug, Clone, PartialEq)]
pub struct UpstreamArgs {
    pub ripwire: PathBuf,
    pub timeout: Duration,
}

impl Default for UpstreamArgs {
    fn default() -> Self {
        Self {
            ripwire: PathBuf::from("ripwire"),
            timeout: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServeArgs {
    pub workspace: PathBuf,
    pub ripwire: PathBuf,
    pub timeout: Duration,
    pub redact_workspace: bool,
    pub incremental: bool,
    /// Local model for notes (Phase 3); `None` keeps it off.
    pub summarizer: Option<SummarizerArgs>,
    /// Kill and restart ripwire above this resident memory (PRD 15.3); `None`: no limit.
    pub ripwire_max_rss_mb: Option<u64>,
    /// The remote classifier (PRD §23); `None` keeps the process offline (RF-ONLINE-01).
    pub online: Option<OnlineArgs>,
    /// Whether `--online` was asked for or implied by `--memory`; `None` when offline.
    pub online_origin: Option<OnlineOrigin>,
    /// Persistent memory (PRD jev-mem §4); implies `online`. `None` keeps no history.
    pub memory: Option<MemoryArgs>,
}

/// How the effective online mode came about, kept for diagnostics (PRD jev-mem §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineOrigin {
    Explicit,
    /// `--memory` without `--online`.
    Implied,
}

/// `--memory` and its `--memory-*` companions (PRD jev-mem §4).
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryArgs {
    /// Longest `context_for_task` waits for memory.
    pub read_deadline: Duration,
    /// Classifier requests one read may send; 0 serves only the cache and the index.
    pub read_request_limit: usize,
    /// Existing memories a new one is compared with.
    pub write_candidates: usize,
    pub retention_days: u32,
    pub max_nodes: usize,
    /// `--memory-selection`: the classifier (default) or, for evaluation, the local ranking.
    pub selection: crate::memory::retrieve::Selection,
}

/// `--online` and its `--jev-*` companions (PRD §23.6, D-059). No credential here: it comes
/// only from the server's environment.
#[derive(Debug, Clone, PartialEq)]
pub struct OnlineArgs {
    /// The only provider of the first increment.
    pub provider: String,
    /// Pinned by default; never a moving alias like `jev-latest`.
    pub model: String,
    pub max_in_flight: usize,
    /// Requests one MCP call may send.
    pub request_limit: usize,
    /// Per attempt.
    pub timeout: Duration,
    pub no_cache: bool,
    /// Caps rendered source, not what is evaluated; `None`: derived from the budget.
    pub max_source_bytes: Option<u64>,
    /// Planner paths the rescore evaluates (D-061).
    pub max_candidates: usize,
    /// Siblings the one-level lookahead may add; 0 turns it off (D-061).
    pub lookahead_max: usize,
    /// Past it the semantic stage stops and reports `interrupted` (D-063).
    pub deadline: Duration,
}

/// `--summarizer-cmd "ollama run phi4"` and its companions (D-034..D-036).
#[derive(Debug, Clone, PartialEq)]
pub struct SummarizerArgs {
    /// Split on whitespace into arguments; never run through a shell.
    pub command: String,
    /// Its output identifies the model version (PRD 10.3).
    pub version_cmd: Option<String>,
    /// Longest a response waits for a note.
    pub wait: Duration,
    /// Hard limit for one generation; the process is killed after it.
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HookArgs {
    pub host: Host,
    pub event: Event,
    /// Absent: the `cwd` of the host's event.
    pub workspace: Option<PathBuf>,
    pub upstream: UpstreamArgs,
    pub state_dir: Option<PathBuf>,
    pub every_prompt: bool,
    pub gate: bool,
    pub log_refs: bool,
    /// Absent: the default coalescing window. `0` answers every edit on its own.
    pub edit_interval_ms: Option<u64>,
    /// Publish observations to the workspace's memory spool (PD-3): local only, never HTTP,
    /// and never implies `--online`.
    pub memory: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptArgs {
    pub workspace: PathBuf,
    pub upstream: UpstreamArgs,
    pub budget_tokens: Option<u32>,
    pub task: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DoctorArgs {
    pub workspace: PathBuf,
    pub upstream: UpstreamArgs,
    /// Where hooks keep session state; checked for write access.
    pub state_dir: Option<PathBuf>,
    /// The local model the server would use; checked, never run.
    pub summarizer: Option<SummarizerArgs>,
    pub json: bool,
    /// Send one synthetic request to the classifier (D-064); off, the doctor never uses the
    /// network.
    pub jev_probe: bool,
    /// The model the probe asks; the pinned default otherwise.
    pub jev_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InstallArgs {
    pub host: Host,
    pub workspace: PathBuf,
    pub hooks: bool,
    pub statusline: bool,
    pub write: bool,
    pub codex_home: Option<PathBuf>,
    /// Start the server with `--online`, the credential referenced from the host's
    /// environment, never written (D-064). Hooks stay offline.
    pub online: bool,
    /// `--memory` on the server and the hooks (PD-3); implies online on the server only.
    pub memory: bool,
}

/// What `memory` does; every action is local: no network, credential or `online` feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryAction {
    /// Queues, sizes, schema and the store's error category, if any.
    Status { json: bool },
    /// One memory and every note derived from it.
    Forget { id: String },
    /// Everything, and collection is revoked until `Resume`.
    ForgetAll,
    /// An explicit note from a JSON file (PD-1).
    Add { file: PathBuf },
    /// Incorporates the spool and runs ready jobs against the provider; needs `--online` (PD-2).
    /// `model` and `candidates` should match the server's, so edge keys do.
    Drain {
        model: Option<String>,
        candidates: Option<usize>,
    },
    /// Gives failed enrichment jobs their runs back: the explicit action of PRD jev-mem §8.2.
    Retry,
    /// Lifts the revocation `memory forget --all` leaves (PD-4).
    Resume,
}

/// `memory <action> --workspace DIR [--state-dir DIR]` (PRD jev-mem §4).
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryCommand {
    pub action: MemoryAction,
    pub workspace: PathBuf,
    pub state_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatuslineArgs {
    pub workspace: Option<PathBuf>,
    pub state_dir: Option<PathBuf>,
    pub detail: bool,
    pub width: Option<usize>,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Serve(ServeArgs),
    Hook(HookArgs),
    HookLog {
        session: String,
        state_dir: Option<PathBuf>,
    },
    /// Every saved hook session reduced to counts (§21.3).
    HookStats {
        state_dir: Option<PathBuf>,
        json: bool,
    },
    Prompt(PromptArgs),
    Doctor(DoctorArgs),
    Install(InstallArgs),
    /// Prints the Claude Code status line from the hooks' projection; never starts ripwire.
    Statusline(StatuslineArgs),
    /// The workspace's persistent memory, locally.
    Memory(MemoryCommand),
    /// `--help` or `--version`: print and exit successfully.
    Info(String),
    /// Internal: run `argv` under a memory limit (how the server starts ripwire, D-050).
    Supervise {
        max_rss_mb: u64,
        argv: Vec<String>,
    },
    /// Internal: the supervisor's watcher process (D-052).
    Watch {
        parent: u32,
        child: u32,
        max_rss_mb: u64,
        program: String,
    },
}

fn usage(detail: impl std::fmt::Display) -> String {
    format!("{detail}\n{USAGE}")
}

fn host(s: Option<String>) -> Result<Host, String> {
    match s.as_deref() {
        Some("claude-code") => Ok(Host::ClaudeCode),
        Some("codex") => Ok(Host::Codex),
        other => Err(usage(format_args!("unknown host {other:?}"))),
    }
}

fn event(s: Option<String>) -> Result<Event, String> {
    match s.as_deref() {
        Some("user-prompt-submit") => Ok(Event::UserPromptSubmit),
        Some("post-tool-use") => Ok(Event::PostToolUse),
        Some("stop") => Ok(Event::Stop),
        other => Err(usage(format_args!("unknown hook event {other:?}"))),
    }
}

/// Flags as they appear; each command then takes the ones it accepts.
#[derive(Default)]
struct Flags {
    workspace: Option<PathBuf>,
    upstream: UpstreamArgs,
    state_dir: Option<PathBuf>,
    session: Option<String>,
    codex_home: Option<PathBuf>,
    budget: Option<u32>,
    summarizer_cmd: Option<String>,
    summarizer_version_cmd: Option<String>,
    summarizer_wait: Option<Duration>,
    summarizer_timeout: Option<Duration>,
    max_rss_mb: Option<u64>,
    edit_interval_ms: Option<u64>,
    width: Option<u64>,
    color: Option<String>,
    id: Option<String>,
    file: Option<PathBuf>,
    jev: HashMap<&'static str, String>,
    memory: HashMap<&'static str, String>,
    switches: Vec<&'static str>,
    words: Vec<String>,
}

impl Flags {
    fn on(&self, switch: &str) -> bool {
        self.switches.contains(&switch)
    }

    fn summarizer(&self) -> Result<Option<SummarizerArgs>, String> {
        let Some(command) = self.summarizer_cmd.clone() else {
            let companion = self.summarizer_version_cmd.is_some()
                || self.summarizer_wait.is_some()
                || self.summarizer_timeout.is_some();
            return match companion {
                true => Err(usage("the --summarizer-* options need --summarizer-cmd")),
                false => Ok(None),
            };
        };
        Ok(Some(SummarizerArgs {
            command,
            version_cmd: self.summarizer_version_cmd.clone(),
            wait: self.summarizer_wait.unwrap_or(Duration::from_millis(1500)),
            timeout: self.summarizer_timeout.unwrap_or(Duration::from_secs(60)),
        }))
    }

    /// `--memory` counts as `--online` (PRD jev-mem §4).
    fn online(&self) -> Result<Option<OnlineArgs>, String> {
        if !self.on("--online") && !self.on("--memory") {
            return match self.jev.is_empty() && !self.on("--jev-no-cache") {
                true => Ok(None),
                false => Err(usage("the --jev-* options need --online")),
            };
        }
        let text = |k: &str, default: &str| self.jev.get(k).map_or(default.into(), String::clone);
        let number = |k: &str, default: u64| -> Result<u64, String> {
            match self.jev.get(k) {
                None => Ok(default),
                Some(v) => v
                    .parse::<u64>()
                    .map_err(|_| usage(format_args!("{k} needs a number"))),
            }
        };
        let positive = |k: &str, default: u64| match number(k, default)? {
            0 => Err(usage(format_args!("{k} must be at least 1"))),
            n => Ok(n),
        };
        let provider = text("--jev-provider", "typesafe");
        if provider != "typesafe" {
            return Err(usage("--jev-provider: the only provider is typesafe"));
        }
        Ok(Some(OnlineArgs {
            provider,
            model: text("--jev-model", "jev-1.13.0"),
            max_in_flight: positive("--jev-max-in-flight", 4)? as usize,
            request_limit: positive("--jev-request-limit", 24)? as usize,
            timeout: Duration::from_millis(positive("--jev-timeout-ms", 15_000)?),
            no_cache: self.on("--jev-no-cache"),
            max_source_bytes: match self.jev.contains_key("--jev-max-source-bytes") {
                true => Some(positive("--jev-max-source-bytes", 0)?),
                false => None,
            },
            max_candidates: positive("--jev-max-candidates", 16)? as usize,
            lookahead_max: number("--jev-lookahead-max", 32)? as usize,
            deadline: Duration::from_millis(positive("--jev-deadline-ms", 8_000)?),
        }))
    }

    fn online_origin(&self) -> Option<OnlineOrigin> {
        match (self.on("--online"), self.on("--memory")) {
            (true, _) => Some(OnlineOrigin::Explicit),
            (false, true) => Some(OnlineOrigin::Implied),
            (false, false) => None,
        }
    }

    fn memory(&self) -> Result<Option<MemoryArgs>, String> {
        if !self.on("--memory") {
            return match self.memory.is_empty() {
                true => Ok(None),
                false => Err(usage("the --memory-* options need --memory")),
            };
        }
        // Each option with its default and its range (PRD jev-mem §4).
        let within = |k: &str, default: u64, min: u64, max: u64| -> Result<u64, String> {
            let Some(v) = self.memory.get(k) else {
                return Ok(default);
            };
            match v.parse::<u64>() {
                Ok(n) if (min..=max).contains(&n) => Ok(n),
                _ => Err(usage(format_args!(
                    "{k} takes a number from {min} to {max}"
                ))),
            }
        };
        Ok(Some(MemoryArgs {
            read_deadline: Duration::from_millis(within("--memory-read-deadline-ms", 750, 1, 750)?),
            read_request_limit: within("--memory-read-request-limit", 4, 0, 4)? as usize,
            write_candidates: within("--memory-write-candidates", 4, 0, 10)? as usize,
            retention_days: within("--memory-retention-days", 30, 1, 365)? as u32,
            max_nodes: within("--memory-max-nodes", 2000, 1, 2000)? as usize,
            selection: match self.memory.get("--memory-selection").map(String::as_str) {
                None | Some("jev") => crate::memory::retrieve::Selection::Jev,
                Some("deterministic") => crate::memory::retrieve::Selection::Deterministic,
                Some(_) => {
                    return Err(usage("--memory-selection takes jev or deterministic"));
                }
            },
        }))
    }

    fn workspace(&self) -> Result<PathBuf, String> {
        self.workspace
            .clone()
            .ok_or_else(|| usage("--workspace is required"))
    }
}

const SWITCHES: &[&str] = &[
    "--redact-workspace",
    "--incremental",
    "--every-prompt",
    "--gate",
    "--log-refs",
    "--json",
    "--hooks",
    "--statusline",
    "--detail",
    "--write",
    "--online",
    "--memory",
    "--jev-no-cache",
    "--jev-probe",
    "--all",
];

/// The valued `--jev-*` flags; kept as text until `Flags::online` checks them.
const JEV: &[&str] = &[
    "--jev-provider",
    "--jev-model",
    "--jev-max-in-flight",
    "--jev-request-limit",
    "--jev-timeout-ms",
    "--jev-max-source-bytes",
    "--jev-max-candidates",
    "--jev-deadline-ms",
    "--jev-lookahead-max",
];

/// The valued `--memory-*` flags; kept as text until `Flags::memory` checks them.
const MEMORY: &[&str] = &[
    "--memory-read-deadline-ms",
    "--memory-read-request-limit",
    "--memory-write-candidates",
    "--memory-retention-days",
    "--memory-max-nodes",
    "--memory-selection",
];

/// `allowed` lists the switches and valued flags this command accepts.
fn flags(args: impl Iterator<Item = String>, allowed: &[&str]) -> Result<Flags, String> {
    let mut f = Flags::default();
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        if !a.starts_with("--") {
            f.words.push(a);
            continue;
        }
        if !allowed.contains(&a.as_str()) {
            if a == "--online" {
                return Err(usage("--online is only available to serve (D-064)"));
            }
            return Err(usage(format_args!("unknown argument '{a}'")));
        }
        if let Some(s) = SWITCHES.iter().find(|s| **s == a) {
            f.switches.push(s);
            continue;
        }
        let value = args
            .next()
            .ok_or_else(|| usage(format_args!("{a} needs a value")))?;
        let number = |v: &str| {
            v.parse::<u64>()
                .map_err(|_| usage(format_args!("{a} needs a number")))
        };
        match a.as_str() {
            "--workspace" => f.workspace = Some(value.into()),
            "--ripwire" => f.upstream.ripwire = value.into(),
            "--timeout-ms" => f.upstream.timeout = Duration::from_millis(number(&value)?),
            "--state-dir" => f.state_dir = Some(value.into()),
            "--session" => f.session = Some(value),
            "--codex-home" => f.codex_home = Some(value.into()),
            "--budget" => f.budget = Some(number(&value)? as u32),
            "--summarizer-cmd" => f.summarizer_cmd = Some(value),
            "--ripwire-max-rss-mb" | "--max-rss-mb" => f.max_rss_mb = Some(number(&value)?),
            "--edit-interval-ms" => f.edit_interval_ms = Some(number(&value)?),
            "--summarizer-version-cmd" => f.summarizer_version_cmd = Some(value),
            "--summarizer-wait-ms" => {
                f.summarizer_wait = Some(Duration::from_millis(number(&value)?))
            }
            "--summarizer-timeout-ms" => {
                f.summarizer_timeout = Some(Duration::from_millis(number(&value)?))
            }
            "--width" => f.width = Some(number(&value)?),
            "--color" => f.color = Some(value),
            "--id" => f.id = Some(value),
            "--file" => f.file = Some(value.into()),
            jev if JEV.contains(&jev) => {
                let key = JEV.iter().find(|k| **k == jev).unwrap();
                f.jev.insert(key, value);
            }
            memory if MEMORY.contains(&memory) => {
                let key = MEMORY.iter().find(|k| **k == memory).unwrap();
                f.memory.insert(key, value);
            }
            _ => return Err(usage(format_args!("unknown argument '{a}'"))),
        }
    }
    Ok(f)
}

const SUMMARIZER: [&str; 4] = [
    "--summarizer-cmd",
    "--summarizer-version-cmd",
    "--summarizer-wait-ms",
    "--summarizer-timeout-ms",
];

const UPSTREAM: &[&str] = &["--workspace", "--ripwire", "--timeout-ms"];

fn with<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    UPSTREAM.iter().chain(extra).copied().collect()
}

fn no_words(f: &Flags) -> Result<(), String> {
    match f.words.first() {
        Some(w) => Err(usage(format_args!("unexpected '{w}'"))),
        None => Ok(()),
    }
}

pub fn parse(args: Vec<String>) -> Result<Command, String> {
    let own = args
        .iter()
        .position(|a| a == "--")
        .map_or(&args[..], |i| &args[..i]);
    if own.iter().any(|a| a == "-h" || a == "--help") {
        return Ok(Command::Info(USAGE.into()));
    }
    if own.iter().any(|a| a == "--version") {
        return Ok(Command::Info(format!(
            "ripwire-broker {}",
            env!("CARGO_PKG_VERSION")
        )));
    }
    let mut it = args.into_iter().peekable();
    let sub = match it.peek().map(String::as_str) {
        Some(s) if !s.starts_with("--") => it.next(),
        _ => None,
    };
    match sub.as_deref() {
        None | Some("serve") => {
            let f = flags(
                it,
                &with(
                    &[
                        "--redact-workspace",
                        "--incremental",
                        "--ripwire-max-rss-mb",
                        SUMMARIZER[0],
                        SUMMARIZER[1],
                        SUMMARIZER[2],
                        SUMMARIZER[3],
                        "--online",
                        "--memory",
                        "--jev-no-cache",
                    ]
                    .into_iter()
                    .chain(JEV.iter().copied())
                    .chain(MEMORY.iter().copied())
                    .collect::<Vec<_>>(),
                ),
            )?;
            no_words(&f)?;
            Ok(Command::Serve(ServeArgs {
                workspace: f.workspace()?,
                ripwire: f.upstream.ripwire.clone(),
                timeout: f.upstream.timeout,
                redact_workspace: f.on("--redact-workspace"),
                incremental: f.on("--incremental"),
                summarizer: f.summarizer()?,
                ripwire_max_rss_mb: f.max_rss_mb,
                online: f.online()?,
                online_origin: f.online_origin(),
                memory: f.memory()?,
            }))
        }
        Some("__supervise") => {
            let rest: Vec<String> = it.collect();
            let split = rest
                .iter()
                .position(|a| a == "--")
                .ok_or_else(|| usage("__supervise needs -- COMMAND"))?;
            let f = flags(rest[..split].iter().cloned(), &["--max-rss-mb"])?;
            no_words(&f)?;
            Ok(Command::Supervise {
                max_rss_mb: f
                    .max_rss_mb
                    .ok_or_else(|| usage("--max-rss-mb is required"))?,
                argv: rest[split + 1..].to_vec(),
            })
        }
        Some("__watch") => {
            let mut values = HashMap::new();
            let mut it = it;
            while let (Some(k), Some(v)) = (it.next(), it.next()) {
                values.insert(k, v);
            }
            let num = |k: &str| {
                values
                    .get(k)
                    .and_then(|v| v.parse::<u64>().ok())
                    .ok_or_else(|| usage(format_args!("__watch needs {k}")))
            };
            Ok(Command::Watch {
                parent: num("--parent")? as u32,
                child: num("--child")? as u32,
                max_rss_mb: num("--max-rss-mb")?,
                program: values.get("--program").cloned().unwrap_or_default(),
            })
        }
        Some("hook") => {
            let (host, event) = (host(it.next())?, event(it.next())?);
            let f = flags(
                it,
                &with(&[
                    "--state-dir",
                    "--every-prompt",
                    "--gate",
                    "--log-refs",
                    "--edit-interval-ms",
                    "--memory",
                ]),
            )?;
            no_words(&f)?;
            Ok(Command::Hook(HookArgs {
                host,
                event,
                workspace: f.workspace.clone(),
                upstream: f.upstream.clone(),
                state_dir: f.state_dir.clone(),
                every_prompt: f.on("--every-prompt"),
                gate: f.on("--gate"),
                log_refs: f.on("--log-refs"),
                edit_interval_ms: f.edit_interval_ms,
                memory: f.on("--memory"),
            }))
        }
        Some("hook-log") => {
            let f = flags(it, &["--session", "--state-dir"])?;
            no_words(&f)?;
            Ok(Command::HookLog {
                session: f.session.ok_or_else(|| usage("--session is required"))?,
                state_dir: f.state_dir,
            })
        }
        Some("hook-stats") => {
            let f = flags(it, &["--state-dir", "--json"])?;
            no_words(&f)?;
            Ok(Command::HookStats {
                state_dir: f.state_dir.clone(),
                json: f.on("--json"),
            })
        }
        Some("prompt") => {
            let f = flags(it, &with(&["--budget"]))?;
            if f.words.is_empty() {
                return Err(usage("prompt needs a TASK"));
            }
            Ok(Command::Prompt(PromptArgs {
                workspace: f.workspace()?,
                upstream: f.upstream.clone(),
                budget_tokens: f.budget,
                task: f.words.join(" "),
            }))
        }
        Some("doctor") => {
            let f = flags(
                it,
                &with(&[
                    "--json",
                    "--state-dir",
                    SUMMARIZER[0],
                    SUMMARIZER[1],
                    "--jev-probe",
                    "--jev-model",
                ]),
            )?;
            no_words(&f)?;
            let jev_model = f.jev.get("--jev-model").cloned();
            if jev_model.is_some() && !f.on("--jev-probe") {
                return Err(usage("--jev-model needs --jev-probe here"));
            }
            Ok(Command::Doctor(DoctorArgs {
                workspace: f.workspace()?,
                upstream: f.upstream.clone(),
                state_dir: f.state_dir.clone(),
                summarizer: f.summarizer()?,
                json: f.on("--json"),
                jev_probe: f.on("--jev-probe"),
                jev_model,
            }))
        }
        Some("install") => {
            let host = host(it.next())?;
            let f = flags(
                it,
                &[
                    "--workspace",
                    "--hooks",
                    "--statusline",
                    "--write",
                    "--codex-home",
                    "--online",
                    "--memory",
                ],
            )?;
            no_words(&f)?;
            if f.on("--statusline") && host != Host::ClaudeCode {
                return Err(usage("--statusline is only available to claude-code"));
            }
            Ok(Command::Install(InstallArgs {
                host,
                workspace: f.workspace()?,
                hooks: f.on("--hooks"),
                statusline: f.on("--statusline"),
                write: f.on("--write"),
                codex_home: f.codex_home.clone(),
                online: f.on("--online"),
                memory: f.on("--memory"),
            }))
        }
        Some("statusline") => {
            let f = flags(
                it,
                &[
                    "--workspace",
                    "--state-dir",
                    "--detail",
                    "--width",
                    "--color",
                ],
            )?;
            no_words(&f)?;
            let color = match f.color.as_deref() {
                None | Some("never") => Color::Never,
                Some("always") => Color::Always,
                Some(other) => {
                    return Err(usage(format_args!(
                        "--color takes never or always, not '{other}'"
                    )));
                }
            };
            Ok(Command::Statusline(StatuslineArgs {
                workspace: f.workspace.clone(),
                state_dir: f.state_dir.clone(),
                detail: f.on("--detail"),
                width: f.width.map(|w| w as usize),
                color,
            }))
        }
        Some("memory") => {
            let verb = it.next();
            let extra: &[&str] = match verb.as_deref() {
                Some("status") => &["--json"],
                Some("forget") => &["--all", "--id"],
                Some("add") => &["--file"],
                Some("drain") => &["--online", "--jev-model", "--memory-write-candidates"],
                Some("retry") => &[],
                Some("resume") => &[],
                other => return Err(usage(format_args!("unknown memory command {other:?}"))),
            };
            let allowed: Vec<&str> = ["--workspace", "--state-dir"]
                .iter()
                .chain(extra)
                .copied()
                .collect();
            let f = flags(it, &allowed)?;
            no_words(&f)?;
            let action = match (verb.as_deref(), f.on("--all"), f.id.clone()) {
                (Some("status"), ..) => MemoryAction::Status {
                    json: f.on("--json"),
                },
                (Some("forget"), true, None) => MemoryAction::ForgetAll,
                (Some("forget"), false, Some(id)) => MemoryAction::Forget { id },
                (Some("forget"), ..) => return Err(usage("memory forget takes --all or --id ID")),
                (Some("drain"), ..) => match f.on("--online") {
                    true => MemoryAction::Drain {
                        model: f.jev.get("--jev-model").cloned(),
                        candidates: match f.memory.get("--memory-write-candidates") {
                            None => None,
                            Some(v) => match v.parse::<usize>() {
                                Ok(n) if n <= 10 => Some(n),
                                _ => {
                                    return Err(usage(
                                        "--memory-write-candidates takes a number from 0 to 10",
                                    ));
                                }
                            },
                        },
                    },
                    false => {
                        return Err(usage(
                            "memory drain needs --online: it sends memories to the provider",
                        ));
                    }
                },
                (Some("add"), ..) => MemoryAction::Add {
                    file: f
                        .file
                        .clone()
                        .ok_or_else(|| usage("memory add needs --file PATH"))?,
                },
                (Some("retry"), ..) => MemoryAction::Retry,
                _ => MemoryAction::Resume,
            };
            Ok(Command::Memory(MemoryCommand {
                action,
                workspace: f.workspace()?,
                state_dir: f.state_dir.clone(),
            }))
        }
        Some(other) => Err(usage(format_args!("unknown command '{other}'"))),
    }
}
