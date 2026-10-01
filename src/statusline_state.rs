//! The status line's projection (PRD §24.6.3): a small private file per host session
//! and workspace, written by the hooks after they save their state, read by `statusline` without a
//! lock. Counts and kinds only: no prompt, code, plain path, symbol or fingerprint.

use crate::hook::{SessionState, SessionTally};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read as _;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_SNAPSHOT_BYTES: u64 = 16 * 1024;
/// The only host that renders a status line today (D3).
pub const HOST: &str = "claude-code";

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

#[derive(Debug, Clone, PartialEq)]
pub enum Read {
    Missing,
    Corrupt,
    Incompatible,
    Valid(Snapshot),
}

/// What the hooks keep in the session state for the status line: the workspace it is bound to,
/// the tally at binding time, and the last analysis and delivery.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub workspace_key: String,
    /// `SessionTally` when this workspace was bound: the bar counts from here (spec §6.3).
    pub baseline: SessionTally,
    pub last_analysis: Option<Analysis>,
    pub last_delivery: Option<Delivery>,
}

/// Binds the session to a workspace. A new key (or a state saved before the status line existed)
/// starts a fresh visual baseline and forgets the summaries of the previous root (spec §6.3).
pub fn bind(state: &mut SessionState, workspace_key: &str) {
    if state
        .statusline
        .as_ref()
        .is_some_and(|s| s.workspace_key == workspace_key)
    {
        return;
    }
    state.statusline = Some(Summary {
        workspace_key: workspace_key.into(),
        baseline: state.stats.clone(),
        last_analysis: None,
        last_delivery: None,
    });
}

/// The projection of a bound state: counters since the binding, replaced whole on every write.
pub fn project(state: &SessionState, now: u64) -> Option<Snapshot> {
    let s = state.statusline.as_ref()?;
    let (t, b) = (&state.stats, &s.baseline);
    Some(Snapshot {
        schema_version: SCHEMA_VERSION,
        host: HOST.into(),
        workspace_key: s.workspace_key.clone(),
        updated_at: now,
        opted_out: state.opted_out,
        stats: VisibleStats {
            events: t.events.saturating_sub(b.events),
            injections: t.injections.saturating_sub(b.injections),
            delivered: t.delivered.saturating_sub(b.delivered),
            session_hits: t.session_hits.saturating_sub(b.session_hits),
        },
        last_analysis: s.last_analysis.clone(),
        last_delivery: s.last_delivery.clone(),
    })
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn workspace_key(root: &Path) -> String {
    hex(root.as_os_str().as_bytes())
}

/// sha256 over a length-prefixed tuple: no two (host, session, root) share an encoding, and no
/// session id ever becomes part of a path.
pub fn path(state_dir: &Path, host: &str, session_id: &str, root: &Path) -> PathBuf {
    let mut h = Sha256::new();
    for part in [
        b"ripwire-broker/statusline/v1".as_slice(),
        host.as_bytes(),
        session_id.as_bytes(),
        root.as_os_str().as_bytes(),
    ] {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part);
    }
    state_dir
        .join("statusline")
        .join(format!("{:x}.json", h.finalize()))
}

pub fn publish(
    state_dir: &Path,
    session_id: &str,
    root: &Path,
    snapshot: &Snapshot,
) -> std::io::Result<()> {
    let file = path(state_dir, &snapshot.host, session_id, root);
    let dir = file.parent().expect("statusline dir");
    crate::state::write_private(dir, &file, serde_json::to_string(snapshot)?.as_bytes())
}

/// Never waits for the hooks' lock and never creates anything.
pub fn read(state_dir: &Path, host: &str, session_id: &str, root: &Path) -> Read {
    let file = path(state_dir, host, session_id, root);
    match std::fs::symlink_metadata(&file) {
        Ok(m) if m.file_type().is_file() => {}
        _ => return Read::Missing,
    }
    let Ok(f) = std::fs::File::open(&file) else {
        return Read::Missing;
    };
    let mut text = String::new();
    if f.take(MAX_SNAPSHOT_BYTES + 1)
        .read_to_string(&mut text)
        .is_err()
        || text.len() as u64 > MAX_SNAPSHOT_BYTES
    {
        return Read::Corrupt;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Read::Corrupt;
    };
    if v.get("schema_version").and_then(serde_json::Value::as_u64)
        != Some(u64::from(SCHEMA_VERSION))
    {
        return Read::Incompatible;
    }
    let Ok(s) = serde_json::from_value::<Snapshot>(v) else {
        return Read::Corrupt;
    };
    if s.host != host || s.workspace_key != workspace_key(root) {
        return Read::Incompatible;
    }
    Read::Valid(s)
}
