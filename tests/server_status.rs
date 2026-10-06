//! The server's projection for the status line: whether `serve` runs online, and how many Jev
//! requests it sent in the last seconds. Written by `serve`, read by `statusline` without a lock.
use ripwire_broker::server_status::*;
use ripwire_broker::statusline_state::workspace_key;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn status(root: &Path, pid: u32, updated_at: u64, online: bool, calls: &[(u64, u32)]) -> Status {
    Status {
        schema_version: SCHEMA_VERSION,
        workspace_key: workspace_key(root),
        pid,
        updated_at,
        online,
        memory: false,
        jev_calls: calls.to_vec(),
        mem_reads: vec![],
        mem_stores: vec![],
        jev_key: KeyState::Ok,
    }
}

fn view(online: bool, jev_calls: u32) -> View {
    View {
        online,
        jev_calls,
        ..View::default()
    }
}

fn dirs() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    (tmp, root)
}

#[test]
fn activity_keeps_one_count_per_second_for_the_window_only() {
    let a = Activity::default();
    for at in [100, 100, 101, 103, 103, 103] {
        a.record(at);
    }
    assert_eq!(a.calls(103), vec![(100, 2), (101, 1), (103, 3)]);
    // Second 100 leaves the five-second window at 105.
    assert_eq!(a.calls(105), vec![(101, 1), (103, 3)]);
    a.record(200);
    assert_eq!(a.calls(200), vec![(200, 1)]);
}

#[test]
fn activity_reports_new_calls_once() {
    let a = Activity::default();
    assert!(!a.take_dirty());
    a.record(10);
    assert!(a.take_dirty());
    assert!(!a.take_dirty());
}

#[test]
fn only_the_last_five_seconds_are_counted() {
    let s = status(Path::new("/r"), 1, 100, true, &[(95, 7), (96, 1), (100, 2)]);
    assert_eq!(s.recent_calls(100), 3);
    assert_eq!(s.recent_calls(101), 2);
    assert_eq!(s.recent_calls(105), 0);
}

#[test]
fn a_published_status_reads_back_and_the_file_is_private() {
    let (tmp, root) = dirs();
    let state = tmp.path().join("state");
    publish(&state, &root, &status(&root, 42, 1_000, true, &[(999, 4)])).unwrap();
    let file = path(&state, &root, 42);
    let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert_eq!(read(&state, &root, 1_000), Some(view(true, 4)));
}

#[test]
fn several_servers_of_a_workspace_add_up_and_any_online_one_counts() {
    let (tmp, root) = dirs();
    let state = tmp.path().join("state");
    publish(&state, &root, &status(&root, 1, 1_000, true, &[(1_000, 2)])).unwrap();
    publish(&state, &root, &status(&root, 2, 1_000, false, &[])).unwrap();
    publish(&state, &root, &status(&root, 3, 999, true, &[(998, 5)])).unwrap();
    assert_eq!(read(&state, &root, 1_000), Some(view(true, 7)));
}

#[test]
fn a_status_not_refreshed_in_time_is_ignored() {
    let (tmp, root) = dirs();
    let state = tmp.path().join("state");
    publish(&state, &root, &status(&root, 1, 1_000, true, &[])).unwrap();
    assert!(read(&state, &root, 1_000 + STALE_SECS).is_some());
    assert_eq!(read(&state, &root, 1_000 + STALE_SECS + 1), None);
}

#[test]
fn another_workspace_or_schema_is_not_read() {
    let (tmp, root) = dirs();
    let other = tmp.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let other = other.canonicalize().unwrap();
    let state = tmp.path().join("state");
    publish(&state, &other, &status(&other, 1, 1_000, true, &[])).unwrap();
    assert_eq!(read(&state, &root, 1_000), None);

    // Right file name, wrong key inside: refused.
    let mut forged = status(&other, 2, 1_000, true, &[]);
    let file = path(&state, &root, 2);
    std::fs::write(&file, serde_json::to_vec(&forged).unwrap()).unwrap();
    assert_eq!(read(&state, &root, 1_000), None);

    forged = Status {
        schema_version: SCHEMA_VERSION + 1,
        ..status(&root, 2, 1_000, true, &[])
    };
    std::fs::write(&file, serde_json::to_vec(&forged).unwrap()).unwrap();
    assert_eq!(read(&state, &root, 1_000), None);
}

#[test]
fn corrupt_oversized_and_linked_files_are_ignored() {
    let (tmp, root) = dirs();
    let state = tmp.path().join("state");
    publish(&state, &root, &status(&root, 1, 1_000, true, &[])).unwrap();
    std::fs::write(path(&state, &root, 1), "{not json").unwrap();
    assert_eq!(read(&state, &root, 1_000), None);
    std::fs::write(path(&state, &root, 1), vec![b' '; MAX_BYTES as usize + 1]).unwrap();
    assert_eq!(read(&state, &root, 1_000), None);

    let real = tmp.path().join("real.json");
    std::fs::write(
        &real,
        serde_json::to_vec(&status(&root, 1, 1_000, true, &[])).unwrap(),
    )
    .unwrap();
    std::fs::remove_file(path(&state, &root, 1)).unwrap();
    std::os::unix::fs::symlink(&real, path(&state, &root, 1)).unwrap();
    assert_eq!(read(&state, &root, 1_000), None);
}

#[test]
fn reading_creates_nothing() {
    let (tmp, root) = dirs();
    let state = tmp.path().join("state");
    assert_eq!(read(&state, &root, 1_000), None);
    assert!(!state.exists());
}

#[tokio::test]
async fn the_publisher_writes_at_once_counts_calls_and_removes_its_file_when_dropped() {
    let (tmp, root) = dirs();
    let state = tmp.path().join("state");
    let activity = std::sync::Arc::new(Activities::default());
    let mode = Mode {
        online: true,
        memory: true,
    };
    let publisher = Publisher::start(state.clone(), root.clone(), mode, activity.clone());
    let file = path(&state, &root, std::process::id());
    let now = ripwire_broker::hook::now();
    let seen = |want: u32| {
        let (state, root) = (state.clone(), root.clone());
        async move {
            for _ in 0..50 {
                let got = read(&state, &root, ripwire_broker::hook::now());
                if got.map(|v| v.jev_calls) == Some(want) {
                    return got;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            None
        }
    };
    assert_eq!(
        seen(0).await,
        Some(View {
            memory: true,
            ..view(true, 0)
        })
    );
    activity.jev.record(now);
    activity.jev.record(now);
    activity.mem_reads.record(now);
    activity.mem_stores.record(now);
    activity.mem_stores.record(now);
    activity.mem_stores.record(now);
    let got = seen(2).await.unwrap();
    assert_eq!((got.mem_reads, got.mem_stores), (1, 3));
    drop(publisher);
    assert!(!file.exists());
}

#[test]
fn memory_reads_and_stores_are_counted_like_jev_requests_and_add_up() {
    let (tmp, root) = dirs();
    let state = tmp.path().join("state");
    let with_memory = |pid, reads: &[(u64, u32)], stores: &[(u64, u32)]| Status {
        memory: true,
        mem_reads: reads.to_vec(),
        mem_stores: stores.to_vec(),
        ..status(&root, pid, 1_000, true, &[])
    };
    publish(
        &state,
        &root,
        &with_memory(1, &[(990, 9), (999, 2)], &[(1_000, 1)]),
    )
    .unwrap();
    publish(&state, &root, &with_memory(2, &[(1_000, 1)], &[(997, 4)])).unwrap();
    assert_eq!(
        read(&state, &root, 1_000),
        Some(View {
            online: true,
            memory: true,
            jev_calls: 0,
            mem_reads: 3,
            mem_stores: 5,
            jev_key: KeyState::Ok,
        })
    );
}

#[test]
fn the_key_state_travels_in_the_file_and_the_worst_one_wins() {
    let (tmp, root) = dirs();
    let state = tmp.path().join("state");
    let with = |pid, key| Status {
        jev_key: key,
        ..status(&root, pid, 1_000, true, &[])
    };
    publish(&state, &root, &with(1, KeyState::Ok)).unwrap();
    assert_eq!(read(&state, &root, 1_000).unwrap().jev_key, KeyState::Ok);
    publish(&state, &root, &with(2, KeyState::Invalid)).unwrap();
    assert_eq!(
        read(&state, &root, 1_000).unwrap().jev_key,
        KeyState::Invalid
    );
    publish(&state, &root, &with(3, KeyState::Missing)).unwrap();
    assert_eq!(
        read(&state, &root, 1_000).unwrap().jev_key,
        KeyState::Missing
    );
}

#[test]
fn a_key_change_is_published_like_new_activity() {
    let a = Activities::default();
    assert!(!a.jev.take_dirty());
    a.set_key(KeyState::Missing);
    assert_eq!(a.key(), KeyState::Missing);
    assert!(a.take_key_dirty());
    a.set_key(KeyState::Missing);
    assert!(!a.take_key_dirty(), "the same state again is no change");
}

#[test]
fn the_global_switch_is_off_unless_set() {
    // The binary sets it when the key is missing; nothing in a test process does.
    assert!(!ripwire_broker::online::no_jev_api_key());
}
