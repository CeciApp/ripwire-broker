//! `SemanticCache` (PRD §23.8, D-064): in memory, one entry per question. The key is a
//! digest of everything that decides the answer; the value is the validated probability and
//! non-sensitive metadata. No query, path, source or credential is ever stored in clear.
//! The cache never decides relevance.

use super::{SemanticStage, prompt};
use sha2::{Digest, Sha256};
use std::ops::Range;

/// The version of the eligibility and unit policy; part of every key.
pub const POLICY_VERSION: &str = "policy/v1";

/// What decides one answer (v0.1 §15.2).
pub struct KeyParts<'a> {
    pub provider: &'a str,
    pub endpoint: &'a str,
    pub model: &'a str,
    pub stage: SemanticStage,
    pub query: &'a str,
    pub content_hash: &'a str,
    /// Bytes of the snapshot the question showed.
    pub range: Range<usize>,
}

pub type Key = [u8; 32];

/// Measured at ~68 entries per MCP call and ~680 bytes each (D-097): this ceiling holds the
/// semantic cache to about 2.7 MiB, roughly 59 calls of history. Past it, revisiting the same
/// content with the same question can cost a fresh Jev request.
pub const MAX_ENTRIES: usize = 4_000;

pub fn key(p: &KeyParts) -> Key {
    let mut h = Sha256::new();
    let stage = match p.stage {
        SemanticStage::FileAdmission => "file_admission",
        SemanticStage::SourceSelection => "source_selection",
    };
    for part in [
        p.provider,
        p.endpoint,
        p.model,
        prompt::VERSION,
        POLICY_VERSION,
        stage,
        p.query,
        p.content_hash,
        &format!("{}..{}", p.range.start, p.range.end),
    ] {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part.as_bytes());
    }
    h.finalize().into()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cached {
    pub probability: f64,
    /// `sha256:` of the request that first carried the question.
    pub request_digest: String,
    pub stored_at: std::time::SystemTime,
}

#[derive(Debug)]
pub struct SemanticCache {
    /// Oldest insertion first out. `stored_at` cannot order them: one batched answer inserts
    /// dozens of entries within the clock's resolution, and ties would make which entry gives way
    /// depend on the platform.
    entries: crate::fifo::FifoMap<Key, Cached>,
}

impl Default for SemanticCache {
    fn default() -> Self {
        Self {
            entries: crate::fifo::FifoMap::new(MAX_ENTRIES),
        }
    }
}

impl SemanticCache {
    pub fn get(&self, key: &Key) -> Option<&Cached> {
        self.entries.get(key)
    }

    /// Only validated probabilities are stored; an unknown answer is never cached. At
    /// `MAX_ENTRIES` the oldest insertion gives way; updating a key already held is not a new
    /// entry and evicts nothing.
    pub fn insert(&mut self, key: Key, probability: f64, request_digest: String) {
        let cached = Cached {
            probability,
            request_digest,
            stored_at: std::time::SystemTime::now(),
        };
        self.entries.insert(key, cached);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.len() == 0
    }

    /// Hex keys and values, for inspection (CA-ONLINE-13).
    pub fn dump(&self) -> Vec<(String, Cached)> {
        self.entries
            .iter()
            .map(|(k, v)| (k.iter().map(|b| format!("{b:02x}")).collect(), v.clone()))
            .collect()
    }
}
