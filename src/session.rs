//! What a session already received (PRD 11.1, D-029): fingerprints only, never content.

use crate::model::{Item, Note, Risk, TestItem};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, VecDeque};

/// `why_included` of an item already delivered unchanged in this session.
pub const SEEN_REFERENCE: &str = "already delivered in this session (unchanged); call again with include_seen=true for the full item";

/// Measured at ~6 fingerprints per MCP call and ~128 bytes each (D-097): about 640 KiB,
/// roughly 800 calls of history. It also bounds the state file the hooks rewrite per event.
pub const MAX_REMEMBERED: usize = 5_000;

/// Fingerprints of what a session was shown. Serializable so hooks can persist it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMemory {
    seen: BTreeSet<String>,
    /// Insertion order of what this process remembered, oldest first, so eviction drops the
    /// oldest rather than an arbitrary fingerprint. Deliberately not serialized: the state
    /// file's shape is unchanged, and a memory restored from disk therefore carries no order,
    /// so its entries are evicted in fingerprint order until this process refills the queue.
    #[serde(skip)]
    order: VecDeque<String>,
}

/// Only the fingerprints decide whether two memories are the same; the insertion order is
/// bookkeeping for eviction and is absent from anything persisted.
impl PartialEq for SessionMemory {
    fn eq(&self, other: &Self) -> bool {
        self.seen == other.seen
    }
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

    /// The fingerprints themselves, for counting overlap between sessions; never content.
    pub(crate) fn fingerprints(&self) -> impl Iterator<Item = &str> {
        self.seen.iter().map(String::as_str)
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
        if self.seen.insert(fingerprint.clone()) {
            self.order.push_back(fingerprint);
        }
        self.trim();
    }

    /// Down to `MAX_REMEMBERED`, oldest first. Forgetting a fingerprint only costs a repeated
    /// delivery of that one item, never a wrong answer.
    pub(crate) fn trim(&mut self) {
        while self.seen.len() > MAX_REMEMBERED {
            let oldest = self
                .order
                .pop_front()
                .or_else(|| self.seen.iter().next().cloned());
            match oldest {
                Some(f) => {
                    self.seen.remove(&f);
                }
                None => break,
            }
        }
    }
}
