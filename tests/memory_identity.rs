//! Seam: identity and time of a memory (PRD jev-mem §5.3, §5.4). Workspace, entities and
//! hashes are pure functions of what they name; nothing here reads a clock.
mod common;

use ripwire_broker::memory::identity::{self, Symbol};
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

fn symbol<'a>(name: &'a str) -> Symbol<'a> {
    Symbol {
        language: "python",
        qualified_name: name,
        signature: "(token)",
        revision: "sha256:abc",
    }
}

#[test]
fn homonymous_symbols_in_different_files_never_share_an_entity() {
    let ws = "w".repeat(64);
    let a = identity::symbol_entity(&ws, "src/a.py", Some(&symbol("parse")));
    let b = identity::symbol_entity(&ws, "src/b.py", Some(&symbol("parse")));
    assert_eq!(a.kind, EntityKind::Symbol);
    assert_ne!(a.id, b.id, "two `parse` in distinct files never coincide");

    let other_ws = "v".repeat(64);
    let c = identity::symbol_entity(&other_ws, "src/a.py", Some(&symbol("parse")));
    assert_ne!(a.id, c.id, "nor across workspaces");
    assert_ne!(
        identity::file_entity(&ws, "src/a.py").id,
        identity::file_entity(&other_ws, "src/a.py").id,
        "a file entity belongs to its workspace"
    );
    assert_eq!(
        a.id,
        identity::symbol_entity(&ws, "src/a.py", Some(&symbol("parse"))).id
    );
}

#[test]
fn a_symbol_without_an_unambiguous_descriptor_falls_back_to_the_file_entity() {
    let ws = "w".repeat(64);
    let file = identity::file_entity(&ws, "src/a.py");
    assert_eq!(file.kind, EntityKind::File);
    assert_eq!(identity::symbol_entity(&ws, "src/a.py", None), file);
    for blank in [
        Symbol {
            language: "",
            ..symbol("parse")
        },
        Symbol {
            qualified_name: "",
            ..symbol("parse")
        },
        Symbol {
            signature: "",
            ..symbol("parse")
        },
        Symbol {
            revision: "",
            ..symbol("parse")
        },
    ] {
        assert_eq!(
            identity::symbol_entity(&ws, "src/a.py", Some(&blank)),
            file,
            "never a fabricated symbol: {blank:?}"
        );
    }
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
