//! One run scored against the task's reference patch (PRD §16.3 and §23.15).

use super::corpus::Task;
use super::transcript::Summary;
use serde::Serialize;
use std::path::Path;
use std::process::Command;
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
    /// Reference tests the agent was shown or ran; 1.0 when the reference names none.
    pub test_recall: f64,
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
        test_recall: fraction(named_tests, task.reference.tests.len()),
        correct,
    }
}

/// Files changed in the working tree against `HEAD`, new ones included, relative to the root.
pub fn modified_files(workdir: &Path) -> Vec<String> {
    let Ok(out) = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .args(["status", "--porcelain", "-uall", "--no-renames"])
        .output()
    else {
        return vec![];
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.get(3..))
        .map(|p| p.trim_matches('"').to_string())
        .collect()
}

/// Runs a task's `check` in the clone: exit 0 within `timeout` is correct.
pub fn run_check(workdir: &Path, check: &str, timeout: Duration) -> bool {
    let Ok(mut child) = Command::new("sh")
        .args(["-c", check])
        .current_dir(workdir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return false;
    };
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}
