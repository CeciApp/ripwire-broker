//! What the memory worker costs, by operation (PRD jev-mem §8.3, §14): counts, bytes and
//! failure categories only, never a path, a text or the credential.

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Operation {
    /// Requests sent, retries included.
    pub attempts: u64,
    pub retries: u64,
    /// Questions sent, counted on every attempt.
    pub questions: u64,
    /// Request bodies sent.
    pub bytes_sent: u64,
    /// Failed attempts by category (`server`, `timeout`, `auth`...).
    pub failures: BTreeMap<&'static str, u64>,
    /// Requests not sent because the 24-hour budget was spent.
    pub quota_refusals: u64,
}

impl Operation {
    pub fn add(&mut self, other: &Operation) {
        self.attempts += other.attempts;
        self.retries += other.retries;
        self.questions += other.questions;
        self.bytes_sent += other.bytes_sent;
        self.quota_refusals += other.quota_refusals;
        for (k, v) in &other.failures {
            *self.failures.entry(k).or_default() += v;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Metrics {
    pub typing: Operation,
    pub relations: Operation,
    pub consolidation: Operation,
    /// Consolidation rounds run.
    pub rounds: u64,
    /// Jobs that finished their planned stages (complete or partial).
    pub jobs_done: u64,
    pub jobs_failed: u64,
}

impl Metrics {
    pub fn add(&mut self, run: &Metrics) {
        self.typing.add(&run.typing);
        self.relations.add(&run.relations);
        self.consolidation.add(&run.consolidation);
        self.rounds += run.rounds;
        self.jobs_done += run.jobs_done;
        self.jobs_failed += run.jobs_failed;
    }
}
