//! Property tests that need a workspace on disk (D-112): the workspace guard (P0.1), the
//! eligibility policy (P0.2) and evidence units (P0.3).
//!
//! Separate from `tests/props.rs` because every case here does file I/O, so this target runs with
//! a low case count while that one runs with thousands. Select it with
//! `cargo nextest run -E 'binary(props_fs)'`.
//!
//! **Safety rules this file obeys, because two of these properties fuzz a path-traversal guard:**
//! the root is always a fresh `tempfile::TempDir`; **no generated path is ever written to,
//! created or removed** — generated strings are only ever *asked about*; the files are made once
//! by hand at fixed names; nothing generated is executed; and the sizes are capped well under
//! `MAX_READ_BYTES` so a runner cannot be exhausted.

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use ripwire_broker::online::reader::{self, WorkspaceReader};
use ripwire_broker::workspace::Workspace;
use std::fs;
use std::path::Path;

/// Few cases, because each one touches the filesystem.
fn config() -> Config {
    Config {
        cases: 48,
        failure_persistence: None,
        ..Config::default()
    }
}

// ---------------------------------------------------------------- P0.1 — the workspace guard

/// A root with something to escape from: a nested directory, a file, and a symlink pointing
/// **out** of the root. Made by hand, at fixed names.
fn guarded_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("sub/deep")).unwrap();
    fs::write(root.join("sub/deep/f.rs"), "fn a() {}\n").unwrap();
    fs::write(root.join("plain.txt"), "x\n").unwrap();
    // The escape a traversal guard exists for. `/etc` always exists and is never written to.
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc", root.join("out")).unwrap();
    dir
}

/// Path-shaped strings built from pieces that matter to a traversal guard, rather than arbitrary
/// text that would almost never form a path.
fn path_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            "..",
            ".",
            "/",
            "sub",
            "deep",
            "f.rs",
            "out",
            "etc",
            "passwd",
            "plain.txt",
            "",
            "\u{e9}",
            "//",
            "\\",
            "~",
            " ",
            "\u{0}",
            "a",
        ]),
        0..8,
    )
    .prop_map(|v| v.join("/"))
    .boxed()
    .prop_union(
        prop::collection::vec(
            prop::sample::select(vec!["..", "sub", "out", "etc", "/"]),
            1..12,
        )
        .prop_map(|v| v.concat())
        .boxed(),
    )
}

#[test]
fn a_path_the_guard_accepts_always_stays_inside_the_root() {
    let dir = guarded_root();
    let root = dir.path().canonicalize().unwrap();
    let ws = Workspace::new(&root).unwrap();

    TestRunner::new(config())
        .run(&path_strategy(), |path| {
            // Only ever asked about. Never created, never written, never removed.
            let Ok(rel) = ws.relative(&path) else {
                return Ok(());
            };
            let joined = root.join(&rel);
            // A **component** equal to `..`, not the substring: `"...."` is an ordinary file
            // name and only `Component::ParentDir` climbs. Asserting on the substring failed on
            // `"...."`, which was my assertion being wrong, not the guard.
            prop_assert!(
                !Path::new(&rel)
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir)),
                "the accepted relative path climbs: {:?} from {:?}",
                rel,
                path
            );
            prop_assert!(
                joined.starts_with(&root),
                "{:?} was accepted as {:?}, which lands at {:?}",
                path,
                rel,
                joined
            );
            // The oracle, where the filesystem can answer: an existing accepted path must
            // canonicalize back inside the root. This is what catches a symlink escape.
            if joined.exists() {
                let real = joined.canonicalize().unwrap();
                prop_assert!(
                    real.starts_with(&root),
                    "{:?} was accepted but resolves to {:?}",
                    path,
                    real
                );
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_line_seed_obeys_the_same_rule_as_a_path() {
    let dir = guarded_root();
    let root = dir.path().canonicalize().unwrap();
    let ws = Workspace::new(&root).unwrap();

    TestRunner::new(config())
        .run(&(path_strategy(), 0u32..9999), |(path, line)| {
            let seed = format!("@{path}:{line}");
            // A seed is accepted only when its file part is, and refused whenever that is.
            prop_assert_eq!(
                ws.check_symbol(&seed).is_ok(),
                ws.relative(&path).is_ok(),
                "seed {:?} and path {:?} disagree",
                seed,
                path
            );
            // A plain symbol carries no path and is never refused on these grounds.
            prop_assert!(ws.check_symbol(&path.replace('@', "")).is_ok());
            Ok(())
        })
        .unwrap();
}

// ---------------------------------------------------------------- P0.2 — eligibility

/// One file per reason a file must not be read, at fixed names, plus one that must be read.
fn policy_root() -> (tempfile::TempDir, Vec<(&'static str, bool)>) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
    fs::create_dir_all(root.join("target/debug")).unwrap();
    fs::create_dir_all(root.join(".hidden")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();

    fs::write(root.join("src/ok.rs"), "fn a() {}\n").unwrap();
    fs::write(root.join(".env"), "SECRET=1\n").unwrap();
    fs::write(root.join(".env.local"), "SECRET=1\n").unwrap();
    fs::write(root.join("id_rsa"), "-----BEGIN OPENSSH PRIVATE KEY-----\n").unwrap();
    fs::write(root.join("key.pem"), "-----BEGIN PRIVATE KEY-----\nx\n").unwrap();
    fs::write(root.join(".hidden/h.rs"), "fn h() {}\n").unwrap();
    fs::write(root.join("node_modules/pkg/i.js"), "let a = 1\n").unwrap();
    fs::write(root.join("target/debug/b.rs"), "fn b() {}\n").unwrap();
    fs::write(root.join("bin.dat"), [0u8, 1, 2, 0, 255, 0]).unwrap();
    fs::write(root.join("bad.txt"), [0xff, 0xfe, 0xfd]).unwrap();
    fs::write(root.join("ignored.rs"), "fn i() {}\n").unwrap();
    // A sensitive **name** with innocuous, non-hidden content. Without it nothing in this table
    // isolates `sensitive_name`: `.env` is also hidden, and `id_rsa`/`key.pem` also carry the
    // private-key marker, so each was refused by a different control than the one intended.
    // Removing `sensitive_name` altogether left the table green until this file existed (D-112).
    fs::write(root.join("service.key"), "just a note, no marker\n").unwrap();
    fs::write(root.join(".gitignore"), "ignored.rs\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("src/ok.rs", root.join("link.rs")).unwrap();

    let cases = vec![
        ("src/ok.rs", true),
        (".env", false),
        (".env.local", false),
        ("id_rsa", false),
        ("key.pem", false),
        (".hidden/h.rs", false),
        ("node_modules/pkg/i.js", false),
        ("target/debug/b.rs", false),
        ("bin.dat", false),
        ("bad.txt", false),
        ("ignored.rs", false),
        ("service.key", false),
        ("link.rs", false),
        ("does/not/exist.rs", false),
    ];
    (dir, cases)
}

#[test]
fn a_file_the_policy_refuses_is_never_read() {
    let (dir, cases) = policy_root();
    let reader = WorkspaceReader::new(dir.path()).unwrap();

    for (rel, eligible) in &cases {
        let got = reader.snapshot(rel);
        assert_eq!(
            got.is_ok(),
            *eligible,
            "{rel}: expected eligible={eligible}, got {:?}",
            got.as_ref().map(|s| s.path.clone()).map_err(|e| e.as_str())
        );
    }
    // And the one eligible file really was read, so the table above is not vacuously true.
    let snap = reader.snapshot("src/ok.rs").unwrap();
    assert!(snap.content_hash.starts_with("sha256:"));
    assert!(!snap.preview().is_empty());
}

#[test]
fn asking_for_an_arbitrary_path_never_reads_a_refused_file() {
    let (dir, cases) = policy_root();
    let reader = WorkspaceReader::new(dir.path()).unwrap();
    let refused: Vec<&str> = cases
        .iter()
        .filter(|(_, ok)| !ok)
        .map(|(p, _)| *p)
        .collect();

    TestRunner::new(config())
        .run(
            &prop::collection::vec(
                prop::sample::select(vec![
                    "..",
                    ".",
                    "/",
                    "src",
                    "ok.rs",
                    ".env",
                    "id_rsa",
                    "node_modules",
                    "pkg",
                    "i.js",
                    "target",
                    ".hidden",
                    "h.rs",
                    "link.rs",
                    "ignored.rs",
                    "bin.dat",
                ]),
                1..6,
            )
            .prop_map(|v| v.join("/")),
            |rel| {
                // Only asked about; nothing is created from a generated name.
                if let Ok(snap) = reader.snapshot(&rel) {
                    prop_assert!(
                        !refused.contains(&snap.path.as_str()),
                        "{:?} was read through the name {:?}",
                        snap.path,
                        rel
                    );
                    prop_assert!(
                        snap.content_hash.starts_with("sha256:"),
                        "a snapshot without a digest"
                    );
                }
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn a_changed_byte_makes_a_snapshot_stale() {
    let dir = tempfile::tempdir().unwrap();
    // A fixed name. The generated value is the *content*, never the path.
    let path = dir.path().join("f.rs");
    let reader = WorkspaceReader::new(dir.path()).unwrap();

    TestRunner::new(config())
        .run(&("[a-z\n]{1,200}", "[a-z\n]{1,200}"), |(before, after)| {
            fs::write(&path, &before).unwrap();
            let snap = reader
                .snapshot("f.rs")
                .expect("a plain source file is eligible");
            prop_assert!(reader.is_fresh(&snap), "fresh right after being read");
            fs::write(&path, &after).unwrap();
            prop_assert_eq!(
                reader.is_fresh(&snap),
                before == after,
                "freshness disagrees with the bytes"
            );
            Ok(())
        })
        .unwrap();
}

// ---------------------------------------------------------------- P0.3 — evidence units

#[test]
fn units_cover_the_text_exactly_once_and_within_the_cap() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f.rs");
    let reader = WorkspaceReader::new(dir.path()).unwrap();

    // Short lines, a multi-byte character, and occasionally one line far above MAX_UNIT_BYTES so
    // the splitting path is exercised. Capped well under MAX_READ_BYTES on purpose.
    let text = prop::collection::vec(
        prop_oneof![
            9 => "[a-z ]{0,60}",
            1 => Just("\u{e9}\u{1f600}ok".to_string()),
            1 => Just("z".repeat(reader::MAX_UNIT_BYTES + 500)),
        ],
        0..40,
    )
    .prop_map(|lines| lines.join("\n"));

    TestRunner::new(config())
        .run(
            &(text, prop::collection::vec(1u64..30, 0..6)),
            |(body, mut symbol_lines)| {
                symbol_lines.sort_unstable();
                symbol_lines.dedup();
                fs::write(&path, &body).unwrap();
                let snap = match reader.snapshot("f.rs") {
                    Ok(s) => s,
                    // An empty body is not eligible; nothing to assert about its units.
                    Err(_) => return Ok(()),
                };
                let whole = snap.preview_at(usize::MAX);
                let units = reader::units(&snap, &symbol_lines);

                if snap.location_only() {
                    prop_assert!(units.is_empty(), "a location-only file produced units");
                    return Ok(());
                }

                // Contiguous from zero, non-overlapping, covering everything exactly once.
                let mut at = 0usize;
                for u in &units {
                    prop_assert_eq!(u.bytes.start, at, "a gap or an overlap at {}", at);
                    prop_assert!(u.bytes.end > u.bytes.start, "an empty unit");
                    prop_assert!(
                        u.bytes.len() <= reader::MAX_UNIT_BYTES,
                        "a unit of {} bytes over a cap of {}",
                        u.bytes.len(),
                        reader::MAX_UNIT_BYTES
                    );
                    // Slicing here would panic on a boundary that is not a character boundary.
                    let piece = snap.text(u);
                    prop_assert_eq!(piece, &whole[u.bytes.clone()]);
                    // 1-based inclusive lines, consistent with the bytes.
                    let before = whole[..u.bytes.start].matches('\n').count() as u64;
                    prop_assert_eq!(u.start_line, before + 1, "start_line against the bytes");
                    prop_assert!(u.end_line >= u.start_line, "end_line before start_line");
                    at = u.bytes.end;
                }
                // Split by case rather than guarded by `if !units.is_empty()`, which excused
                // exactly the case where the lost unit was the only one — dropping the trailing
                // open unit went undetected until this changed (D-112).
                //
                // An **empty file is eligible** and snapshots, with a digest of no bytes, and has
                // no units. I had assumed it was ineligible; the property said otherwise.
                if whole.is_empty() {
                    prop_assert!(units.is_empty(), "an empty file produced units");
                    return Ok(());
                }
                prop_assert!(!units.is_empty(), "a non-empty file produced no unit");
                prop_assert_eq!(at, whole.len(), "the tail of the text has no unit");

                // A unit starts at every symbol line the file actually has.
                let line_count = whole.split_inclusive('\n').count() as u64;
                for line in symbol_lines.iter().filter(|l| **l <= line_count) {
                    prop_assert!(
                        units.iter().any(|u| u.start_line == *line),
                        "no unit starts at symbol line {}, of {} lines",
                        line,
                        line_count
                    );
                }
                Ok(())
            },
        )
        .unwrap();
}

/// `files_in` is a **lister, not the gate.** It offers names; `snapshot` decides. So a listing
/// may well name `.env` — and the caller in `coordinator` skips it precisely because `snapshot`
/// refuses it ("ineligible siblings are policy, not candidates ripwire named").
///
/// Asserting that a listing never names a refused file failed on `.env`, and that was my test
/// claiming a promise the function never made. What is worth asserting is the boundary that does
/// hold: whatever a listing offers, only eligible files can be **read** (D-112).
#[test]
fn a_listing_offers_names_but_only_eligible_files_can_be_read() {
    let (dir, cases) = policy_root();
    let reader = WorkspaceReader::new(dir.path()).unwrap();
    let refused: Vec<&str> = cases
        .iter()
        .filter(|(_, ok)| !ok)
        .map(|(p, _)| *p)
        .collect();

    let mut offered_a_refused_name = false;
    for d in ["", ".", "src", "node_modules", ".hidden", "target"] {
        for offered in reader.files_in(d) {
            if refused.contains(&offered.as_str()) {
                offered_a_refused_name = true;
                assert!(
                    reader.snapshot(&offered).is_err(),
                    "listing {d:?} offered {offered:?} and it could be read"
                );
            }
        }
    }
    assert!(
        offered_a_refused_name,
        "the listing named nothing refused, so this test proved nothing"
    );
    // And a gitignored file is never offered, which is the walker's job rather than the gate's.
    assert!(
        !reader.files_in("").contains(&"ignored.rs".to_string()),
        "a gitignored file was listed"
    );
    let _ = Path::new("unused");
}
