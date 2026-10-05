//! One run scored against the task's reference patch (PRD §16.3 and §23.15).

use super::corpus::Task;
use super::transcript::Summary;
use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Score {
    /// Reference files the agent modified.
    pub file_recall: f64,
    /// Modified files that are in the reference; `None` when nothing was modified.
    pub file_precision: Option<f64>,
    /// Reference files the broker presented; `None` when no broker answered.
    pub presented_recall: Option<f64>,
    /// 1-based position of the first reference file among those presented.
    pub first_correct_rank: Option<usize>,
    /// Reference tests the agent was shown or ran; `None` when the reference names none.
    pub test_recall: Option<f64>,
    /// The task's `check` passed; `None` when the task has none.
    pub correct: Option<bool>,
}

fn fraction(hit: usize, of: usize) -> f64 {
    if of == 0 { 1.0 } else { hit as f64 / of as f64 }
}

pub fn score(task: &Task, s: &Summary, modified: &[String], correct: Option<bool>) -> Score {
    let reference = &task.reference.files;
    let is_ref = |f: &String| reference.contains(f);
    let hits = reference.iter().filter(|f| modified.contains(f)).count();
    let presented = &s.presented_files;
    let named_tests = task
        .reference
        .tests
        .iter()
        .filter(|t| s.presented_tests.contains(t) || s.commands.iter().any(|c| c.contains(*t)))
        .count();
    Score {
        file_recall: fraction(hits, reference.len()),
        file_precision: (!modified.is_empty()).then(|| {
            fraction(
                modified.iter().filter(|f| is_ref(f)).count(),
                modified.len(),
            )
        }),
        presented_recall: (!presented.is_empty()).then(|| {
            fraction(
                reference.iter().filter(|f| presented.contains(f)).count(),
                reference.len(),
            )
        }),
        first_correct_rank: presented.iter().position(is_ref).map(|p| p + 1),
        test_recall: (!task.reference.tests.is_empty())
            .then(|| fraction(named_tests, task.reference.tests.len())),
        correct,
    }
}

/// A file's state, to tell later whether it changed: the sha256 of its content, or `None` when it
/// does not exist.
pub fn state(workdir: &Path, file: &str) -> Option<Vec<u8>> {
    use sha2::{Digest, Sha256};
    std::fs::read(workdir.join(file))
        .ok()
        .map(|bytes| Sha256::digest(bytes).to_vec())
}

/// Files that differ from `base` in the working tree, whatever the agent committed meanwhile,
/// plus new untracked ones, relative to the root. NUL-separated, so no name comes back quoted.
pub fn modified_files(workdir: &Path, base: &str) -> Vec<String> {
    let names = |args: &[&str]| -> Vec<String> {
        super::git(workdir, args)
            .map(|stdout| {
                stdout
                    .split(|b| *b == 0)
                    .filter(|n| !n.is_empty())
                    .map(|n| String::from_utf8_lossy(n).into_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut files = names(&["diff", "--name-only", "-z", "--no-renames", base]);
    files.extend(names(&["ls-files", "-o", "--exclude-standard", "-z"]));
    files.sort();
    files.dedup();
    files
}

/// Kills `pid`'s process group: what a timed-out command started goes with it (a test runner
/// holding a database, a child holding a pipe open). Without `unsafe`, through the shell's own
/// `kill`, in its POSIX form: procps' `/usr/bin/kill` (Ubuntu 24.04) misreads `-KILL -<pgid>`
/// and can signal every process of the user, the CI runner included.
pub(crate) fn kill_group(pid: u32) {
    let _ = Command::new("sh")
        .args(["-c", "kill -s KILL -- -\"$1\"", "sh", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Runs `command` (a task's `setup`, `check` or `teardown`, already pointed at the copy): exit 0
/// within `timeout` passes, anything else, including a timeout, fails. Its stdout and stderr go
/// to `log`, or nowhere. It runs in a process group of its own, killed whole on a timeout.
pub fn passes(mut command: Command, timeout: Duration, log: Option<&Path>) -> bool {
    use std::os::unix::process::CommandExt as _;
    let file = log.and_then(|p| std::fs::File::create(p).ok());
    let (out, err) = match file
        .as_ref()
        .and_then(|f| Some((f.try_clone().ok()?, f.try_clone().ok()?)))
    {
        Some((o, e)) => (Stdio::from(o), Stdio::from(e)),
        None => (Stdio::null(), Stdio::null()),
    };
    let Ok(mut child) = command.stdout(out).stderr(err).process_group(0).spawn() else {
        return false;
    };
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                kill_group(child.id());
                let _ = child.wait();
                return false;
            }
        }
    }
}
