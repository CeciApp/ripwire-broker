//! `--memory-debug-log` (D-164): one line per memory event in `debug.log`, beside the store, for a
//! `tail -F` in another terminal. Written by every process of the workspace (server, hooks,
//! `memory` commands), one whole line per write; ids, counts and reasons, never content.

mod common;

use ripwire_broker::memory::admission::{self, Draft, Event, Outcome, Stamp, Tests};
use ripwire_broker::memory::debug::{DebugLog, FILE};
use ripwire_broker::memory::model::Record;
use ripwire_broker::memory::{identity, store::Store};
use ripwire_broker::online::reader::WorkspaceReader;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// `YYYY-MM-DD HH:MM:SS UTC process#pid stage detail`.
fn well_formed(line: &str, process: &str) -> bool {
    let b = line.as_bytes();
    line.len() > 24
        && b[4] == b'-'
        && b[7] == b'-'
        && b[13] == b':'
        && line[19..].starts_with(" UTC ")
        && line[24..].starts_with(&format!("{process}#{} ", std::process::id()))
}

#[test]
fn an_event_is_one_line_with_time_process_and_stage() {
    let dir = tempfile::tempdir().unwrap();
    let log = DebugLog::open(dir.path(), "test").unwrap();
    log.event("collect", "after_edit scope=2 -> spool");
    let got = lines(&dir.path().join(FILE));
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(well_formed(&got[0], "test"), "{:?}", got[0]);
    assert!(
        got[0].ends_with(" collect   after_edit scope=2 -> spool"),
        "the stage is padded so the details line up: {:?}",
        got[0]
    );
}

#[test]
fn the_file_is_private_and_a_link_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let inner = dir.path().join("memory/ws");
    DebugLog::open(&inner, "test").unwrap().event("x", "y");
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&inner.join(FILE)), 0o600);
    assert_eq!(
        mode(&inner),
        0o700,
        "a missing directory is created private"
    );

    let other = tempfile::tempdir().unwrap();
    let target = other.path().join("elsewhere");
    std::fs::write(&target, "").unwrap();
    std::os::unix::fs::symlink(&target, other.path().join(FILE)).unwrap();
    assert!(DebugLog::open(other.path(), "test").is_err());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "");
}

#[test]
fn control_characters_never_split_or_color_an_event() {
    let dir = tempfile::tempdir().unwrap();
    let log = DebugLog::open(dir.path(), "test").unwrap();
    log.event("read", "a\nb\r\u{1b}[31mc\td");
    let got = lines(&dir.path().join(FILE));
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(got[0].ends_with("a b  [31mc d"), "{:?}", got[0]);
}

#[test]
fn writers_of_two_processes_never_interleave() {
    let dir = tempfile::tempdir().unwrap();
    // Two opens of one path: what two processes have.
    let (a, b) = (
        DebugLog::open(dir.path(), "serve").unwrap(),
        DebugLog::open(dir.path(), "hook").unwrap(),
    );
    let detail = "x".repeat(300);
    std::thread::scope(|s| {
        for log in [&a, &b] {
            let detail = &detail;
            s.spawn(move || {
                for _ in 0..400 {
                    log.event("enrich", detail);
                }
            });
        }
    });
    let got = lines(&dir.path().join(FILE));
    assert_eq!(got.len(), 800);
    for line in &got {
        assert!(
            (well_formed(line, "serve") || well_formed(line, "hook")) && line.ends_with(&detail),
            "{line:?}"
        );
    }
}

#[test]
fn past_the_limit_the_file_rotates_and_every_writer_follows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(FILE);
    let rotated = dir.path().join(format!("{FILE}.1"));
    let a = DebugLog::open_with_limit(dir.path(), "serve", 2_000).unwrap();
    let b = DebugLog::open_with_limit(dir.path(), "hook", 2_000).unwrap();
    // About 66 bytes a line: the 31st goes past 2,000 bytes, and 35 leave a few in the new file.
    for _ in 0..35 {
        a.event("ingest", "added=1 duplicates=0");
    }
    assert!(rotated.exists(), "the full file was set aside");
    let set_aside = lines(&rotated).len();
    assert!(set_aside >= 25, "{set_aside}");
    assert!(std::fs::metadata(&path).unwrap().len() <= 2_000);
    // `b` still holds the file `a` moved away: its next line goes to the new one, where `tail -F`
    // is, and the file set aside is left alone.
    b.event("collect", "after the rotation");
    let now = lines(&path);
    assert!(
        now.last()
            .is_some_and(|l| l.ends_with("after the rotation"))
            && now.len() > 1,
        "{now:?}"
    );
    assert_eq!(
        lines(&rotated).len(),
        set_aside,
        "the file set aside is kept"
    );
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "the new file is private too");
}

fn observed(root: &Path, ws: &str, path: &str, key: &str) -> Record {
    let reader = WorkspaceReader::new(root).unwrap();
    let draft = Draft {
        event_key: key.into(),
        event: Event::AfterEdit,
        outcome: Outcome::AnalysisCompleted,
        tests: Tests::Unknown,
        scope: vec![path.into()],
        evidence: vec!["situational_awareness".into()],
    };
    let stamp = Stamp {
        observed_at_ms: 1,
        ingest_seq: 0,
        generation: 0,
        retention_ms: u64::MAX / 2,
    };
    admission::admit(&reader, ws, &draft, stamp).unwrap()
}

#[test]
fn a_store_with_the_log_says_what_it_did_by_id_never_by_content() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    common::write(root.path(), "src/cache.rs", "fn evict() {}\n");
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Store::new(st.path(), &ws).with_debug_log("test").unwrap();
    let record = observed(root.path(), &ws, "src/cache.rs", "k1");

    store.enqueue(&record).unwrap();
    store.ingest().unwrap();
    store.forget(&record.node_id, u64::MAX / 2).unwrap();

    let got = lines(&store.dir().join(FILE));
    let short = &record.node_id[..12];
    let stage = |s: &str| {
        got.iter()
            .find(|l| l[24..].split_whitespace().nth(1) == Some(s))
    };
    let spool = stage("spool").unwrap_or_else(|| panic!("{got:?}"));
    assert!(spool.contains(short), "{spool}");
    let ingest = stage("ingest").unwrap_or_else(|| panic!("{got:?}"));
    assert!(ingest.contains("added=1"), "{ingest}");
    let forget = stage("forget").unwrap_or_else(|| panic!("{got:?}"));
    assert!(
        forget.contains(short) && forget.contains("removed=1"),
        "{forget}"
    );
    let all = got.join("\n");
    assert!(
        !all.contains(&record.content) && !all.contains("Evento"),
        "no memory text in the log: {all}"
    );
}

#[test]
fn a_store_without_the_log_writes_no_file() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    common::write(root.path(), "src/cache.rs", "fn evict() {}\n");
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Store::new(st.path(), &ws);
    store
        .enqueue(&observed(root.path(), &ws, "src/cache.rs", "k1"))
        .unwrap();
    store.ingest().unwrap();
    assert!(!store.dir().join(FILE).exists());
}
