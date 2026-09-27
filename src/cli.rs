//! Command line (D-028): `serve` (the default, also without a subcommand), `hook`, `hook-log`,
//! `prompt`, `doctor` and `install`. Parsing is pure; nothing here touches the disk.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

pub const USAGE: &str = "\
usage: ripwire-broker [serve] --workspace DIR [--ripwire BIN] [--timeout-ms N] [--redact-workspace] [--incremental]
                      [--ripwire-max-rss-mb N]
                      [--summarizer-cmd CMD [--summarizer-version-cmd CMD] [--summarizer-wait-ms N] [--summarizer-timeout-ms N]]
       ripwire-broker hook <claude-code|codex> <user-prompt-submit|post-tool-use|stop> [--workspace DIR]
                      [--ripwire BIN] [--timeout-ms N] [--state-dir DIR] [--every-prompt] [--gate] [--log-refs]
       ripwire-broker hook-log --session ID [--state-dir DIR]
       ripwire-broker prompt --workspace DIR [--ripwire BIN] [--timeout-ms N] [--budget N] TASK...
       ripwire-broker doctor --workspace DIR [--ripwire BIN] [--timeout-ms N] [--state-dir DIR] [--json]
                      [--summarizer-cmd CMD [--summarizer-version-cmd CMD]]
       ripwire-broker install <claude-code|codex> --workspace DIR [--hooks] [--write] [--codex-home DIR]";

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
}

#[derive(Debug, Clone, PartialEq)]
pub struct InstallArgs {
    pub host: Host,
    pub workspace: PathBuf,
    pub hooks: bool,
    pub write: bool,
    pub codex_home: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Serve(ServeArgs),
    Hook(HookArgs),
    HookLog {
        session: String,
        state_dir: Option<PathBuf>,
    },
    Prompt(PromptArgs),
    Doctor(DoctorArgs),
    Install(InstallArgs),
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
    "--write",
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
            "--summarizer-version-cmd" => f.summarizer_version_cmd = Some(value),
            "--summarizer-wait-ms" => {
                f.summarizer_wait = Some(Duration::from_millis(number(&value)?))
            }
            "--summarizer-timeout-ms" => {
                f.summarizer_timeout = Some(Duration::from_millis(number(&value)?))
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
                &with(&[
                    "--redact-workspace",
                    "--incremental",
                    "--ripwire-max-rss-mb",
                    SUMMARIZER[0],
                    SUMMARIZER[1],
                    SUMMARIZER[2],
                    SUMMARIZER[3],
                ]),
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
                &with(&["--state-dir", "--every-prompt", "--gate", "--log-refs"]),
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
                &with(&["--json", "--state-dir", SUMMARIZER[0], SUMMARIZER[1]]),
            )?;
            no_words(&f)?;
            Ok(Command::Doctor(DoctorArgs {
                workspace: f.workspace()?,
                upstream: f.upstream.clone(),
                state_dir: f.state_dir.clone(),
                summarizer: f.summarizer()?,
                json: f.on("--json"),
            }))
        }
        Some("install") => {
            let host = host(it.next())?;
            let f = flags(it, &["--workspace", "--hooks", "--write", "--codex-home"])?;
            no_words(&f)?;
            Ok(Command::Install(InstallArgs {
                host,
                workspace: f.workspace()?,
                hooks: f.on("--hooks"),
                write: f.on("--write"),
                codex_home: f.codex_home.clone(),
            }))
        }
        Some(other) => Err(usage(format_args!("unknown command '{other}'"))),
    }
}
