//! Workspace guard (RF-02): every path the broker forwards must resolve inside the fixed root.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// Canonicalizes the root once, at startup.
    pub fn new(root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|e| format!("workspace {}: {e}", root.display()))?;
        if !root.is_dir() {
            return Err(format!("workspace {} is not a directory", root.display()));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns `path` relative to the root, refusing `..` escapes, outside absolute paths and
    /// symlinks that lead out of the root. Paths that do not exist yet (deleted files) are
    /// checked through their nearest existing ancestor.
    pub fn relative(&self, path: &str) -> Result<String, String> {
        let refuse = || format!("path '{path}' is outside the workspace");
        let joined = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.root.join(path)
        };
        let lexical = normalize(&joined).ok_or_else(refuse)?;
        let resolved = resolve_existing(&lexical);
        let rel = resolved.strip_prefix(&self.root).map_err(|_| refuse())?;
        Ok(rel.to_string_lossy().into_owned())
    }

    /// Symbols may be `@FILE:LINE` line seeds; their file part obeys the same rule.
    pub fn check_symbol(&self, symbol: &str) -> Result<(), String> {
        if let Some(seed) = symbol.strip_prefix('@') {
            let file = seed.rsplit_once(':').map(|(f, _)| f).unwrap_or(seed);
            self.relative(file)?;
        }
        Ok(())
    }
}

/// Lexically resolves `.` and `..`; `None` if `..` climbs above the filesystem root.
fn normalize(p: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Some(out)
}

/// Canonicalizes the longest existing prefix (following symlinks) and re-appends the rest.
fn resolve_existing(p: &Path) -> PathBuf {
    let mut existing = p.to_path_buf();
    let mut rest = Vec::new();
    while !existing.exists() {
        match (
            existing.file_name().map(|n| n.to_os_string()),
            existing.parent(),
        ) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                existing = parent.to_path_buf();
            }
            _ => return p.to_path_buf(),
        }
    }
    let mut out = existing.canonicalize().unwrap_or(existing);
    for name in rest.into_iter().rev() {
        out.push(name);
    }
    out
}
