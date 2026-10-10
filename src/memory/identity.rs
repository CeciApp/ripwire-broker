//! Identity of a memory (PRD jev-mem §5.3): the workspace it belongs to, the entities it names
//! and the hashes that make a replay the same node. Pure functions, apart from reading the
//! `.git` pointers of a workspace; nothing here reads a clock.

use super::model::{
    Assertion, Entity, EntityKind, EventTime, Kind, Parent, Record, Revision, Source,
    TemporalReference, TimestampRole,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// SHA-256 in hex over the components, each preceded by its length: moving a boundary between
/// components is another identity.
/// The crate's one framed SHA-256: each part prefixed by its length, so no two lists of parts
/// collide. Note keys and session fingerprints use it too.
pub fn hash(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update((p.len() as u64).to_le_bytes());
        h.update(p.as_bytes());
    }
    format!("{:x}", h.finalize())
}

/// The canonical root, the worktree's git-dir and the repository's common dir; a root outside
/// git is identified by itself. Never by a remote URL: two worktrees of one repository at the
/// same HEAD keep separate memories.
pub fn workspace_id(root: &Path) -> Result<String, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("workspace {}: {e}", root.display()))?;
    let text = |p: &Path| {
        p.to_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{} is not UTF-8", p.display()))
    };
    let id = match git_dirs(&root) {
        Some((git_dir, common)) => hash(&[
            "memory/v1/workspace",
            &text(&root)?,
            &text(&git_dir)?,
            &text(&common)?,
        ]),
        None => hash(&["memory/v1/workspace/plain", &text(&root)?]),
    };
    Ok(id)
}

/// The nearest `.git` at or above `root`: a directory, or a worktree's `gitdir:` file.
fn git_dirs(root: &Path) -> Option<(PathBuf, PathBuf)> {
    let dot = root
        .ancestors()
        .map(|d| d.join(".git"))
        .find(|p| p.exists())?;
    let git_dir = match dot.is_dir() {
        true => dot,
        false => {
            let text = std::fs::read_to_string(&dot).ok()?;
            let target = text.strip_prefix("gitdir:")?.trim();
            dot.parent()?.join(target)
        }
    };
    let git_dir = git_dir.canonicalize().ok()?;
    let common = match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(rel) => git_dir.join(rel.trim()).canonicalize().ok()?,
        Err(_) => git_dir.clone(),
    };
    Some((git_dir, common))
}

/// `path` is relative to the workspace root and already normalized.
pub fn file_entity(workspace_id: &str, path: &str) -> Entity {
    Entity {
        id: hash(&["memory/v1/entity", workspace_id, "file", path]),
        kind: EntityKind::File,
        path: path.into(),
        revision: None,
    }
}

/// The semantic fields of a record: everything but the ingestion clock, the scores, the retry
/// state, retention and the session's event key.
#[derive(Serialize)]
struct Semantic<'a> {
    kind: Kind,
    content: &'a str,
    event_time: &'a Option<EventTime>,
    timestamp_role: TimestampRole,
    temporal_references: &'a [TemporalReference],
    entities: &'a [Entity],
    sources: &'a [Source],
    revision: &'a Option<Revision>,
    assertion: &'a Option<Assertion>,
    derived_from: &'a [Parent],
}

pub fn content_hash(r: &Record) -> String {
    let semantic = Semantic {
        kind: r.kind,
        content: &r.content,
        event_time: &r.event_time,
        timestamp_role: r.timestamp_role,
        temporal_references: &r.temporal_references,
        entities: &r.entities,
        sources: &r.sources,
        revision: &r.revision,
        assertion: &r.assertion,
        derived_from: &r.derived_from,
    };
    let canonical = serde_json::to_string(&semantic).unwrap_or_default();
    hash(&["memory/v1/content", &canonical])
}

/// `H(memory/v1, workspace_id, kind, content_hash)`.
pub fn node_id(workspace_id: &str, kind: Kind, content_hash: &str) -> String {
    let kind = match kind {
        Kind::EditObservation => "edit_observation",
        Kind::FinishObservation => "finish_observation",
        Kind::ExplicitNote => "explicit_note",
        Kind::DerivedNote => "derived_note",
    };
    hash(&["memory/v1", workspace_id, kind, content_hash])
}
