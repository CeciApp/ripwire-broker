//! The status line's projection (PRD §24.6.3): a small private file per host session
//! and workspace, written by the hooks after they save their state, read by `statusline` without a
//! lock. Counts and kinds only: no prompt, code, plain path, symbol or fingerprint.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub host: String,
    pub workspace_key: String,
    pub updated_at: u64,
    pub opted_out: bool,
    pub stats: VisibleStats,
    pub last_analysis: Option<Analysis>,
    pub last_delivery: Option<Delivery>,
}

/// The session's tally since this workspace was bound to it (`SessionTally - baseline`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VisibleStats {
    pub events: u64,
    pub injections: u64,
    pub delivered: u64,
    pub session_hits: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub at: u64,
    pub event: String,
    pub status: AnalysisStatus,
    /// A `BrokerError::error` kind (a fixed vocabulary), never a message.
    pub error_kind: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    Ready,
    AttentionRequired,
    Unknown,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delivery {
    pub at: u64,
    pub estimated_tokens: u32,
}
