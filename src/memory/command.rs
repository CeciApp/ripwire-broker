//! `ripwire-broker memory …`: local operations on a workspace's store. None of them builds a
//! classifier client, starts ripwire or needs a credential (PRD jev-mem §4).

use super::admission::{self, Note, Stamp};
use super::identity;
use super::store::{Refusal, Store};
use super::time::{Clock, SystemClock};
use crate::cli::{MemoryAction, MemoryCommand};
use crate::online::reader::WorkspaceReader;
use crate::state::StateStore;
use serde_json::json;

const DAY_MS: u64 = 24 * 60 * 60 * 1000;
/// `forget` does not know the retention the server runs with, so a tombstone lasts the longest
/// one allowed (`--memory-retention-days` 365).
const TOMBSTONE_MS: u64 = 365 * DAY_MS;
/// `memory add` keeps a note for the default retention.
const NOTE_RETENTION_MS: u64 = 30 * DAY_MS;

/// A category for stderr; never a path or content.
fn category(r: Refusal) -> String {
    match r {
        Refusal::Unavailable(u) => format!("store unavailable ({})", u.as_str()),
        Refusal::Full(f) => format!("store full ({f:?})"),
        Refusal::Locked => "another process holds the store; try again".into(),
        Refusal::InvalidId => "not a memory id".into(),
        Refusal::Crashed => "interrupted".into(),
        Refusal::Revoked => "collection is revoked: run `memory resume` first".into(),
    }
}

/// What to print on success, or the error for stderr.
pub fn run(cmd: &MemoryCommand) -> Result<String, String> {
    let dir = cmd
        .state_dir
        .clone()
        .or_else(StateStore::default_dir)
        .ok_or("no state directory: pass --state-dir")?;
    let workspace_id = identity::workspace_id(&cmd.workspace)?;
    let store = Store::new(&dir, &workspace_id);
    let now = SystemClock.now_ms();
    match &cmd.action {
        MemoryAction::Status { json } => Ok(status(&store, *json)),
        MemoryAction::Forget { id } => {
            let n = store
                .forget(id, now.saturating_add(TOMBSTONE_MS))
                .map_err(|r| format!("memory forget: {}", category(r)))?;
            Ok(format!(
                "forgot {n} memories; the id stays blocked for 365 days"
            ))
        }
        MemoryAction::ForgetAll => {
            let n = store
                .forget_all(now.saturating_add(TOMBSTONE_MS))
                .map_err(|r| format!("memory forget: {}", category(r)))?;
            Ok(format!(
                "forgot {n} memories; collection is revoked until `memory resume`"
            ))
        }
        MemoryAction::Add { file } => {
            let refused = |why: &str| format!("memory add: refused ({why})");
            // A file inside the workspace passes the whole policy on its path from the root
            // (hidden and dependency directories included); one outside, on its own name.
            let file = std::path::absolute(file).map_err(|_| refused("outside"))?;
            let root = cmd
                .workspace
                .canonicalize()
                .map_err(|e| format!("workspace {}: {e}", cmd.workspace.display()))?;
            let parent = file.parent().ok_or_else(|| refused("outside"))?;
            let parent = parent.canonicalize().map_err(|_| refused("unreadable"))?;
            let name = file
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| refused("outside"))?;
            let input = match parent.strip_prefix(&root) {
                Ok(rel) => WorkspaceReader::new(&root)?.snapshot(&rel.join(name).to_string_lossy()),
                Err(_) => WorkspaceReader::new(&parent)?.snapshot(name),
            }
            .map_err(|why| refused(why.as_str()))?;
            let note: Note = serde_json::from_str(input.preview_at(usize::MAX))
                .map_err(|_| refused("malformed"))?;
            let stamp = Stamp {
                observed_at_ms: now,
                ingest_seq: 0,
                generation: 0,
                retention_ms: NOTE_RETENTION_MS,
            };
            let reader = WorkspaceReader::new(&cmd.workspace)?;
            let record = admission::admit_note(&reader, &workspace_id, &note, stamp)
                .map_err(|why| refused(why.as_str()))?;
            store
                .enqueue(&record)
                .map_err(|r| format!("memory add: {}", category(r)))?;
            // Incorporated now when nobody else holds the store; pending otherwise.
            let _ = store.ingest();
            Ok("added 1 note".into())
        }
        // Needs the provider: the binary runs it, with the `online` feature (PD-2).
        MemoryAction::Drain => Err("memory drain is run by the server binary".into()),
        MemoryAction::Retry => {
            let n = store
                .retry_all_failed()
                .map_err(|r| format!("memory retry: {}", category(r)))?;
            Ok(format!("brought back {n} failed job(s)"))
        }
        MemoryAction::Resume => match store.resume() {
            Ok(true) => Ok("memory collection resumed for this workspace".into()),
            Ok(false) => Ok("memory collection was not revoked for this workspace".into()),
            Err(r) => Err(format!("memory resume: {}", category(r))),
        },
    }
}

fn status(store: &Store, as_json: bool) -> String {
    let state = store.load();
    let usage = store.usage().unwrap_or_default();
    let error = state.as_ref().err().map(|u| u.as_str());
    let state = state.unwrap_or_default();
    let v = json!({
        "schema_version": super::store::SCHEMA_VERSION,
        "error": error,
        "revoked": store.is_revoked(),
        "generation": state.generation,
        "nodes": state.nodes.len(),
        "tombstones": state.tombstones.len(),
        "pending": usage.pending,
        "spool_bytes": usage.spool_bytes,
        "snapshot_bytes": usage.snapshot_bytes,
        "attempts_24h": state.ledger.used(SystemClock.now_ms()).0,
        "questions_24h": state.ledger.used(SystemClock.now_ms()).1,
    });
    if as_json {
        return serde_json::to_string_pretty(&v).unwrap_or_default();
    }
    let mut text = format!(
        "memory: {} memories, {} pending, generation {}, {} bytes",
        state.nodes.len(),
        usage.pending,
        state.generation,
        usage.snapshot_bytes + usage.spool_bytes
    );
    if let Some(e) = error {
        text.push_str(&format!("\nstore unavailable: {e}"));
    }
    if store.is_revoked() {
        text.push_str("\ncollection revoked: run `memory resume` to collect again");
    }
    text
}
