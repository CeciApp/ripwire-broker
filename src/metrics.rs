//! Local, in-memory metrics (PRD 16.1). Counts and durations only: never prompts, code,
//! paths, symbols or response bodies.

use crate::model::{Envelope, Status};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

#[derive(Debug, Default, Clone, Serialize)]
pub struct ToolMetrics {
    pub calls: u64,
    pub errors: u64,
    pub total_us: u64,
    pub requested_tokens: u64,
    pub estimated_tokens: u64,
    pub items_shown: u64,
    pub items_omitted: u64,
    pub ready: u64,
    pub attention_required: u64,
    pub unknown: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Metrics {
    pub tools: BTreeMap<&'static str, ToolMetrics>,
    pub upstream_calls: u64,
    pub upstream_us: u64,
    /// Upstream verb names are low-sensitivity routing data (PRD 16.1).
    pub upstream_verbs: BTreeMap<&'static str, u64>,
    pub truncated_responses: u64,
    /// Items, tests and risks not sent in full because the session already had them
    /// (the "logical cache hit" of PRD 16.1).
    pub session_hits: u64,
    pub session: SessionStats,
    /// The last `RECENT_REQUESTS` tool calls with their upstream calls (PRD 14.2 correlation).
    pub recent_requests: VecDeque<RequestRecord>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct SessionStats {
    /// How many fingerprints the session holds; never their content.
    pub remembered: usize,
}

pub const RECENT_REQUESTS: usize = 32;

/// One upstream call made on behalf of a tool call: verb, duration and outcome only.
#[derive(Debug, Clone, Serialize)]
pub struct UpstreamSpan {
    pub verb: &'static str,
    pub us: u64,
    /// `ok` or the error kind (`upstream_refused`...).
    pub outcome: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestRecord {
    pub request_id: u64,
    pub tool: &'static str,
    /// The envelope status, or the error kind.
    pub outcome: &'static str,
    pub total_us: u64,
    pub upstream: Vec<UpstreamSpan>,
}

impl Metrics {
    pub fn upstream(&mut self, verb: &'static str, took: Duration) {
        self.upstream_calls += 1;
        self.upstream_us += took.as_micros() as u64;
        *self.upstream_verbs.entry(verb).or_default() += 1;
    }

    pub fn request(&mut self, record: RequestRecord) {
        if self.recent_requests.len() == RECENT_REQUESTS {
            self.recent_requests.pop_front();
        }
        self.recent_requests.push_back(record);
    }

    pub fn tool<E>(&mut self, tool: &'static str, took: Duration, result: &Result<Envelope, E>) {
        let m = self.tools.entry(tool).or_default();
        m.calls += 1;
        m.total_us += took.as_micros() as u64;
        let Ok(env) = result else {
            m.errors += 1;
            return;
        };
        m.requested_tokens += env.budget.requested_tokens as u64;
        m.estimated_tokens += env.budget.estimated_tokens as u64;
        m.items_shown += env.budget.shown as u64;
        m.items_omitted += env.budget.omitted as u64;
        match env.status {
            Status::Ready => m.ready += 1,
            Status::AttentionRequired => m.attention_required += 1,
            Status::Unknown => m.unknown += 1,
        }
        if env.budget.truncated {
            self.truncated_responses += 1;
        }
    }
}
