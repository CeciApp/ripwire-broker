//! The A/B instrument (PRD §16.2–16.4, §17, §23.15; D-116): a corpus of tasks with a reference
//! patch, run by a real agent in each arm, reduced to counts and scored against the PRD's bars.
//! It drives the agent from outside and never runs inside the broker: nothing here is reachable
//! from `serve`, the hooks or any other command of `ripwire-broker`. The binary is `ripwire-eval`.

pub mod arm;
pub mod corpus;
pub mod report;
pub mod runner;
pub mod score;
pub mod transcript;

use std::path::Path;
use std::process::{Command, Stdio};

/// `git -C dir args`, never reading the terminal: its stdout, or why it failed.
fn git(dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
