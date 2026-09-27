//! What a session already received (PRD 11.1, D-029): fingerprints only, never content.

use crate::model::{Item, Note, Risk, TestItem};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// `why_included` of an item already delivered unchanged in this session.
pub const SEEN_REFERENCE: &str = "already delivered in this session (unchanged); call again with include_seen=true for the full item";

/// Fingerprints of what a session was shown. Serializable so hooks can persist it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionMemory {
    seen: BTreeSet<String>,
}

fn digest(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.len().to_le_bytes());
        h.update(p.as_bytes());
    }
    format!("{:x}", h.finalize())
}

/// Location, name, signature and body: a changed body is a different item.
pub(crate) fn item_fingerprint(i: &Item) -> String {
    let line = i.line.map(|l| l.to_string()).unwrap_or_default();
    let content = i
        .content
        .as_ref()
        .map(|c| c.untrusted_repository_data.as_str())
        .unwrap_or("");
    digest(&[
        "item",
        &i.path,
        &line,
        i.symbol.as_deref().unwrap_or(""),
        i.signature.as_deref().unwrap_or(""),
        content,
    ])
}

pub(crate) fn test_fingerprint(t: &TestItem) -> String {
    digest(&["test", &t.path, t.run.as_deref().unwrap_or("")])
}

pub(crate) fn risk_fingerprint(r: &Risk) -> String {
    digest(&[
        "risk",
        r.kind,
        r.path.as_deref().unwrap_or(""),
        r.symbol.as_deref().unwrap_or(""),
        &r.message,
    ])
}

pub(crate) fn note_fingerprint(n: &Note) -> String {
    digest(&["note", &n.scope, &n.text.untrusted_repository_data])
}

/// The same item reduced to a pointer.
pub(crate) fn reference(i: &Item) -> Item {
    Item {
        signature: None,
        content: None,
        why_included: SEEN_REFERENCE.into(),
        ..i.clone()
    }
}

impl SessionMemory {
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    /// Whether this exact risk was delivered before; gate risks are resent, so callers that
    /// must tell old from new ask here.
    pub fn knows_risk(&self, r: &Risk) -> bool {
        self.has(&risk_fingerprint(r))
    }

    pub(crate) fn has(&self, fingerprint: &str) -> bool {
        self.seen.contains(fingerprint)
    }

    pub(crate) fn remember(&mut self, fingerprint: String) {
        self.seen.insert(fingerprint);
    }
}
