//! The enrichment queue (PRD jev-mem §6, §8.2): one job per node, runs counted on disk, leases
//! that belong to whoever holds their lock file, and the 24-hour ledger of attempts and
//! questions, shared by every process of the workspace.

use serde::{Deserialize, Serialize};
use std::fs::File;

/// Runs of a job in all, retries of a write included; then it fails until asked again.
pub const MAX_RUNS: u32 = 2;
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    #[default]
    Pending,
    Leased,
    /// Out of runs; stays so until `retry_failed`.
    Failed,
    Done,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub state: JobState,
    /// Runs started, on disk: a restart does not give a job more.
    pub runs: u32,
    pub not_before_ms: u64,
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Done,
    /// Try again, not before then, if runs are left.
    Retry {
        not_before_ms: u64,
    },
    Failed,
    /// Nothing was sent (the quota is spent, the store was busy): back to pending, not before
    /// then, and the run is not counted.
    Defer {
        not_before_ms: u64,
    },
}

/// A job taken by this process. It is held through the lock on its lease file: when the
/// holder dies the lock goes with it, and another process can take the job again.
pub struct Lease {
    pub(crate) node_id: String,
    pub(crate) run: u32,
    /// When it was taken.
    pub(crate) at_ms: u64,
    pub(crate) _lock: File,
}

impl Lease {
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// 1 for the first run.
    pub fn run(&self) -> u32 {
        self.run
    }
}

/// Attempts and questions sent in the last 24 hours (PRD jev-mem §8.2). A clock that goes back
/// is read as the latest time seen, so it never frees quota.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    /// `(at_ms, attempts, questions)`.
    pub entries: Vec<(u64, u32, u32)>,
    pub high_water_ms: u64,
}

impl Ledger {
    /// Records the charge if it fits both caps; `false`, recording nothing, otherwise.
    pub fn charge(
        &mut self,
        now_ms: u64,
        attempts: u32,
        questions: u32,
        max_attempts: u32,
        max_questions: u32,
    ) -> bool {
        let now = now_ms.max(self.high_water_ms);
        self.high_water_ms = now;
        self.entries
            .retain(|(at, _, _)| at.saturating_add(DAY_MS) > now);
        let (used_attempts, used_questions) = self.used(now);
        if used_attempts.saturating_add(attempts) > max_attempts
            || used_questions.saturating_add(questions) > max_questions
        {
            return false;
        }
        self.entries.push((now, attempts, questions));
        true
    }
}

impl Ledger {
    /// Records what was already sent, whatever the caps: a request out is spent.
    pub fn record(&mut self, now_ms: u64, attempts: u32, questions: u32) {
        let now = now_ms.max(self.high_water_ms);
        self.high_water_ms = now;
        self.entries
            .retain(|(at, _, _)| at.saturating_add(DAY_MS) > now);
        self.entries.push((now, attempts, questions));
    }

    /// Attempts and questions charged in the 24 hours before `now_ms` (or the latest time seen).
    /// The sums saturate: a corrupt or hand-edited file with huge counts never wraps into free
    /// quota (D-151).
    pub fn used(&self, now_ms: u64) -> (u32, u32) {
        let now = now_ms.max(self.high_water_ms);
        self.entries
            .iter()
            .filter(|(at, _, _)| at.saturating_add(DAY_MS) > now)
            .fold((0u32, 0u32), |(a, q), e| {
                (a.saturating_add(e.1), q.saturating_add(e.2))
            })
    }
}
