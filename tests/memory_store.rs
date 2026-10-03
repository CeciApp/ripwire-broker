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
        "ingest_seq": 0,
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

// ---------------------------------------------------------------- spool → snapshot (§6, CA-4, CA-7)

use ripwire_broker::memory::store::{Full, Limits, Refusal, Step};

/// The same observation seen again by another session: another clock and event key.
fn seen_again(n: u64) -> Record {
    let mut r = record(n);
    r.observed_at_ms += 10_000;
    r.event_key = "another session".into();
    r
}

#[test]
fn replaying_the_same_observation_yields_one_node_and_one_increment() {
    let state = tempfile::tempdir().unwrap();
    let store = Store::new(state.path(), &"g".repeat(64));
    store.enqueue(&record(1)).unwrap();
    store.enqueue(&seen_again(1)).unwrap();
    let first = store.ingest().unwrap();
    assert_eq!((first.added, first.duplicates), (1, 0));

    store.enqueue(&seen_again(1)).unwrap();
    store.enqueue(&record(2)).unwrap();
    let second = store.ingest().unwrap();
    assert_eq!((second.added, second.duplicates), (1, 1));

    let s = store.load().unwrap();
    assert_eq!(s.nodes.len(), 2);
    let seqs: Vec<u64> = s.nodes.values().map(|r| r.ingest_seq).collect();
    assert_eq!(
        seqs,
        [1, 2],
        "one increment per new node, assigned under the lock"
    );
    assert_eq!(s.generation, 2);
    assert_eq!(store.pending().unwrap(), 0, "the spool is drained");
}

#[test]
fn a_crash_between_commit_and_spool_removal_does_not_duplicate() {
    for step in [Step::BeforePublish, Step::AfterPublish, Step::MidRemoval] {
        let state = tempfile::tempdir().unwrap();
        let store = Store::new(state.path(), &"h".repeat(64));
        for n in 1..=3 {
            store.enqueue(&record(n)).unwrap();
        }
        assert_eq!(
            store.ingest_crashing_at(step),
            Err(Refusal::Crashed),
            "{step:?}"
        );
        let survived = store.load().unwrap();
        match step {
            Step::BeforePublish => assert!(survived.nodes.is_empty(), "nothing committed"),
            _ => assert_eq!(survived.nodes.len(), 3, "{step:?}: committed whole"),
        }

        let resumed = store.ingest().unwrap();
        let s = store.load().unwrap();
        assert_eq!(s.nodes.len(), 3, "{step:?}");
        let mut seqs: Vec<u64> = s.nodes.values().map(|r| r.ingest_seq).collect();
        seqs.sort();
        assert_eq!(seqs, [1, 2, 3], "{step:?}: no node counted twice");
        assert_eq!(resumed.added + survived.nodes.len(), 3, "{step:?}");
        assert_eq!(store.pending().unwrap(), 0, "{step:?}");
    }
}

#[test]
fn a_full_store_refuses_new_writes_with_a_reason() {
    let state = tempfile::tempdir().unwrap();
    let small = |l: Limits| Store::with_limits(state.path(), &"i".repeat(64), l);
    assert_eq!(
        Limits::default(),
        Limits {
            max_nodes: 2_000,
            spool_entries: 1_000,
            spool_bytes: 16 * 1024 * 1024,
            snapshot_bytes: 64 * 1024 * 1024,
            total_bytes: 96 * 1024 * 1024,
        },
        "PRD jev-mem §6"
    );

    let store = small(Limits {
        max_nodes: 2,
        ..Limits::default()
    });
    for n in 1..=3 {
        store.enqueue(&record(n)).unwrap();
    }
    let got = store.ingest().unwrap();
    assert_eq!((got.added, got.refused), (2, Some(Full::Nodes)));
    assert_eq!(store.load().unwrap().nodes.len(), 2, "never past the cap");
    assert_eq!(
        store.pending().unwrap(),
        1,
        "the refused one stays pending, not lost"
    );

    let store = Store::with_limits(
        state.path(),
        "n",
        Limits {
            spool_entries: 2,
            ..Limits::default()
        },
    );
    store.enqueue(&record(10)).unwrap();
    store.enqueue(&record(11)).unwrap();
    assert_eq!(
        store.enqueue(&record(12)),
        Err(Refusal::Full(Full::SpoolEntries))
    );
    assert_eq!(
        store.enqueue(&seen_again(11)).map(|_| ()),
        Ok(()),
        "a replay takes no room"
    );

    let one = serde_json::to_vec(&record(20)).unwrap().len() as u64;
    let st = tempfile::tempdir().unwrap();
    let store = Store::with_limits(
        st.path(),
        "j",
        Limits {
            spool_bytes: one + 10,
            ..Limits::default()
        },
    );
    store.enqueue(&record(20)).unwrap();
    assert_eq!(
        store.enqueue(&record(21)),
        Err(Refusal::Full(Full::SpoolBytes))
    );

    let st = tempfile::tempdir().unwrap();
    let store = Store::with_limits(
        st.path(),
        "k",
        Limits {
            snapshot_bytes: one,
            ..Limits::default()
        },
    );
    store.enqueue(&record(30)).unwrap();
    store.enqueue(&record(31)).unwrap();
    assert_eq!(store.ingest().unwrap().refused, Some(Full::Snapshot));
    assert!(store.load().unwrap().nodes.len() < 2);

    let st = tempfile::tempdir().unwrap();
    let store = Store::with_limits(
        st.path(),
        "l",
        Limits {
            total_bytes: one + 10,
            ..Limits::default()
        },
    );
    store.enqueue(&record(40)).unwrap();
    store.ingest().unwrap();
    assert_eq!(store.enqueue(&record(41)), Err(Refusal::Full(Full::Total)));
}

#[test]
fn a_second_writer_does_not_wait_and_reports_the_lock() {
    let state = tempfile::tempdir().unwrap();
    let store = Store::new(state.path(), &"m".repeat(64));
    store.enqueue(&record(1)).unwrap();
    let held = store.writer().unwrap();
    let started = std::time::Instant::now();
    assert_eq!(store.ingest().map(|_| ()), Err(Refusal::Locked));
    assert!(
        started.elapsed() < std::time::Duration::from_millis(500),
        "did not wait"
    );
    assert_eq!(
        store.enqueue(&record(2)).map(|_| ()),
        Ok(()),
        "publishing to the spool needs no lock"
    );
    assert!(store.load().is_ok(), "nor does reading");
    drop(held);
    assert_eq!(store.ingest().unwrap().added, 2);
}

// ---------------------------------------------------------------- retention (§6, CA-11)

use ripwire_broker::memory::model::{Kind, Parent};

fn expiring(n: u64, at: u64) -> Record {
    let mut r = record(n);
    r.expires_at_ms = at;
    r
}

fn derived(n: u64, at: u64, parents: &[&Record]) -> Record {
    let mut r = expiring(n, at);
    r.kind = Kind::DerivedNote;
    r.derived_from = parents
        .iter()
        .map(|p| Parent {
            node_id: p.node_id.clone(),
            content_hash: p.content_hash.clone(),
        })
        .collect();
    r
}

fn stored(state: &Path, records: &[Record]) -> Store {
    let store = Store::new(state, &"r".repeat(64));
    for r in records {
        store.enqueue(r).unwrap();
    }
    store.ingest().unwrap();
    store
}

fn ids(store: &Store) -> Vec<u64> {
    let s = store.load().unwrap();
    s.nodes
        .values()
        .map(|r| r.node_id.parse().unwrap())
        .collect()
}

#[test]
fn expired_nodes_take_their_derived_notes_with_them() {
    let state = tempfile::tempdir().unwrap();
    let (old, young) = (expiring(1, 100), expiring(2, 10_000));
    let note = derived(3, 10_000, &[&old]);
    let of_note = derived(4, 10_000, &[&note]);
    let store = stored(state.path(), &[old, young, note, of_note]);

    let swept = store.sweep(99).unwrap();
    assert_eq!(
        (swept.removed, swept.suspended),
        (0, false),
        "nothing is due yet"
    );
    let generation = store.load().unwrap().generation;

    let swept = store.sweep(100).unwrap();
    assert_eq!(
        swept.removed, 3,
        "the node, its note, and the note made from that note"
    );
    assert_eq!(ids(&store), [2]);
    assert_eq!(
        store.load().unwrap().generation,
        generation + 1,
        "a new generation"
    );
}

#[test]
fn a_derived_note_never_outlives_its_parents() {
    let state = tempfile::tempdir().unwrap();
    let (a, b) = (expiring(1, 10_000), expiring(2, 100));
    let note = derived(3, 1_000_000, &[&a, &b]);
    let store = stored(state.path(), &[a, b, note]);
    store.sweep(100).unwrap();
    assert_eq!(ids(&store), [1], "one parent gone is enough");
}

#[test]
fn a_clock_rollback_suspends_expiry_by_age_but_keeps_the_caps() {
    let state = tempfile::tempdir().unwrap();
    let store = stored(state.path(), &[expiring(1, 500)]);
    store.sweep(50).unwrap();
    store.enqueue(&expiring(2, 20)).unwrap();
    store.ingest().unwrap();
    let back = store.sweep(30).unwrap();
    assert!(back.suspended, "the clock went back from 50 to 30");
    assert_eq!(
        back.removed, 0,
        "a node due at 20 waits for a trusted clock"
    );
    assert_eq!(ids(&store), [1, 2]);

    let small = Store::with_limits(
        state.path(),
        &"r".repeat(64),
        Limits {
            max_nodes: 2,
            ..Limits::default()
        },
    );
    small.enqueue(&record(9)).unwrap();
    assert_eq!(
        small.ingest().unwrap().refused,
        Some(Full::Nodes),
        "caps still hold"
    );

    let recovered = store.sweep(600).unwrap();
    assert_eq!((recovered.suspended, recovered.removed), (false, 2));
}

// ---------------------------------------------------------------- forget (§6, CA-7, CA-11)

#[test]
fn forget_by_id_removes_the_node_its_descendants_and_blocks_reingestion() {
    let state = tempfile::tempdir().unwrap();
    let parent = expiring(1, 10_000);
    let note = derived(2, 10_000, &[&parent]);
    let store = stored(state.path(), &[parent.clone(), note, expiring(3, 10_000)]);
    let generation = store.load().unwrap().generation;

    let forgotten = store.forget(&parent.node_id, 5_000).unwrap();
    assert_eq!(forgotten, 2, "the node and the note derived from it");
    assert_eq!(ids(&store), [3]);
    assert!(store.load().unwrap().generation > generation);

    store.enqueue(&seen_again(1)).unwrap();
    let again = store.ingest().unwrap();
    assert_eq!(
        (again.added, again.forgotten),
        (0, 1),
        "blocked while the tombstone lasts"
    );
    assert_eq!(ids(&store), [3]);
    assert_eq!(store.pending().unwrap(), 0);

    // A forgotten id that never reached the store is blocked as well.
    assert_eq!(store.forget(&record(7).node_id, 5_000).unwrap(), 0);
    store.enqueue(&record(7)).unwrap();
    assert_eq!(store.ingest().unwrap().forgotten, 1);

    // The tombstone lasts as long as the retention it was given, and no longer.
    store.sweep(5_000).unwrap();
    store.enqueue(&record(7)).unwrap();
    assert_eq!(store.ingest().unwrap().added, 1);
}

#[test]
fn an_old_spool_entry_cannot_resurrect_a_forgotten_node() {
    let state = tempfile::tempdir().unwrap();
    let store = Store::new(state.path(), &"s".repeat(64));
    store.enqueue(&record(1)).unwrap();
    store.ingest().unwrap();
    // A copy of the spool entry, as a crash before its removal would have left it.
    store.enqueue(&seen_again(1)).unwrap();
    store.forget(&record(1).node_id, 1_000_000).unwrap();
    assert_eq!(
        store.pending().unwrap(),
        0,
        "forget clears its spool entries too"
    );

    store.enqueue(&record(1)).unwrap();
    let late = store.ingest().unwrap();
    assert_eq!((late.added, late.forgotten), (0, 1));
    assert!(store.load().unwrap().nodes.is_empty());
}

#[test]
fn forget_leaves_no_temporary_or_backup_with_text() {
    let state = tempfile::tempdir().unwrap();
    let store = Store::new(state.path(), &"t".repeat(64));
    let marker = "unique-marker-5f1c9a";
    let mut secret = record(1);
    secret.content = format!("observation {marker}");
    store.enqueue(&secret).unwrap();
    store.ingest().unwrap();
    store.enqueue(&secret).unwrap();
    store.forget(&secret.node_id, 1_000).unwrap();

    let mut files = vec![];
    let mut dirs = vec![state.path().to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            match p.is_dir() {
                true => dirs.push(p),
                false => files.push(p),
            }
        }
    }
    assert!(!files.is_empty());
    for f in files {
        let text = String::from_utf8_lossy(&fs::read(&f).unwrap()).into_owned();
        assert!(
            !text.contains(marker),
            "{} still holds the text",
            f.display()
        );
    }
}

// ---------------------------------------------------------------- revocation (§6, PD-4)

#[test]
fn after_forget_all_nothing_is_collected_until_resumed() {
    let state = tempfile::tempdir().unwrap();
    let ws = "u".repeat(64);
    let store = Store::new(state.path(), &ws);
    store.enqueue(&record(1)).unwrap();
    store.ingest().unwrap();
    store.enqueue(&record(2)).unwrap();

    assert_eq!(store.forget_all(1_000_000).unwrap(), 1);
    assert!(store.load().unwrap().nodes.is_empty());
    assert_eq!(store.pending().unwrap(), 0, "pending observations go too");
    assert!(store.is_revoked());

    // A new process, as after a restart: the marker wins over --memory.
    let restarted = Store::new(state.path(), &ws);
    assert!(restarted.is_revoked());
    assert_eq!(restarted.enqueue(&record(3)), Err(Refusal::Revoked));
    assert_eq!(restarted.ingest().map(|_| ()), Err(Refusal::Revoked));
    assert_eq!(restarted.pending().unwrap(), 0);

    assert!(restarted.resume().unwrap(), "it was revoked");
    assert!(!restarted.is_revoked());
    assert!(!restarted.resume().unwrap(), "resuming twice is harmless");
    restarted.enqueue(&record(3)).unwrap();
    assert_eq!(restarted.ingest().unwrap().added, 1);
    restarted.enqueue(&record(1)).unwrap();
    assert_eq!(
        restarted.ingest().unwrap().forgotten,
        1,
        "forgotten ids stay forgotten"
    );
}

// ---------------------------------------------------------------- review of phase 1 (D-137)

#[test]
fn an_enqueue_racing_forget_all_never_survives_it() {
    let state = tempfile::tempdir().unwrap();
    let store = Store::new(state.path(), &"v".repeat(64));
    // forget --all lands after the enqueue checked the marker and before its file appeared.
    let got = store.enqueue_with(&record(1), || {
        store.forget_all(1_000_000).unwrap();
    });
    assert_eq!(got, Err(Refusal::Revoked));
    assert_eq!(
        store.pending().unwrap(),
        0,
        "nothing the user asked to erase stays"
    );
}

/// A temporary as a writer killed between create and rename leaves it.
fn leftover(dir: &Path, name: &str, age_secs: u64) -> std::path::PathBuf {
    fs::create_dir_all(dir).unwrap();
    let p = dir.join(name);
    fs::write(&p, "observation with text").unwrap();
    let when = std::time::SystemTime::now() - std::time::Duration::from_secs(age_secs);
    fs::File::options()
        .write(true)
        .open(&p)
        .unwrap()
        .set_modified(when)
        .unwrap();
    p
}

#[test]
fn leftover_temporaries_are_counted_cleaned_and_erased() {
    let state = tempfile::tempdir().unwrap();
    let store = Store::new(state.path(), &"x".repeat(64));
    store.enqueue(&record(1)).unwrap();
    let spool = store.dir().join("spool");
    let stale = leftover(&spool, "a.tmp99-1", 3_600);
    let fresh = leftover(&spool, "b.tmp99-2", 0);
    assert_eq!(
        store.usage().unwrap().pending,
        1,
        "a temporary is not an observation"
    );
    assert!(
        store.usage().unwrap().spool_bytes > fs::metadata(&stale).unwrap().len(),
        "but it takes room"
    );

    store.ingest().unwrap();
    assert!(!stale.exists(), "an old one is a dead writer's: removed");
    assert!(fresh.exists(), "a recent one may still be renamed");

    let snap_tmp = leftover(store.dir(), "snapshot.tmp99-3", 0);
    store.forget_all(1).unwrap();
    assert!(
        !fresh.exists() && !snap_tmp.exists(),
        "forget --all leaves no temporary"
    );
}

#[test]
fn forget_all_on_an_unreadable_store_still_erases_the_spool() {
    let state = tempfile::tempdir().unwrap();
    let store = Store::new(state.path(), &"y".repeat(64));
    store.enqueue(&record(1)).unwrap();
    store.ingest().unwrap();
    store.enqueue(&record(2)).unwrap();
    let snapshot = store.dir().join("snapshot.json");
    fs::write(&snapshot, "{broken").unwrap();

    assert_eq!(
        store.forget_all(1),
        Err(Refusal::Unavailable(Unavailable::Corrupt))
    );
    assert!(store.is_revoked());
    assert_eq!(
        store.pending().unwrap(),
        0,
        "pending observations are erased anyway"
    );
    assert_eq!(
        fs::read_to_string(&snapshot).unwrap(),
        "{broken",
        "the snapshot is kept as it was"
    );
}

#[test]
fn one_bad_spool_entry_does_not_block_ingestion_and_a_newer_schema_is_kept() {
    let state = tempfile::tempdir().unwrap();
    let store = Store::new(state.path(), &"z".repeat(64));
    store.enqueue(&record(1)).unwrap();
    let spool = store.dir().join("spool");
    std::os::unix::fs::symlink("/etc/hosts", spool.join("planted.json")).unwrap();
    let mut newer = serde_json::to_value(record(2)).unwrap();
    newer["schema_version"] = json!(2);
    fs::write(spool.join("newer.json"), newer.to_string()).unwrap();

    let got = store.ingest().unwrap();
    assert_eq!(got.added, 1, "the good one gets in");
    assert!(
        !spool.join("planted.json").exists(),
        "a link is dropped, never followed"
    );
    assert!(
        spool.join("newer.json").exists(),
        "a newer schema is not ours to destroy"
    );
    assert_eq!(store.ingest().unwrap().added, 0);
}

#[test]
fn a_linked_spool_directory_is_never_written_through() {
    let state = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let store = Store::new(state.path(), &"q".repeat(64));
    store.enqueue(&record(1)).unwrap();
    store.ingest().unwrap();
    let spool = store.dir().join("spool");
    fs::remove_dir(&spool).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), &spool).unwrap();

    assert_eq!(
        store.enqueue(&record(2)),
        Err(Refusal::Unavailable(Unavailable::Symlink))
    );
    assert!(fs::read_dir(elsewhere.path()).unwrap().next().is_none());
}
