//! Seam: what the memory admits (PRD jev-mem §5). The record `memory/v1`, its limits, and the
//! admission filters in front of the store.
use ripwire_broker::memory::model::{
    MAX_CONTENT_BYTES, MAX_ENTITIES, MAX_RECORD_BYTES, MAX_SOURCES, MAX_TEMPORAL_REFERENCES,
    Record, Rejected,
};
use serde_json::{Value, json};

/// A valid automatic observation, as JSON: the shape the spool stores.
fn record() -> Value {
    json!({
        "schema_version": 1,
        "policy_version": "memory-policy/v1",
        "node_id": "n".repeat(64),
        "content_hash": "c".repeat(64),
        "workspace_id": "w".repeat(64),
        "event_key": "e".repeat(64),
        "kind": "edit_observation",
        "content": "Evento: análise após edição. Escopo: src/cache.rs.",
        "observed_at_ms": 1_000,
        "ingest_seq": 1,
        "timestamp_role": "observation",
        "entities": [{"id": "x".repeat(64), "kind": "file", "path": "src/cache.rs"}],
        "sources": [{
            "path": "src/cache.rs",
            "sha256": "a".repeat(64),
            "verb": "quality_delta",
            "basis": "ripwire"
        }],
        "expires_at_ms": 2_000,
        "generation": 1
    })
}

fn parse(v: &Value) -> Result<Record, Rejected> {
    Record::parse(&serde_json::to_vec(v).unwrap())
}

#[test]
fn a_record_round_trips_and_an_oversized_one_is_refused_whole() {
    let r = parse(&record()).expect("a valid record is admitted");
    let again = Record::parse(&serde_json::to_vec(&r).unwrap()).unwrap();
    assert_eq!(r, again, "JSON round trip");
    assert_eq!(r.schema_version, 1);
    assert!(r.event_time.is_none(), "never inferred");

    // Absent type scores are unknown, never zero (PRD jev-mem §5.1).
    assert_eq!(r.types.episodic, None);
    assert_eq!(r.types.semantic, None);
    assert_eq!(r.types.procedural, None);
    assert_eq!(r.types.preference, None);

    let mut v = record();
    v["content"] = json!("é".repeat(MAX_CONTENT_BYTES / 2) + "x");
    assert_eq!(parse(&v), Err(Rejected::ContentTooLong), "2.001 bytes");
    v["content"] = json!("x".repeat(MAX_CONTENT_BYTES));
    assert!(parse(&v).is_ok(), "2.000 bytes fit");

    let mut v = record();
    let entity = v["entities"][0].clone();
    v["entities"] = json!(vec![entity; MAX_ENTITIES + 1]);
    assert_eq!(parse(&v), Err(Rejected::TooManyEntities));

    let mut v = record();
    let source = v["sources"][0].clone();
    v["sources"] = json!(vec![source; MAX_SOURCES + 1]);
    assert_eq!(parse(&v), Err(Rejected::TooManySources));

    let mut v = record();
    let reference = json!({"start_ms": 1, "precision": "day", "evidence_id": "s0"});
    v["temporal_references"] = json!(vec![reference.clone(); MAX_TEMPORAL_REFERENCES + 1]);
    assert_eq!(parse(&v), Err(Rejected::TooManyTemporalReferences));
    v["temporal_references"] = json!(vec![reference; MAX_TEMPORAL_REFERENCES]);
    assert!(parse(&v).is_ok(), "eight references fit");

    // Every list within its own cap, the whole still over 16 KiB: refused, not cut.
    let mut v = record();
    let mut source = v["sources"][0].clone();
    source["path"] = json!("p".repeat(1_100));
    v["sources"] = json!(vec![source; MAX_SOURCES]);
    assert!(serde_json::to_vec(&v).unwrap().len() > MAX_RECORD_BYTES);
    assert_eq!(parse(&v), Err(Rejected::RecordTooLarge));

    let r = parse(&record()).unwrap();
    let mut big = r.clone();
    big.content = "x".repeat(MAX_CONTENT_BYTES + 1);
    assert_eq!(big.check(), Err(Rejected::ContentTooLong));
    assert_eq!(
        big.content.len(),
        MAX_CONTENT_BYTES + 1,
        "checking never truncates the evidence"
    );
    let mut wide = r.clone();
    let mut source = wide.sources[0].clone();
    source.path = "p".repeat(1_100);
    wide.sources = vec![source; MAX_SOURCES];
    assert_eq!(
        wide.check(),
        Err(Rejected::RecordTooLarge),
        "a record built in process"
    );

    assert_eq!(Record::parse(b"{"), Err(Rejected::Malformed));
    let mut v = record();
    v["schema_version"] = json!(2);
    assert_eq!(parse(&v), Err(Rejected::UnknownSchema));
}
