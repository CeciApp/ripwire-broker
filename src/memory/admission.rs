//! What an automatic observation may hold (PRD jev-mem §5.2): enumerated fields only, sources
//! that pass the `--online` eligibility policy, and nothing shaped like a secret or personal
//! data. In doubt, nothing is kept. A refusal names its reason and never its content.

use super::identity;
use super::model::{
    Assertion, Kind, POLICY_VERSION, Record, Rejected, SCHEMA_VERSION, Source, TimestampRole,
};
use crate::online::reader::{Ineligible, WorkspaceReader};

pub const RENDERER_VERSION: &str = "memory-observation/v1";

/// Which analysis the broker observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// `context_after_edit`.
    AfterEdit,
    /// `context_before_finish`.
    BeforeFinish,
}

/// What the analysis reported. There is no "tests passed" or "fixed": the broker never knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    AnalysisCompleted,
    AttentionRequired,
}

/// The broker never runs the tests, so their outcome is always unknown to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tests {
    Unknown,
}

/// What the broker saw, structured: the only input of an automatic memory. No prompt,
/// transcript, diff, file body or shell output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub event_key: String,
    pub event: Event,
    pub outcome: Outcome,
    pub tests: Tests,
    /// Paths relative to the workspace root.
    pub scope: Vec<String>,
    /// Names of the analyses the outcome rests on, such as `quality_delta`.
    pub evidence: Vec<String>,
}

/// The store's part of a record: when, in which order and generation, and for how long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub observed_at_ms: u64,
    pub ingest_seq: u64,
    pub generation: u64,
    pub retention_ms: u64,
}

/// Why an observation was not admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// No eligible source to observe.
    Empty,
    Ineligible(Ineligible),
    SecretShaped,
    PersonalData,
    Record(Rejected),
}

impl Refused {
    /// A category for counting; never the refused value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Ineligible(why) => why.as_str(),
            Self::SecretShaped => "secret_shaped",
            Self::PersonalData => "personal_data",
            Self::Record(_) => "record_limit",
        }
    }
}

/// The record for `draft`, or why it is refused whole.
pub fn admit(
    reader: &WorkspaceReader,
    workspace_id: &str,
    draft: &Draft,
    stamp: Stamp,
) -> Result<Record, Refused> {
    for text in draft.scope.iter().chain(&draft.evidence) {
        if secret_shaped(text) {
            return Err(Refused::SecretShaped);
        }
        if personal_data(text) {
            return Err(Refused::PersonalData);
        }
    }
    let mut snapshots = Vec::new();
    for path in &draft.scope {
        snapshots.push(reader.snapshot(path).map_err(Refused::Ineligible)?);
    }
    snapshots.sort_by(|a, b| a.path.cmp(&b.path));
    snapshots.dedup_by(|a, b| a.path == b.path);
    if snapshots.is_empty() {
        return Err(Refused::Empty);
    }
    let (kind, tool, action) = match draft.event {
        Event::AfterEdit => (
            Kind::EditObservation,
            "context_after_edit",
            "analysis_after_edit",
        ),
        Event::BeforeFinish => (
            Kind::FinishObservation,
            "context_before_finish",
            "analysis_before_finish",
        ),
    };
    let paths: Vec<&str> = snapshots.iter().map(|s| s.path.as_str()).collect();
    let mut record = Record {
        schema_version: SCHEMA_VERSION,
        policy_version: POLICY_VERSION.into(),
        node_id: String::new(),
        content_hash: String::new(),
        workspace_id: workspace_id.into(),
        event_key: draft.event_key.clone(),
        kind,
        content: render(draft, &snapshots),
        observed_at_ms: stamp.observed_at_ms,
        ingest_seq: stamp.ingest_seq,
        event_time: None,
        timestamp_role: TimestampRole::Observation,
        temporal_references: vec![],
        entities: paths
            .iter()
            .map(|p| identity::file_entity(workspace_id, p))
            .collect(),
        sources: snapshots
            .iter()
            .map(|s| Source {
                path: s.path.clone(),
                sha256: s.content_hash.clone(),
                symbol: None,
                lines: None,
                verb: tool.into(),
                basis: "broker".into(),
                ripwire_version: None,
            })
            .collect(),
        revision: None,
        assertion: Some(Assertion {
            action: action.into(),
            outcome: outcome_code(draft.outcome).into(),
            scope: paths.join(","),
        }),
        types: Default::default(),
        enrichment: Default::default(),
        derived_from: vec![],
        expires_at_ms: stamp.observed_at_ms.saturating_add(stamp.retention_ms),
        generation: stamp.generation,
    };
    record.content_hash = identity::content_hash(&record);
    record.node_id = identity::node_id(workspace_id, kind, &record.content_hash);
    record.check().map_err(Refused::Record)?;
    Ok(record)
}

fn outcome_code(o: Outcome) -> &'static str {
    match o {
        Outcome::AnalysisCompleted => "analysis_completed",
        Outcome::AttentionRequired => "attention_required",
    }
}

/// `memory-observation/v1`: event, scope, outcome with its origin, evidence and source hashes,
/// in this order. An absence is said as unknown, never as success.
fn render(draft: &Draft, sources: &[crate::online::reader::Snapshot]) -> String {
    let event = match draft.event {
        Event::AfterEdit => "análise após edição",
        Event::BeforeFinish => "análise antes da conclusão",
    };
    let outcome = match draft.outcome {
        Outcome::AnalysisCompleted => "análise concluída",
        Outcome::AttentionRequired => "atenção requerida",
    };
    let tests = match draft.tests {
        Tests::Unknown => "execução de testes desconhecida",
    };
    let scope: Vec<&str> = sources.iter().map(|s| s.path.as_str()).collect();
    let hashes: Vec<&str> = sources.iter().map(|s| s.content_hash.as_str()).collect();
    let mut evidence = draft.evidence.join(", ");
    if !evidence.is_empty() {
        evidence.push_str("; ");
    }
    format!(
        "Evento: {event}. Escopo: {}. Observado pelo broker: {outcome}; {tests}. Evidência: \
         {evidence}revisão de fonte {}.",
        scope.join(", "),
        hashes.join(", ")
    )
}

/// Pieces of text split on anything that cannot be part of a token.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || "-_.@+".contains(c)))
        .filter(|w| !w.is_empty())
}

/// Conservative: well-known credential prefixes and assignments. It does not promise to catch
/// every secret; the fields it scans are short and enumerated (PRD jev-mem §5.2).
fn secret_shaped(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let keyword = [
        "-----begin",
        "bearer ",
        "api_key",
        "apikey",
        "password=",
        "secret=",
        "token=",
    ]
    .iter()
    .any(|k| lower.contains(k));
    keyword
        || words(text).any(|w| {
            let long = w.len() >= 20;
            (long && w.starts_with("sk-"))
                || (w.starts_with("AKIA")
                    && w.len() >= 20
                    && w[4..20]
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()))
                || (long
                    && [
                        "ghp_",
                        "gho_",
                        "ghs_",
                        "ghu_",
                        "github_pat_",
                        "xoxb-",
                        "xoxp-",
                    ]
                    .iter()
                    .any(|p| w.starts_with(p)))
                || (w.starts_with("eyJ") && w.matches('.').count() >= 2)
        })
}

/// Identifiable personal data: an e-mail address.
fn personal_data(text: &str) -> bool {
    words(text).any(|w| match w.split_once('@') {
        Some((user, domain)) => !user.is_empty() && domain.contains('.'),
        None => false,
    })
}

/// A note the operator supplies with `memory add` (PRD jev-mem §5.2). Its text is untrusted
/// data: kept verbatim, never followed as an instruction.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Note {
    pub text: String,
    /// Workspace-relative paths the note is about.
    #[serde(default)]
    pub references: Vec<String>,
}

/// The `explicit_note` record for `note`, under the same filters as an observation.
pub fn admit_note(
    reader: &WorkspaceReader,
    workspace_id: &str,
    note: &Note,
    stamp: Stamp,
) -> Result<Record, Refused> {
    for text in std::iter::once(&note.text).chain(&note.references) {
        if secret_shaped(text) {
            return Err(Refused::SecretShaped);
        }
        if personal_data(text) {
            return Err(Refused::PersonalData);
        }
    }
    if note.text.trim().is_empty() {
        return Err(Refused::Empty);
    }
    let mut snapshots = Vec::new();
    for path in &note.references {
        snapshots.push(reader.snapshot(path).map_err(Refused::Ineligible)?);
    }
    snapshots.sort_by(|a, b| a.path.cmp(&b.path));
    snapshots.dedup_by(|a, b| a.path == b.path);
    let mut record = Record {
        schema_version: SCHEMA_VERSION,
        policy_version: POLICY_VERSION.into(),
        node_id: String::new(),
        content_hash: String::new(),
        workspace_id: workspace_id.into(),
        event_key: "operator_supplied".into(),
        kind: Kind::ExplicitNote,
        content: note.text.clone(),
        observed_at_ms: stamp.observed_at_ms,
        ingest_seq: stamp.ingest_seq,
        event_time: None,
        timestamp_role: TimestampRole::Observation,
        temporal_references: vec![],
        entities: snapshots
            .iter()
            .map(|s| identity::file_entity(workspace_id, &s.path))
            .collect(),
        sources: snapshots
            .iter()
            .map(|s| Source {
                path: s.path.clone(),
                sha256: s.content_hash.clone(),
                symbol: None,
                lines: None,
                verb: "memory_add".into(),
                basis: "operator_supplied".into(),
                ripwire_version: None,
            })
            .collect(),
        revision: None,
        assertion: None,
        types: Default::default(),
        enrichment: Default::default(),
        derived_from: vec![],
        expires_at_ms: stamp.observed_at_ms.saturating_add(stamp.retention_ms),
        generation: stamp.generation,
    };
    record.content_hash = identity::content_hash(&record);
    record.node_id = identity::node_id(workspace_id, record.kind, &record.content_hash);
    record.check().map_err(Refused::Record)?;
    Ok(record)
}
