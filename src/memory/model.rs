//! The record `memory/v1` (PRD jev-mem §5.1) and its limits. A record over a limit is refused
//! whole, with a reason: cutting it could drop the evidence an assertion rests on.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub const POLICY_VERSION: &str = "memory-policy/v1";
/// `x_t`, the canonical content, in UTF-8 bytes.
pub const MAX_CONTENT_BYTES: usize = 2_000;
pub const MAX_ENTITIES: usize = 16;
pub const MAX_SOURCES: usize = 16;
pub const MAX_TEMPORAL_REFERENCES: usize = 8;
/// The serialized record.
pub const MAX_RECORD_BYTES: usize = 16 * 1024;

/// Why a record was not admitted. Carries no content: it is counted, never logged with text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rejected {
    Malformed,
    UnknownSchema,
    ContentTooLong,
    TooManyEntities,
    TooManySources,
    TooManyTemporalReferences,
    RecordTooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    EditObservation,
    FinishObservation,
    ExplicitNote,
    DerivedNote,
}

/// What a timestamp means; an observation time is never an event time (PRD jev-mem §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampRole {
    Observation,
    ExplicitEvent,
    Unknown,
}

/// An explicit event time with its origin; never inferred from mtime or a commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventTime {
    pub start_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<i64>,
    pub precision: String,
    /// The evidence the time comes from.
    pub source_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalReference {
    pub start_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<i64>,
    pub precision: String,
    pub evidence_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    File,
    Symbol,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub kind: EntityKind,
    /// Relative to the workspace root.
    pub path: String,
    /// The revision descriptor a symbol identity rests on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

/// `μ_t`: where an observation comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// Relative to the workspace root.
    pub path: String,
    /// Of the file's bytes; required for a code source.
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<(u32, u32)>,
    pub verb: String,
    pub basis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ripwire_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHash {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// Sorted by path.
    pub manifest: Vec<FileHash>,
    pub dirty: bool,
}

/// Structured fields; free text only in an explicit, consented note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assertion {
    pub action: String,
    pub outcome: String,
    pub scope: String,
}

/// The four overlapping type scores; `None` is unknown, never zero.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Types {
    pub episodic: Option<f64>,
    pub semantic: Option<f64>,
    pub procedural: Option<f64>,
    pub preference: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrichmentState {
    #[default]
    Pending,
    Partial,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Enrichment {
    pub state: EnrichmentState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parent {
    pub node_id: String,
    pub content_hash: String,
}

/// `o_t = (x_t, τ_t, μ_t)`: one node, shared by the four relational views.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub schema_version: u32,
    pub policy_version: String,
    pub node_id: String,
    pub content_hash: String,
    pub workspace_id: String,
    pub event_key: String,
    pub kind: Kind,
    /// `x_t`, from the renderer `memory-observation/v1`.
    pub content: String,
    /// Local wall clock: evidence of when it was observed, nothing more.
    pub observed_at_ms: u64,
    /// Receipt order in this store; not causality.
    pub ingest_seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_time: Option<EventTime>,
    pub timestamp_role: TimestampRole,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub temporal_references: Vec<TemporalReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<Entity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<Revision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assertion: Option<Assertion>,
    #[serde(default)]
    pub types: Types,
    #[serde(default)]
    pub enrichment: Enrichment,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<Parent>,
    pub expires_at_ms: u64,
    pub generation: u64,
}

impl Record {
    /// Reads a stored or submitted record and admits it only whole and within every limit.
    pub fn parse(bytes: &[u8]) -> Result<Record, Rejected> {
        let record: Record = serde_json::from_slice(bytes).map_err(|_| Rejected::Malformed)?;
        record.check()?;
        Ok(record)
    }

    /// The limits of PRD jev-mem §5.1. Never changes the record.
    pub fn check(&self) -> Result<(), Rejected> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(Rejected::UnknownSchema);
        }
        if self.content.len() > MAX_CONTENT_BYTES {
            return Err(Rejected::ContentTooLong);
        }
        if self.entities.len() > MAX_ENTITIES {
            return Err(Rejected::TooManyEntities);
        }
        if self.sources.len() > MAX_SOURCES {
            return Err(Rejected::TooManySources);
        }
        if self.temporal_references.len() > MAX_TEMPORAL_REFERENCES {
            return Err(Rejected::TooManyTemporalReferences);
        }
        let size = serde_json::to_vec(self).map_or(usize::MAX, |b| b.len());
        if size > MAX_RECORD_BYTES {
            return Err(Rejected::RecordTooLarge);
        }
        Ok(())
    }
}

/// The four relational views over the same nodes (PRD jev-mem §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Graph {
    Semantic,
    Temporal,
    Causal,
    Entity,
}

/// Whether an edge is a fact the store establishes or a classifier's inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeBasis {
    Deterministic,
    JevInference,
}

/// A directed, typed relation between two nodes. An inferred one never feeds callers, tests,
/// contracts or the finish gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub graph: Graph,
    pub relation: String,
    pub basis: EdgeBasis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_version: Option<String>,
    pub policy: String,
    pub generation: u64,
}

impl Edge {
    /// `(source, target, relation, model, prompt, policy)`: the same inference twice is one edge.
    pub fn key(&self) -> String {
        super::identity::hash(&[
            "memory/v1/edge",
            &self.source,
            &self.target,
            &self.relation,
            self.model.as_deref().unwrap_or(""),
            self.prompt_version.as_deref().unwrap_or(""),
            &self.policy,
        ])
    }
}
