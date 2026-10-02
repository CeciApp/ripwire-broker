//! The dirty files of a git working tree and when each last changed (D-129): how a shell command
//! that edited files is told apart from one that only read, before ripwire starts.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Past this many `git status` entries the tree is "too dirty to follow" and has no fingerprint,
/// so the session state never grows with it.
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
    /// The repository's top directory: two fingerprints compare only within one repository.
    /// Empty in a fingerprint saved before it.
    #[serde(default)]
    pub top: String,
    pub entries: Vec<Entry>,
}

/// Why a tree has no fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unusable {
    /// Not a git repository, or git is missing or failed: cheap to find out again.
    NotGit,
    /// git did not answer within the budget.
    TooSlow,
    /// More than `MAX_FINGERPRINT_ENTRIES` entries.
    TooDirty,
}

impl Unusable {
    /// Whether asking again is likely to cost as much for the same answer, so the session should
    /// stop asking (D-129): a slow or very dirty tree stays so, while a failing git fails fast.
    pub fn switches_off(self) -> bool {
        matches!(self, Unusable::TooSlow | Unusable::TooDirty)
    }
}

/// The fingerprint of the repository holding `root`, within the usual git budget.
pub fn fingerprint(root: &Path) -> Result<Fingerprint, Unusable> {
    fingerprint_within(root, GIT_TIMEOUT)
}

/// The fingerprint of the repository holding `root`, with `budget` for both git calls together.
pub fn fingerprint_within(root: &Path, budget: Duration) -> Result<Fingerprint, Unusable> {
    let deadline = Instant::now() + budget;
    let top = git(root, &["rev-parse", "--show-toplevel"], deadline)?;
    let top = String::from_utf8(top).map_err(|_| Unusable::NotGit)?;
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
        // A `stat` per dirty file is time too: a slow filesystem counts against the same budget.
        if Instant::now() >= deadline {
            return Err(Unusable::TooSlow);
        }
        let path = top.join(rel);
        entries.push(Entry {
            stamp: stamp(&path),
            path: path.to_string_lossy().into_owned(),
        });
        if entries.len() > MAX_FINGERPRINT_ENTRIES {
            return Err(Unusable::TooDirty);
        }
    }
    Ok(Fingerprint {
        top: top.to_string_lossy().into_owned(),
        entries,
    })
}

/// The paths that are new, changed or deleted in `after`, then those dirty in `before` and clean
/// in `after` (a revert or a commit is an edit too). Once each. A fingerprint of another
/// repository is no baseline: its dirty files are not this command's edits.
pub fn changed(before: &Fingerprint, after: &Fingerprint) -> Vec<String> {
    if before.top != after.top {
        return vec![];
    }
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
/// the user's lock) and returns stdout. A repository chosen by the caller's environment
/// (`GIT_DIR` and friends, set when the host itself runs under a git hook) would describe some
/// other tree than `root`'s, so those variables are dropped.
fn git(root: &Path, args: &[&str], deadline: Instant) -> Result<Vec<u8>, Unusable> {
    if Instant::now() >= deadline {
        return Err(Unusable::TooSlow);
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Unusable::NotGit)?;
    // Read on a thread: a long listing would fill the pipe and stall the child past the deadline.
    // The answer comes back over a channel with the same deadline: a process that git (or a
    // wrapper) left behind can hold the pipe open long after git exits, and a plain `join` would
    // wait for it. Such a reader is abandoned; it ends when the pipe closes.
    let mut stdout = child.stdout.take().ok_or(Unusable::NotGit)?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = tx.send(stdout.read_to_end(&mut buf).map(|_| buf));
    });
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let left = deadline.saturating_duration_since(Instant::now());
                let out = match rx.recv_timeout(left) {
                    Ok(read) => read.map_err(|_| Unusable::NotGit)?,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        return Err(Unusable::TooSlow);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(Unusable::NotGit);
                    }
                };
                return if status.success() {
                    Ok(out)
                } else {
                    Err(Unusable::NotGit)
                };
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(2)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Unusable::TooSlow);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Unusable::NotGit);
            }
        }
    }
}
