//! Seam: identity and time of a memory (PRD jev-mem §5.3, §5.4). Workspace, entities and
//! hashes are pure functions of what they name; nothing here reads a clock.
mod common;

use ripwire_broker::memory::identity;
use ripwire_broker::memory::model::{EntityKind, Kind, Record};
use serde_json::json;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

#[test]
fn two_worktrees_of_one_repository_have_different_workspace_ids() {
    let repo = common::sample_repo();
    let other = tempfile::tempdir().unwrap();
    let wt = other.path().join("wt");
    git(
        repo.path(),
        &["worktree", "add", "-q", wt.to_str().unwrap()],
    );

    let main = identity::workspace_id(repo.path()).unwrap();
    let linked = identity::workspace_id(&wt).unwrap();
    assert_ne!(
        main, linked,
        "same repository and HEAD, different worktrees"
    );
    assert_eq!(main, identity::workspace_id(repo.path()).unwrap(), "stable");
    assert_eq!(main.len(), 64, "SHA-256 in hex");

    // A subdirectory is another root, and a plain directory still has an identity.
    let sub = identity::workspace_id(&repo.path().join("src")).unwrap();
    assert_ne!(sub, main);
    let plain = tempfile::tempdir().unwrap();
    assert!(
        identity::workspace_id(plain.path()).is_ok(),
        "no git needed"
    );
    assert!(identity::workspace_id(&plain.path().join("missing")).is_err());
}

#[test]
fn a_worktree_recreated_at_the_same_path_for_another_repository_is_another_workspace() {
    let (a, b) = (common::sample_repo(), common::sample_repo());
    let other = tempfile::tempdir().unwrap();
    let wt = other.path().join("wt");
    let path = wt.to_str().unwrap();

    git(a.path(), &["worktree", "add", "-q", path]);
    let first = identity::workspace_id(&wt).unwrap();
    git(a.path(), &["worktree", "remove", path]);
    git(b.path(), &["worktree", "add", "-q", path]);
    let second = identity::workspace_id(&wt).unwrap();

    assert_ne!(first, second, "memory is never imported by reusing a path");
}

#[test]
fn a_file_entity_belongs_to_its_workspace() {
    let ws = "w".repeat(64);
    let other_ws = "v".repeat(64);
    let file = identity::file_entity(&ws, "src/a.py");
    assert_eq!(file.kind, EntityKind::File);
    assert_eq!(file, identity::file_entity(&ws, "src/a.py"));
    assert_ne!(file.id, identity::file_entity(&other_ws, "src/a.py").id);
    assert_ne!(file.id, identity::file_entity(&ws, "src/b.py").id);
}

#[test]
fn a_rename_without_evidence_yields_a_new_entity() {
    let ws = "w".repeat(64);
    let before = identity::file_entity(&ws, "src/old.py");
    let after = identity::file_entity(&ws, "src/new.py");
    assert_ne!(before.id, after.id, "no fusion by content similarity");
}

fn record() -> Record {
    serde_json::from_value(json!({
        "schema_version": 1,
        "policy_version": "memory-policy/v1",
        "node_id": "",
        "content_hash": "",
        "workspace_id": "w".repeat(64),
        "event_key": "e1",
        "kind": "edit_observation",
        "content": "Evento: análise após edição. Escopo: src/cache.rs.",
        "observed_at_ms": 1_000,
        "ingest_seq": 1,
        "timestamp_role": "observation",
        "sources": [{"path": "src/cache.rs", "sha256": "a".repeat(64), "verb": "quality_delta", "basis": "ripwire"}],
        "expires_at_ms": 2_000,
        "generation": 1
    }))
    .unwrap()
}

#[test]
fn the_node_id_ignores_clock_scores_and_retries() {
    let r = record();
    let hash = identity::content_hash(&r);
    let id = identity::node_id(&r.workspace_id, r.kind, &hash);
    assert_eq!(hash.len(), 64);

    let mut later = r.clone();
    later.observed_at_ms = 9_999;
    later.ingest_seq = 42;
    later.expires_at_ms = 99_999;
    later.generation = 7;
    later.event_key = "another session".into();
    later.types.episodic = Some(0.9);
    later.enrichment.prompt_version = Some("memory-prompts/v1".into());
    assert_eq!(
        identity::content_hash(&later),
        hash,
        "replay is the same node"
    );

    let mut changed = r.clone();
    changed.sources[0].sha256 = "b".repeat(64);
    assert_ne!(identity::content_hash(&changed), hash, "a changed source");
    let mut changed = r.clone();
    changed.content.push('!');
    assert_ne!(identity::content_hash(&changed), hash, "another outcome");

    assert_ne!(
        identity::node_id(&r.workspace_id, Kind::FinishObservation, &hash),
        id,
        "the kind is part of the node"
    );
    assert_ne!(identity::node_id(&"v".repeat(64), r.kind, &hash), id);
}

// ---------------------------------------------------------------- time (PRD jev-mem §5.4)

use ripwire_broker::memory::admission::{self, Draft, Event, Outcome, Tests};
use ripwire_broker::memory::model::TimestampRole;
use ripwire_broker::memory::time::{Clock, Sequence};
use ripwire_broker::online::reader::WorkspaceReader;
use std::cell::RefCell;

/// A wall clock that answers what it is told, in order.
struct Script(RefCell<Vec<u64>>);

impl Clock for Script {
    fn now_ms(&self) -> u64 {
        self.0.borrow_mut().remove(0)
    }
}

/// The next stamp of `seq`, read from `clock`: what the store does when it admits.
fn stamp(seq: &mut Sequence, clock: &dyn Clock, retention_ms: u64) -> admission::Stamp {
    admission::Stamp {
        observed_at_ms: clock.now_ms(),
        ingest_seq: seq.advance().unwrap(),
        generation: 1,
        retention_ms,
    }
}

#[test]
fn mtime_commit_and_a_clock_rollback_never_become_event_time() {
    let repo = common::sample_repo();
    let root = repo.path();
    let reader = WorkspaceReader::new(root).unwrap();
    let ws = identity::workspace_id(root).unwrap();
    let draft = Draft {
        event_key: "s/e".into(),
        event: Event::AfterEdit,
        outcome: Outcome::AnalysisCompleted,
        tests: Tests::Unknown,
        scope: vec!["src/auth.py".into()],
        evidence: vec![],
    };
    let clock = Script(RefCell::new(vec![5_000, 5_000, 1_000]));
    let mut seq = Sequence::default();
    let day = 24 * 60 * 60 * 1000;

    let first = admission::admit(&reader, &ws, &draft, stamp(&mut seq, &clock, day)).unwrap();
    assert_eq!(first.timestamp_role, TimestampRole::Observation);
    assert_eq!(
        first.event_time, None,
        "an observation time is not an event time"
    );
    assert_eq!((first.observed_at_ms, first.ingest_seq), (5_000, 1));

    // An old mtime and a commit dated in the past change nothing about time.
    let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(86_400);
    std::fs::File::options()
        .write(true)
        .open(root.join("src/auth.py"))
        .unwrap()
        .set_modified(old)
        .unwrap();
    let ok = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "user.email=t@t", "-c", "user.name=t"])
        .args([
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "old",
            "--date",
            "2001-01-01T00:00:00",
        ])
        .env("GIT_COMMITTER_DATE", "2001-01-01T00:00:00")
        .status()
        .unwrap()
        .success();
    assert!(ok);
    let mut replay = Sequence::default();
    let again = admission::admit(&reader, &ws, &draft, stamp(&mut replay, &clock, day)).unwrap();
    assert_eq!(again, first, "same node, same time fields");

    // The wall clock goes back: the sequence still grows.
    let mut later = seq.clone();
    let back = stamp(&mut later, &clock, day);
    assert_eq!((back.observed_at_ms, back.ingest_seq), (1_000, 2));
}
