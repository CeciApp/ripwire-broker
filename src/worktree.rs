//! The dirty files of a git working tree and when each last changed (D-129): how a shell command
//! that edited files is told apart from one that only read, before ripwire starts.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Past this many dirty files the tree is "too dirty to follow" and has no fingerprint, so the
/// session state never grows with it.
pub const MAX_FINGERPRINT_ENTRIES: usize = 5000;

/// For both git calls together: a hook must never hold the host up for long.
const GIT_TIMEOUT: Duration = Duration::from_millis(500);

/// When a file last changed, as far as a cheap `stat` can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    pub secs: u64,
    pub nanos: u32,
    pub size: u64,
}

/// One dirty path, absolute; no stamp when it no longer exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    pub stamp: Option<Stamp>,
}

/// The dirty files of a tree, in `git status` order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub entries: Vec<Entry>,
}

/// The fingerprint of the repository holding `root`; `None` outside git, when git fails or is
/// slow, or past `MAX_FINGERPRINT_ENTRIES`.
pub fn fingerprint(root: &Path) -> Option<Fingerprint> {
    let deadline = Instant::now() + GIT_TIMEOUT;
    let top = git(root, &["rev-parse", "--show-toplevel"], deadline)?;
    let top = String::from_utf8(top).ok()?;
    let top = Path::new(top.trim_end_matches('\n'));
    let out = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        deadline,
    )?;
    let mut entries = Vec::new();
    let mut fields = out.split(|b| *b == 0).filter(|f| !f.is_empty());
    while let Some(field) = fields.next() {
        // "XY path": two status letters, a space, the path relative to the repository root.
        if field.len() < 4 {
            continue;
        }
        // A rename or copy is followed by one more field, the original path.
        if field[..2].iter().any(|c| matches!(c, b'R' | b'C')) {
            fields.next();
        }
        let Ok(rel) = std::str::from_utf8(&field[3..]) else {
            continue;
        };
        let path = top.join(rel);
        entries.push(Entry {
            stamp: stamp(&path),
            path: path.to_string_lossy().into_owned(),
        });
        if entries.len() > MAX_FINGERPRINT_ENTRIES {
            return None;
        }
    }
    Some(Fingerprint { entries })
}

/// The paths that are new, changed or deleted in `after`, then those dirty in `before` and clean
/// in `after` (a revert or a commit is an edit too). Once each.
pub fn changed(before: &Fingerprint, after: &Fingerprint) -> Vec<String> {
    let was: HashMap<&str, Option<Stamp>> = before
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e.stamp))
        .collect();
    let now: HashSet<&str> = after.entries.iter().map(|e| e.path.as_str()).collect();
    let mut out: Vec<String> = after
        .entries
        .iter()
        .filter(|e| was.get(e.path.as_str()) != Some(&e.stamp))
        .map(|e| e.path.clone())
        .collect();
    out.extend(
        before
            .entries
            .iter()
            .filter(|e| !now.contains(e.path.as_str()))
            .map(|e| e.path.clone()),
    );
    let mut seen = HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    let t = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(Stamp {
        secs: t.as_secs(),
        nanos: t.subsec_nanos(),
        size: meta.len(),
    })
}

/// Runs git read-only (`GIT_OPTIONAL_LOCKS=0`: `status` must not refresh the index or contend for
/// the user's lock) and returns stdout, or `None` on failure or past `deadline`.
fn git(root: &Path, args: &[&str], deadline: Instant) -> Option<Vec<u8>> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Read on a thread: a long listing would fill the pipe and stall the child past the deadline.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        stdout.read_to_end(&mut buf).map(|_| buf)
    });
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = reader.join().ok()?.ok()?;
                return status.success().then_some(out);
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(2)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}
