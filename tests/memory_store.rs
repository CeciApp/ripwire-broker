//! Seam: the memory store on disk (PRD jev-mem §6). Private, outside the repository, published
//! whole or not at all, and never overwritten when it cannot be read.
use ripwire_broker::memory::model::Record;
use ripwire_broker::memory::store::{State, Store, Unavailable};
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn record(n: u64) -> Record {
    serde_json::from_value(json!({
        "schema_version": 1,
        "policy_version": "memory-policy/v1",
        "node_id": format!("{n:064}"),
        "content_hash": format!("{n:064}"),
        "workspace_id": "w".repeat(64),
        "event_key": "e",
        "kind": "edit_observation",
        "content": format!("observation {n}"),
        "observed_at_ms": n,
        "ingest_seq": n,
        "timestamp_role": "observation",
        "expires_at_ms": n + 1,
        "generation": n
    }))
    .unwrap()
}

fn mode(p: &Path) -> u32 {
    fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn the_store_is_private_and_lives_outside_the_repository() {
    let state = tempfile::tempdir().unwrap();
    let ws = "a".repeat(64);
    let store = Store::new(state.path(), &ws);
    assert_eq!(store.dir(), state.path().join("memory").join(&ws));
    assert_eq!(
        store.load(),
        Ok(State::default()),
        "nothing yet is empty, not an error"
    );
    assert!(!store.dir().exists(), "reading creates nothing");

    let s = State {
        generation: 1,
        nodes: [(record(1).node_id.clone(), record(1))].into(),
        ..Default::default()
    };
    store.publish(&s).unwrap();

    assert_eq!(mode(&state.path().join("memory")), 0o700);
    assert_eq!(mode(store.dir()), 0o700);
    for entry in fs::read_dir(store.dir()).unwrap() {
        let p = entry.unwrap().path();
        assert_eq!(mode(&p), 0o600, "{}", p.display());
    }
    assert_eq!(store.load(), Ok(s));
}

#[test]
fn a_symlink_or_an_open_directory_reads_as_unavailable() {
    let state = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let ws = "b".repeat(64);
    let store = Store::new(state.path(), &ws);
    let s = State {
        generation: 1,
        ..Default::default()
    };
    store.publish(&s).unwrap();

    // The snapshot swapped for a link to a file that would parse.
    let snapshot = fs::read_dir(store.dir())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let real = elsewhere.path().join("planted.json");
    fs::rename(&snapshot, &real).unwrap();
    std::os::unix::fs::symlink(&real, &snapshot).unwrap();
    assert_eq!(store.load(), Err(Unavailable::Symlink));
    assert_eq!(
        store.publish(&s),
        Err(Unavailable::Symlink),
        "nor written through"
    );
    assert!(
        fs::symlink_metadata(&snapshot)
            .unwrap()
            .file_type()
            .is_symlink()
    );

    // The whole store directory as a link.
    let other = Store::new(state.path(), &"c".repeat(64));
    fs::create_dir_all(state.path().join("memory")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), other.dir()).unwrap();
    assert_eq!(other.load(), Err(Unavailable::Symlink));

    // A store others can write to is not ours to trust.
    let open = Store::new(state.path(), &"d".repeat(64));
    open.publish(&s).unwrap();
    fs::set_permissions(open.dir(), fs::Permissions::from_mode(0o777)).unwrap();
    assert_eq!(open.load(), Err(Unavailable::NotPrivate));
}

#[test]
fn an_unknown_schema_or_a_corrupt_snapshot_is_never_overwritten() {
    let state = tempfile::tempdir().unwrap();
    let store = Store::new(state.path(), &"e".repeat(64));
    let s = State {
        generation: 1,
        ..Default::default()
    };
    store.publish(&s).unwrap();
    let snapshot = fs::read_dir(store.dir())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();

    for (bytes, why) in [
        (
            b"{\"schema_version\": 99}".to_vec(),
            Unavailable::UnknownSchema,
        ),
        (b"{not json".to_vec(), Unavailable::Corrupt),
        (
            b"{\"schema_version\": 1, \"nodes\": 3}".to_vec(),
            Unavailable::Corrupt,
        ),
    ] {
        fs::write(&snapshot, &bytes).unwrap();
        assert_eq!(store.load(), Err(why));
        assert_eq!(
            store.publish(&s),
            Err(why),
            "a newer or broken store is kept"
        );
        assert_eq!(fs::read(&snapshot).unwrap(), bytes, "byte for byte");
    }
    let leftovers = fs::read_dir(store.dir()).unwrap().count();
    assert_eq!(leftovers, 1, "no temporary left behind");
}

#[test]
fn a_reader_never_observes_a_partial_generation() {
    let state = tempfile::tempdir().unwrap();
    let store = Store::new(state.path(), &"f".repeat(64));
    store.publish(&State::default()).unwrap();
    let done = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| {
            let mut reads = 0;
            while !done.load(std::sync::atomic::Ordering::Relaxed) {
                let s = store.load().expect("always a complete generation");
                assert_eq!(
                    s.nodes.len() as u64,
                    s.generation,
                    "nodes match their generation"
                );
                reads += 1;
            }
            reads
        });
        let mut s = State::default();
        for n in 1..=200 {
            s.generation = n;
            s.nodes.insert(record(n).node_id.clone(), record(n));
            store.publish(&s).unwrap();
        }
        done.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(reader.join().unwrap() > 0);
    });
}
