//! Runs every (task, arm, repeat) once: a fresh repository holding the task's base and its
//! ancestors only, the agent in it with the arm's MCP configuration, the transcript saved, the
//! copy scored and deleted. The source repository is only ever read, by `git fetch`; nothing is
//! written to it.

use super::arm::{Arm, Tools};
use super::corpus::{Corpus, Task};
use super::report::{self, RunRecord};
use super::score;
use super::transcript::{self, Summary};
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Claude Code, headless, isolated from the user's own configuration: only the arm's MCP
/// server (`--strict-mcp-config`), and no user-level settings, hooks or plugins
/// (`--setting-sources project`). Split on whitespace; never through a shell.
pub const DEFAULT_AGENT: &str = "claude -p --output-format stream-json --verbose \
    --strict-mcp-config --mcp-config {mcp_config} --setting-sources project \
    --permission-mode bypassPermissions --no-session-persistence";

pub const ONLINE_KEY: &str = "RIPWIRE_BROKER_JEV_API_KEY";

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub corpus: Corpus,
    pub out: PathBuf,
    pub arms: Vec<Arm>,
    pub repeats: u32,
    /// The agent's argv, with `{mcp_config}` and `{workdir}` replaced per run.
    pub agent: Vec<String>,
    pub tools: Tools,
    pub timeout: Duration,
    pub check_timeout: Duration,
}

fn git(dir: &Path, args: &[&str]) -> Result<(), String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// A fresh private directory under the system temp dir; removed by the caller.
fn scratch(n: usize) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!(
        "ripwire-eval-{}-{n}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// The task's repository at its base, and nothing after it. A task taken from history has its
/// answer in a later commit of the same repository; a plain clone would carry every branch, and
/// `git log --all` would hand the agent the fix. So: a new repository, fetched by the base's id
/// alone (only its ancestors come along), with no ref but a detached `HEAD`.
fn checkout_base(task: &Task, into: &Path) -> Result<(), String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(&task.repo)
        .args([
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", task.base),
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git rev-parse: {e}"))?;
    if !out.status.success() {
        return Err(format!("base {} is not a commit", task.base));
    }
    let base = String::from_utf8_lossy(&out.stdout).trim().to_string();
    std::fs::create_dir_all(into).map_err(|e| e.to_string())?;
    git(into, &["init", "--quiet"])?;
    git(
        into,
        &[
            "fetch",
            "--quiet",
            "--no-tags",
            &task.repo.to_string_lossy(),
            &base,
        ],
    )?;
    git(into, &["checkout", "--quiet", "--detach", &base])?;
    // FETCH_HEAD names the source repository's path; the agent has no use for it.
    let _ = std::fs::remove_file(into.join(".git/FETCH_HEAD"));
    Ok(())
}

/// What one agent run left: the timed transcript, or why there is none.
struct AgentRun {
    events: Vec<(u64, serde_json::Value)>,
    timed_out: bool,
}

fn run_agent(
    argv: &[String],
    workdir: &Path,
    prompt: &str,
    transcript: &Path,
    stderr: &Path,
    timeout: Duration,
) -> Result<AgentRun, String> {
    let (program, args) = argv.split_first().ok_or("empty agent command")?;
    let err = std::fs::File::create(stderr).map_err(|e| e.to_string())?;
    let mut child = Command::new(program)
        .args(args)
        .current_dir(workdir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(err)
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(prompt.as_bytes());
    }
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (tx, rx) = mpsc::channel();
    let start = Instant::now();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send((start.elapsed().as_millis() as u64, line)).is_err() {
                break;
            }
        }
    });
    let mut file = std::fs::File::create(transcript).map_err(|e| e.to_string())?;
    let mut events = vec![];
    let mut timed_out = false;
    loop {
        let left = timeout.saturating_sub(start.elapsed());
        match rx.recv_timeout(left) {
            Ok((at, line)) => {
                let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                let _ = writeln!(file, "{}", json!({"at_ms": at, "event": event}));
                events.push((at, event));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                timed_out = true;
                let _ = child.kill();
                break;
            }
        }
    }
    let _ = child.wait();
    let _ = reader.join();
    Ok(AgentRun { events, timed_out })
}

fn record(task: &Task, arm: Arm, repeat: u32, s: &Summary, sc: &score::Score) -> RunRecord {
    RunRecord {
        task: task.id.clone(),
        repo: task.repo_name(),
        arm: arm.name().into(),
        repeat,
        vocabulary_diverges: task.vocabulary_diverges,
        valid: s.valid(),
        invalid: s.invalid.clone(),
        is_error: s.is_error,
        tokens: s.tokens.total(),
        cost_usd: s.cost_usd,
        duration_ms: s.duration_ms,
        search: s.calls.search,
        read: s.calls.read,
        edit: s.calls.edit,
        mcp: s.calls.mcp,
        other: s.calls.other,
        mcp_result_bytes: s.mcp_result_bytes,
        first_edit_ms: s.first_edit_ms,
        file_recall: sc.file_recall,
        file_precision: sc.file_precision,
        presented_recall: sc.presented_recall,
        first_correct_rank: sc.first_correct_rank,
        test_recall: sc.test_recall,
        correct: sc.correct,
    }
}

fn one(cfg: &RunConfig, task: &Task, arm: Arm, repeat: u32, n: usize) -> RunRecord {
    let failed = |why: String| RunRecord {
        task: task.id.clone(),
        repo: task.repo_name(),
        arm: arm.name().into(),
        repeat,
        vocabulary_diverges: task.vocabulary_diverges,
        invalid: Some(why),
        ..RunRecord::default()
    };
    let dir = match scratch(n) {
        Ok(d) => d,
        Err(e) => return failed(e),
    };
    let result = (|| {
        let work = dir.join("work");
        checkout_base(task, &work)?;
        let config = dir.join("mcp.json");
        std::fs::write(&config, arm.mcp_config(&cfg.tools, &work).to_string())
            .map_err(|e| e.to_string())?;
        let argv: Vec<String> = cfg
            .agent
            .iter()
            .map(|a| {
                a.replace("{mcp_config}", &config.to_string_lossy())
                    .replace("{workdir}", &work.to_string_lossy())
            })
            .collect();
        let name = format!("{}__{}__{repeat}", task.id, arm.name());
        let transcripts = cfg.out.join("transcripts");
        let run = run_agent(
            &argv,
            &work,
            &task.prompt,
            &transcripts.join(format!("{name}.jsonl")),
            &transcripts.join(format!("{name}.stderr")),
            cfg.timeout,
        )?;
        let mut s = transcript::summarize(&run.events);
        if run.timed_out {
            s.invalid = Some(format!("timed out after {} s", cfg.timeout.as_secs()));
        } else if s.valid() {
            s.invalid = arm.contamination(&s);
        }
        let modified = score::modified_files(&work);
        let correct = task
            .check
            .as_deref()
            .map(|c| score::run_check(&work, c, cfg.check_timeout));
        let sc = score::score(task, &s, &modified, correct);
        Ok::<_, String>(record(task, arm, repeat, &s, &sc))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result.unwrap_or_else(failed)
}

/// Runs what `results.jsonl` does not have yet; returns how many runs it made.
pub fn run(cfg: &RunConfig, log: &mut dyn FnMut(&str)) -> Result<usize, String> {
    cfg.corpus.validate().map_err(|e| e.join("\n"))?;
    if cfg.arms.contains(&Arm::BrokerOnline) && std::env::var_os(ONLINE_KEY).is_none() {
        return Err(format!(
            "arm broker-online needs {ONLINE_KEY} in the environment, and consent to send \
             eligible source of every corpus repository to the provider (PRD §23.6)"
        ));
    }
    std::fs::create_dir_all(cfg.out.join("transcripts")).map_err(|e| e.to_string())?;
    let done: std::collections::HashSet<_> =
        report::load(&cfg.out).iter().map(RunRecord::key).collect();
    let mut results = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cfg.out.join("results.jsonl"))
        .map_err(|e| e.to_string())?;
    let mut made = 0;
    for task in &cfg.corpus.tasks {
        for repeat in 1..=cfg.repeats {
            for &arm in &cfg.arms {
                if done.contains(&(task.id.clone(), arm.name().to_string(), repeat)) {
                    continue;
                }
                log(&format!("{} · {} · {repeat}", task.id, arm.name()));
                let r = one(cfg, task, arm, repeat, made);
                if let Some(why) = &r.invalid {
                    log(&format!("  invalid: {why}"));
                }
                let line = serde_json::to_string(&r).map_err(|e| e.to_string())?;
                writeln!(results, "{line}").map_err(|e| e.to_string())?;
                made += 1;
            }
        }
    }
    Ok(made)
}
