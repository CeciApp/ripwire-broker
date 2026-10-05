//! Runs every (task, arm, repeat) once: a fresh repository holding the task's base and its
//! ancestors only, the agent in it with the arm's MCP configuration, the transcript saved, the
//! copy scored and deleted. The tasks of a sequence are one round: they run in order, each from
//! its own base but in the same place and with the same memory store, which goes with the round.
//! The source repository is only ever read, by `git fetch`; nothing is written to it.

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

/// Claude Code, headless, isolated: only the arm's MCP server (`--strict-mcp-config`), and no
/// settings but `.claude/settings.local.json`, which a fresh repository never has
/// (`--setting-sources local`). Neither the user's hooks and plugins nor the ones a repository
/// commits load; hook events are reported so the guard can see one that slipped through.
/// Split on whitespace; never through a shell.
pub const DEFAULT_AGENT: &str = "claude -p --output-format stream-json --verbose \
    --include-hook-events --strict-mcp-config --mcp-config {mcp_config} \
    --setting-sources local --permission-mode bypassPermissions --no-session-persistence";

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

/// A fresh directory under the system temp dir, unique within this process; removed by the
/// caller.
fn scratch() -> Result<PathBuf, String> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("ripwire-eval-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// The task's repository at its base, and nothing after it. A task taken from history has its
/// answer in a later commit of the same repository; a plain clone would carry every branch, and
/// `git log --all` would hand the agent the fix. So: a new repository, fetched by the base's id
/// alone (only its ancestors come along), with no ref but a detached `HEAD`.
fn checkout_base(task: &Task, into: &Path) -> Result<(), String> {
    checkout(task, &task.base, into)
}

/// `rev` of the task's repository and its ancestors only, as `checkout_base` explains.
fn checkout(task: &Task, rev: &str, into: &Path) -> Result<(), String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(&task.repo)
        .args(["rev-parse", "--verify", &format!("{rev}^{{commit}}")])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git rev-parse: {e}"))?;
    if !out.status.success() {
        return Err(format!("{rev} is not a commit"));
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

/// The run's id, safe for a database or file name: lowercase letters, digits and `_`.
fn run_id(task: &Task, arm: Arm, repeat: u32) -> String {
    format!("{}__{}__{repeat}", task.id, arm.name())
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// `{run}`, `{repo}` and `{fix}` in a task's command or environment value, each value passed
/// through `quote` (a command is shell; an environment value is not).
fn expand(task: &Task, id: &str, text: &str, quote: fn(&str) -> String) -> String {
    text.replace("{run}", &quote(id))
        .replace("{repo}", &quote(&task.repo.to_string_lossy()))
        .replace("{fix}", &quote(task.fix.as_deref().unwrap_or("")))
}

/// One shell word: as is when nothing in it is special, otherwise in single quotes.
fn shell_word(text: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "_./:@%+=,-".contains(c);
    if !text.is_empty() && text.chars().all(plain) {
        text.to_string()
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

fn environment(task: &Task, id: &str) -> Vec<(String, String)> {
    task.env
        .iter()
        .map(|(k, v)| (k.clone(), expand(task, id, v, str::to_string)))
        .collect()
}

/// A task's shell command (`setup`, `check`, `teardown`) in the copy, with the run's environment.
fn shell(
    task: &Task,
    id: &str,
    workdir: &Path,
    env: &[(String, String)],
    command: &str,
) -> Command {
    let mut c = Command::new("sh");
    c.args(["-c", &expand(task, id, command, shell_word)])
        .current_dir(workdir)
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null());
    c
}

fn run_agent(
    argv: &[String],
    env: &[(String, String)],
    workdir: &Path,
    prompt: &str,
    transcript: &Path,
    stderr: &Path,
    timeout: Duration,
) -> Result<AgentRun, String> {
    use std::os::unix::process::CommandExt as _;
    let (program, args) = argv.split_first().ok_or("empty agent command")?;
    let err = std::fs::File::create(stderr).map_err(|e| e.to_string())?;
    // A group of its own, so that a timeout ends whatever the agent started: a child left
    // holding its stdout would keep the run waiting for an end of file that never comes.
    let mut child = Command::new(program)
        .args(args)
        .process_group(0)
        .envs(env.iter().map(|(k, v)| (k, v)))
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
                score::kill_group(child.id());
                break;
            }
        }
    }
    let _ = child.wait();
    let _ = reader.join();
    Ok(AgentRun { events, timed_out })
}

/// The round's store as its server sees it (`memory status`): attempts and questions charged in
/// the last 24 hours, and observations plus jobs not processed yet; `None` when it cannot be read.
fn spent(cfg: &RunConfig, work: &Path, state: &Path) -> Option<(u64, u64, u64)> {
    let out = Command::new(&cfg.tools.broker)
        .args(["memory", "status", "--workspace"])
        .arg(work)
        .arg("--json")
        .env("XDG_STATE_HOME", state)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    Some((
        v["attempts_24h"].as_u64()?,
        v["questions_24h"].as_u64()?,
        v["pending"].as_u64()? + v["jobs_pending"].as_u64()?,
    ))
}

fn record(task: &Task, arm: Arm, repeat: u32, s: &Summary, sc: &score::Score) -> RunRecord {
    let read = s.memory_read.unwrap_or_default();
    let memory = |v: u64| arm.memory().then_some(v);
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
        history_incomplete: false,
        agent: s.agent.clone(),
        context_for_task_ms: (s.context_calls > 0).then_some(s.context_ms),
        memory_retrieval_requests: memory(read.requests),
        memory_retrieval_questions: memory(read.questions),
        memories_delivered: memory(read.delivered),
        memory_ingestion_attempts: None,
        memory_ingestion_questions: None,
        memory_jobs_left: None,
    }
}

/// One session in the round's place `dir`: the copy (`work`, made afresh at the task's base, in
/// the same path for every session of a sequence) and the server's state (`state`).
fn one(cfg: &RunConfig, task: &Task, arm: Arm, repeat: u32, dir: &Path) -> RunRecord {
    let failed = |why: String| RunRecord {
        task: task.id.clone(),
        repo: task.repo_name(),
        arm: arm.name().into(),
        repeat,
        vocabulary_diverges: task.vocabulary_diverges,
        invalid: Some(why),
        ..RunRecord::default()
    };
    let work = dir.join("work");
    let _ = std::fs::remove_dir_all(&work);
    let id = run_id(task, arm, repeat);
    let env = environment(task, &id);
    let result = (|| {
        checkout_base(task, &work)?;
        let logs = cfg.out.join("transcripts");
        let name = format!("{}__{}__{repeat}", task.id, arm.name());
        let setup_log = logs.join(format!("{name}.setup.log"));
        if let Some(setup) = &task.setup
            && !score::passes(
                shell(task, &id, &work, &env, setup),
                cfg.check_timeout,
                Some(&setup_log),
            )
        {
            return Err(format!("setup failed (see {})", setup_log.display()));
        }
        // What setup left in the tree (builds, caches) is not the agent's edit, unless the agent
        // then changed it again: each such file is remembered by its content.
        let before_agent: std::collections::HashMap<String, Option<Vec<u8>>> =
            score::modified_files(&work, &task.base)
                .into_iter()
                .map(|f| {
                    let state = score::state(&work, &f);
                    (f, state)
                })
                .collect();
        let config = dir.join("mcp.json");
        let state = dir.join("state");
        let before = arm.memory().then(|| spent(cfg, &work, &state)).flatten();
        std::fs::write(
            &config,
            arm.mcp_config(&cfg.tools, &work, &state).to_string(),
        )
        .map_err(|e| e.to_string())?;
        let argv: Vec<String> = cfg
            .agent
            .iter()
            .map(|a| {
                a.replace("{mcp_config}", &config.to_string_lossy())
                    .replace("{workdir}", &work.to_string_lossy())
            })
            .collect();
        let transcripts = &logs;
        let run = run_agent(
            &argv,
            &env,
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
        let modified: Vec<String> = score::modified_files(&work, &task.base)
            .into_iter()
            .filter(|f| {
                before_agent
                    .get(f)
                    .is_none_or(|was| *was != score::state(&work, f))
            })
            .collect();
        let correct = task.check.as_deref().map(|c| {
            score::passes(
                shell(task, &id, &work, &env, c),
                cfg.check_timeout,
                Some(&logs.join(format!("{name}.check.log"))),
            )
        });
        let sc = score::score(task, &s, &modified, correct);
        let mut r = record(task, arm, repeat, &s, &sc);
        // Ingestion: what the store spent during the session, less what its reads sent.
        if let Some((b, a)) = before.and_then(|b| Some((b, spent(cfg, &work, &state)?))) {
            let read = s.memory_read.unwrap_or_default();
            r.memory_ingestion_attempts = Some(a.0.saturating_sub(b.0 + read.requests));
            r.memory_ingestion_questions = Some(a.1.saturating_sub(b.1 + read.questions));
            r.memory_jobs_left = Some(a.2);
        }
        Ok::<_, String>(r)
    })();
    // Whatever happened after the copy existed: a database left behind by a failed run is still
    // a database left behind.
    if let Some(teardown) = &task.teardown
        && work.exists()
    {
        let _ = score::passes(
            shell(task, &id, &work, &env, teardown),
            cfg.check_timeout,
            None,
        );
    }
    result.unwrap_or_else(failed)
}

/// The corpus as rounds: each task alone, or the tasks of one sequence together, in corpus
/// order (a sequence where its first task is).
fn rounds(tasks: &[Task]) -> Vec<Vec<&Task>> {
    let mut out: Vec<Vec<&Task>> = vec![];
    for t in tasks {
        let joins = t
            .sequence
            .as_ref()
            .and_then(|s| out.iter_mut().find(|r| r[0].sequence.as_ref() == Some(s)));
        match joins {
            Some(round) => round.push(t),
            None => out.push(vec![t]),
        }
    }
    out
}

/// The first line `program --version` prints, or `unavailable`.
fn version_of(program: &Path) -> String {
    Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(|l| l.trim().to_string())
        })
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| "unavailable".into())
}

/// What the runs were made with (PRD jev-mem §14): the executables, the classifier model the
/// arms' servers pin, and the summarizer, which no arm configures. The agent's version is each
/// run's own, from its transcript: running the agent's command outside a session is a run.
fn versions(cfg: &RunConfig) -> serde_json::Value {
    json!({
        "ripwire_broker": version_of(&cfg.tools.broker),
        "ripwire": version_of(&cfg.tools.ripwire),
        "jev_model": crate::memory::runtime::DEFAULT_MODEL,
        "summarizer": "none",
    })
}

/// Runs what `results.jsonl` does not have yet; returns how many runs it made.
pub fn run(cfg: &RunConfig, log: &mut dyn FnMut(&str)) -> Result<usize, String> {
    cfg.corpus.validate().map_err(|e| e.join("\n"))?;
    let online: Vec<&str> = cfg
        .arms
        .iter()
        .filter(|a| a.needs_credential())
        .map(|a| a.name())
        .collect();
    if !online.is_empty() && std::env::var_os(ONLINE_KEY).is_none() {
        return Err(format!(
            "arm(s) {} need {ONLINE_KEY} in the environment, and consent to send eligible \
             source of every corpus repository to the provider (PRD §23.6)",
            online.join(", ")
        ));
    }
    std::fs::create_dir_all(cfg.out.join("transcripts")).map_err(|e| e.to_string())?;
    let versions = versions(cfg);
    let recorded = cfg.out.join("versions.json");
    // One set of binaries per results file: a resume with others would mix them unseen.
    match std::fs::read_to_string(&recorded)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    {
        Some(old) if old != versions => {
            return Err(format!(
                "{} was written by other binaries ({old}), not these ({versions}): use a new --out",
                recorded.display()
            ));
        }
        Some(_) => {}
        None => std::fs::write(&recorded, versions.to_string()).map_err(|e| e.to_string())?,
    }
    let done: std::collections::HashSet<_> =
        report::load(&cfg.out).iter().map(RunRecord::key).collect();
    let mut results = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cfg.out.join("results.jsonl"))
        .map_err(|e| e.to_string())?;
    let rounds = rounds(&cfg.corpus.tasks);
    let recorded = |task: &Task, arm: Arm, repeat: u32| {
        done.contains(&(task.id.clone(), arm.name().to_string(), repeat))
    };
    // A round is recorded whole, at its end. One recorded in part (a session added to a
    // sequence, an interrupted write) cannot be resumed: the rest would start without its history.
    for round in &rounds {
        for repeat in 1..=cfg.repeats {
            for &arm in &cfg.arms {
                let (have, missing): (Vec<&&Task>, Vec<&&Task>) =
                    round.iter().partition(|t| recorded(t, arm, repeat));
                if !have.is_empty() && !missing.is_empty() {
                    let ids = |ts: &[&&Task]| {
                        ts.iter()
                            .map(|t| t.id.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    return Err(format!(
                        "sequence {} · {} · {repeat}: recorded for {} but not for {}; remove its \
                         lines from results.jsonl to run it whole",
                        round[0].sequence.as_deref().unwrap_or(&round[0].id),
                        arm.name(),
                        ids(&have),
                        ids(&missing)
                    ));
                }
            }
        }
    }
    let mut made = 0;
    for round in rounds {
        for repeat in 1..=cfg.repeats {
            for &arm in &cfg.arms {
                if recorded(round[0], arm, repeat) {
                    continue;
                }
                let dir = scratch()?;
                let mut records: Vec<RunRecord> = vec![];
                for task in &round {
                    log(&format!("{} · {} · {repeat}", task.id, arm.name()));
                    let mut r = one(cfg, task, arm, repeat, &dir);
                    if let Some(why) = &r.invalid {
                        log(&format!("  invalid: {why}"));
                    }
                    r.history_incomplete = records.iter().any(|p| !p.valid);
                    records.push(r);
                }
                let _ = std::fs::remove_dir_all(&dir);
                for r in records {
                    let line = serde_json::to_string(&r).map_err(|e| e.to_string())?;
                    writeln!(results, "{line}").map_err(|e| e.to_string())?;
                    made += 1;
                }
            }
        }
    }
    Ok(made)
}

/// Whether a task's check tells its base from its fix: it must fail on a copy at the base and
/// pass on a copy at the fix, each prepared by the task's `setup`. A check that passes at the base
/// measures nothing; one that fails at the fix cannot be met.
fn verify(task: &Task, timeout: Duration, logs: &Path) -> Result<(), String> {
    let (Some(check), Some(fix)) = (&task.check, &task.fix) else {
        return Err(
            "no check or no fix: it can still run, but its correctness is not measured".into(),
        );
    };
    for (rev, label, must_pass) in [(&task.base, "base", false), (fix, "fix", true)] {
        let dir = scratch()?;
        let work = dir.join("work");
        let id = run_id(task, Arm::None, 0).replace("__none__0", &format!("__validate_{label}"));
        let env = environment(task, &id);
        let log = |step: &str| logs.join(format!("{}.{label}.{step}.log", task.id));
        let outcome = (|| {
            checkout(task, rev, &work)?;
            if let Some(setup) = &task.setup
                && !score::passes(
                    shell(task, &id, &work, &env, setup),
                    timeout,
                    Some(&log("setup")),
                )
            {
                return Err(format!(
                    "setup failed at the {label} (see {})",
                    log("setup").display()
                ));
            }
            Ok(score::passes(
                shell(task, &id, &work, &env, check),
                timeout,
                Some(&log("check")),
            ))
        })();
        if let Some(teardown) = &task.teardown
            && work.exists()
        {
            let _ = score::passes(shell(task, &id, &work, &env, teardown), timeout, None);
        }
        let _ = std::fs::remove_dir_all(&dir);
        let see = log("check").display().to_string();
        match (outcome?, must_pass) {
            (true, false) => {
                return Err(format!(
                    "the check passes at the base: it measures nothing (see {see})"
                ));
            }
            (false, true) => {
                return Err(format!(
                    "the check fails at the fix: it cannot be met (see {see})"
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Every task, verified: `(id, Ok(()))`, or why the task is broken or unverifiable. A task with no
/// check or no fix is reported, not refused: it still measures cost and recall. The output of each
/// setup and check goes to `logs`, as `<id>.<base|fix>.<setup|check>.log`.
pub fn validate(
    corpus: &Corpus,
    timeout: Duration,
    logs: &Path,
    log: &mut dyn FnMut(&str),
) -> Vec<(String, Result<(), String>)> {
    let _ = std::fs::create_dir_all(logs);
    corpus
        .tasks
        .iter()
        .map(|t| {
            log(&format!("{} …", t.id));
            (t.id.clone(), verify(t, timeout, logs))
        })
        .collect()
}
