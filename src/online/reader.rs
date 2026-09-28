//! `WorkspaceReader` (PRD §23.4, §23.9): the only way workspace text reaches the classifier.
//! It applies the eligibility policy, reads a snapshot bound to its sha256, and cuts previews
//! and units from that snapshot, so a preview, a question, its answer and a range all refer to
//! the same version of the source.

use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

/// Initial file preview for `file_admission` (v0.1 §6.3).
pub const PREVIEW_BYTES: usize = 16 * 1024;
/// Preview of a lookahead file: ripwire never ranked it, so a short look decides whether it
/// deserves a unit-level one; about 8 fit in a request instead of 2 (D-081).
pub const LOOKAHEAD_PREVIEW_BYTES: usize = 4 * 1024;
/// Target size of a textual chunk when there is no structural unit (v0.1 §9.5).
pub const CHUNK_BYTES: usize = 3 * 1024;
/// A unit above this is split (v0.1 §9.5).
pub const MAX_UNIT_BYTES: usize = 24 * 1024;
/// Above this a file is a location only: admitted or not, no source is selected (v0.1 §9.5).
pub const LOCATION_ONLY_BYTES: usize = 1024 * 1024;
/// Never read more than this to hash a file.
pub const MAX_READ_BYTES: usize = 8 * 1024 * 1024;

/// Why a path is never read for sending. Carries no path or content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ineligible {
    /// Absolute, or escaping the root.
    Outside,
    SensitiveName,
    /// A component starting with `.`, which covers `.git` and other VCS metadata.
    Hidden,
    DependencyOrBuild,
    /// A symlink anywhere below the root; never followed.
    Symlink,
    /// A directory, socket, device or FIFO.
    NotRegular,
    Unreadable,
    /// Excluded by `.gitignore`, `.ignore` or git's exclude files.
    Ignored,
    TooLarge,
    Binary,
    NotUtf8,
    PrivateKey,
}

impl Ineligible {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Outside => "outside",
            Self::SensitiveName => "sensitive_name",
            Self::Hidden => "hidden",
            Self::DependencyOrBuild => "dependency_or_build",
            Self::Symlink => "symlink",
            Self::NotRegular => "not_regular",
            Self::Unreadable => "unreadable",
            Self::Ignored => "ignored",
            Self::TooLarge => "too_large",
            Self::Binary => "binary",
            Self::NotUtf8 => "not_utf8",
            Self::PrivateKey => "private_key",
        }
    }
}

const DEPENDENCY_OR_BUILD: &[&str] = &[
    "node_modules",
    "target",
    "vendor",
    "dist",
    "build",
    "__pycache__",
    "bower_components",
    "venv",
];

const SENSITIVE_NAMES: &[&str] = &[
    ".env",
    ".envrc",
    ".netrc",
    ".npmrc",
    ".pypirc",
    ".pgpass",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    "credentials",
    "credentials.json",
    "secrets.json",
    "secrets.yaml",
    "secrets.yml",
];

/// `env` covers environment files without the leading dot, such as `prod.env` (D-089).
const SENSITIVE_EXTENSIONS: &[&str] = &[
    "pem", "key", "p12", "pfx", "jks", "keystore", "kdbx", "gpg", "env",
];

fn sensitive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    SENSITIVE_NAMES.contains(&lower.as_str())
        || lower.starts_with(".env.")
        || Path::new(&lower)
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|e| SENSITIVE_EXTENSIONS.contains(&e))
}

/// One version of one eligible file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Relative to the root; the absolute root is never sent (PRD §23.9).
    pub path: String,
    /// `sha256:<hex>` of the bytes read.
    pub content_hash: String,
    text: String,
}

impl Snapshot {
    /// At most [`PREVIEW_BYTES`], ending on a line break when there is one, else on a
    /// character boundary.
    pub fn preview(&self) -> &str {
        self.preview_at(PREVIEW_BYTES)
    }

    /// At most `max` bytes, cut like [`Self::preview`].
    pub fn preview_at(&self, max: usize) -> &str {
        if self.text.len() <= max {
            return &self.text;
        }
        let head = &self.text[..floor_char(&self.text, max)];
        match head.rfind('\n') {
            Some(i) => &head[..=i],
            None => head,
        }
    }

    pub fn location_only(&self) -> bool {
        self.text.len() > LOCATION_ONLY_BYTES
    }

    /// The text of a unit cut from this snapshot.
    pub fn text(&self, unit: &Unit) -> &str {
        &self.text[unit.bytes.clone()]
    }
}

fn floor_char(s: &str, at: usize) -> usize {
    (0..=at.min(s.len()))
        .rev()
        .find(|i| s.is_char_boundary(*i))
        .unwrap_or(0)
}

pub struct WorkspaceReader {
    root: PathBuf,
}

impl WorkspaceReader {
    pub fn new(root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|e| format!("workspace {}: {e}", root.display()))?;
        Ok(Self { root })
    }

    /// Reads `rel` if the eligibility policy allows it (v0.1 §9.1).
    pub fn snapshot(&self, rel: &str) -> Result<Snapshot, Ineligible> {
        let parts = relative_parts(rel).ok_or(Ineligible::Outside)?;
        let name = parts.last().ok_or(Ineligible::Outside)?;
        if sensitive_name(name) {
            return Err(Ineligible::SensitiveName);
        }
        if parts.iter().any(|p| p.starts_with('.')) {
            return Err(Ineligible::Hidden);
        }
        if parts[..parts.len() - 1]
            .iter()
            .any(|p| DEPENDENCY_OR_BUILD.contains(&p.as_str()))
        {
            return Err(Ineligible::DependencyOrBuild);
        }
        // Uma travessia para todo o caminho, consultada por componente abaixo. Construir um
        // `ignore::Walk` custa cerca de 100 µs, e fazê-lo por componente dominava o custo de
        // `snapshot` (D-096); a ordem de precedência dos motivos não muda.
        let admitted = admitted_prefixes(&self.root, &parts);
        let mut path = self.root.clone();
        for (i, part) in parts.iter().enumerate() {
            path.push(part);
            let meta = std::fs::symlink_metadata(&path).map_err(|_| Ineligible::Unreadable)?;
            if meta.file_type().is_symlink() {
                return Err(Ineligible::Symlink);
            }
            let last = i == parts.len() - 1;
            if last && !meta.is_file() {
                return Err(Ineligible::NotRegular);
            }
            if last && meta.len() > MAX_READ_BYTES as u64 {
                return Err(Ineligible::TooLarge);
            }
            if !admitted.contains(Path::new(&parts[..=i].join("/"))) {
                return Err(Ineligible::Ignored);
            }
        }
        let bytes = std::fs::read(&path).map_err(|_| Ineligible::Unreadable)?;
        if bytes.len() > MAX_READ_BYTES {
            return Err(Ineligible::TooLarge);
        }
        if bytes[..bytes.len().min(8000)].contains(&0) {
            return Err(Ineligible::Binary);
        }
        let content_hash = format!("sha256:{:x}", Sha256::digest(&bytes));
        let text = String::from_utf8(bytes).map_err(|_| Ineligible::NotUtf8)?;
        if text.contains("-----BEGIN") && text.contains("PRIVATE KEY-----") {
            return Err(Ineligible::PrivateKey);
        }
        Ok(Snapshot {
            path: parts.join("/"),
            content_hash,
            text,
        })
    }

    /// Regular files directly in `dir` (relative; `""` is the root), in path order, without
    /// descending and without following symlinks. What is listed still has to pass
    /// [`Self::snapshot`] before anything is read.
    pub fn files_in(&self, dir: &str) -> Vec<String> {
        let Some(parts) = relative_parts(dir) else {
            return vec![];
        };
        let mut abs = self.root.clone();
        abs.extend(&parts);
        let mut out: Vec<String> = ignore::WalkBuilder::new(&abs)
            .max_depth(Some(1))
            .hidden(false)
            .parents(true)
            .require_git(false)
            .follow_links(false)
            .build()
            .flatten()
            .filter(|e| e.depth() == 1 && e.file_type().is_some_and(|t| t.is_file()))
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .map(|name| match parts.is_empty() {
                true => name,
                false => format!("{}/{name}", parts.join("/")),
            })
            .collect();
        out.sort();
        out
    }

    /// Whether `snap` still matches the file: eligible and with the same hash.
    pub fn is_fresh(&self, snap: &Snapshot) -> bool {
        self.snapshot(&snap.path)
            .is_ok_and(|now| now.content_hash == snap.content_hash)
    }
}

/// `rel` as normal components; `None` for absolute paths and any `..`.
fn relative_parts(rel: &str) -> Option<Vec<String>> {
    let mut parts = vec![];
    for c in Path::new(rel).components() {
        match c {
            Component::Normal(p) => parts.push(p.to_str()?.to_string()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(parts)
}

/// Every prefix of `parts` that a walk from `root` honouring the ignore files on the way
/// lists, as paths relative to `root`. An ignored directory is absent, and so is everything
/// under it, because the walk never descends into it. The walk is pruned to the target path,
/// so it visits one directory per component instead of the subtree.
fn admitted_prefixes(root: &Path, parts: &[String]) -> HashSet<PathBuf> {
    let target: PathBuf = parts.iter().collect();
    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .max_depth(Some(parts.len()))
        .hidden(false)
        .parents(true)
        .require_git(false)
        .follow_links(false);
    let (pruned_root, pruned_target) = (root.to_path_buf(), target.clone());
    builder.filter_entry(move |e| match e.path().strip_prefix(&pruned_root) {
        Ok(rel) => rel.as_os_str().is_empty() || pruned_target.starts_with(rel),
        // Not below the root: leave the decision to the rest of the policy.
        Err(_) => true,
    });
    builder
        .build()
        .flatten()
        .filter_map(|e| {
            e.path()
                .strip_prefix(root)
                .ok()
                .filter(|rel| !rel.as_os_str().is_empty())
                .map(Path::to_path_buf)
        })
        .collect()
}

/// A unit of evidence for `source_selection`: whole lines of one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// One-based, inclusive (the external form of a range, PRD §23.2).
    pub start_line: u64,
    pub end_line: u64,
    /// Half-open byte range into the snapshot's text.
    pub bytes: Range<usize>,
    /// The ripwire symbol line this unit starts at, when it starts at one.
    pub symbol_line: Option<u64>,
}

/// Units of `snap` (RF-ONLINE-07, D-061): a new unit starts at every ripwire symbol line
/// in `symbol_lines`; otherwise units close once they reach ~3 KiB of whole lines, and a
/// line above 24 KiB is cut into pieces. A location-only file has none.
pub fn units(snap: &Snapshot, symbol_lines: &[u64]) -> Vec<Unit> {
    if snap.location_only() {
        return vec![];
    }
    let mut out = vec![];
    let mut open: Option<Unit> = None;
    let mut offset = 0;
    for (n, line) in snap.text.split_inclusive('\n').enumerate() {
        let number = n as u64 + 1;
        let span = offset..offset + line.len();
        offset = span.end;
        let starts_symbol = symbol_lines.contains(&number);
        let too_big = open
            .as_ref()
            .is_some_and(|u| u.bytes.len() + line.len() > MAX_UNIT_BYTES);
        if starts_symbol || too_big {
            out.extend(open.take());
        }
        if line.len() > MAX_UNIT_BYTES {
            out.extend(open.take());
            out.extend(pieces(&snap.text, span, number, starts_symbol));
            continue;
        }
        let unit = open.get_or_insert(Unit {
            start_line: number,
            end_line: number,
            bytes: span.start..span.start,
            symbol_line: starts_symbol.then_some(number),
        });
        unit.end_line = number;
        unit.bytes.end = span.end;
        if unit.bytes.len() >= CHUNK_BYTES {
            out.extend(open.take());
        }
    }
    out.extend(open);
    out
}

/// A single long line cut into pieces of at most [`MAX_UNIT_BYTES`] on character boundaries.
fn pieces(text: &str, span: Range<usize>, line: u64, symbol: bool) -> Vec<Unit> {
    let mut out = vec![];
    let mut start = span.start;
    while start < span.end {
        let end = start + floor_char(&text[start..span.end], MAX_UNIT_BYTES);
        out.push(Unit {
            start_line: line,
            end_line: line,
            bytes: start..end,
            symbol_line: (symbol && out.is_empty()).then_some(line),
        });
        start = end;
    }
    out
}
