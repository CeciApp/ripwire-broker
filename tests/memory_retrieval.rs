//! Seam: reading memory back (PRD jev-mem §10): the local index, routing, scoring, stopping and
//! what `context_for_task` gets. A classifier stand-in, no network.
use ripwire_broker::memory::index;
use ripwire_broker::memory::model::Record;
use ripwire_broker::memory::store::State;
use serde_json::json;

fn id(n: u64) -> String {
    format!("{n:064}")
}

fn rec(n: u64, content: &str, paths: &[&str]) -> Record {
    serde_json::from_value(json!({
        "schema_version": 1, "policy_version": "memory-policy/v1",
        "node_id": id(n), "content_hash": id(n), "workspace_id": "w", "event_key": "e",
        "kind": "edit_observation", "content": content, "observed_at_ms": n, "ingest_seq": n,
        "timestamp_role": "observation",
        "entities": paths.iter().map(|p| json!({"id": format!("e:{p}"), "kind": "file", "path": p})).collect::<Vec<_>>(),
        "expires_at_ms": u64::MAX, "generation": 1
    }))
    .unwrap()
}

fn state(records: Vec<Record>) -> State {
    let mut s = State::default();
    for r in records {
        s.nodes.insert(r.node_id.clone(), r);
    }
    s
}

#[test]
fn tokens_are_unicode_alphanumeric_lowercase_without_stemming() {
    assert_eq!(
        index::tokens("Caching cachés Café_42 src/cache.rs"),
        ["caching", "cachés", "café", "42", "src", "cache", "rs"],
        "split on anything not alphanumeric, lowercased, nothing stemmed"
    );
    assert!(index::tokens("  --- ").is_empty());
}

fn three() -> State {
    state(vec![
        rec(1, "cache eviction policy", &["src/cache.rs"]),
        rec(2, "cache warmup", &["src/warm.rs"]),
        rec(3, "router table", &["src/router.rs"]),
    ])
}

#[test]
fn lexical_rank_is_the_sum_of_idf_over_shared_terms() {
    let ranked = index::lexical_rank(&three(), "cache eviction in src/router.rs");
    let ln = f64::ln;
    // N = 3; df(cache) = 2, df(eviction) = df(router) = 1; idf(t) = ln(1 + N / (1 + df)).
    let expected = [
        (id(1), ln(2.0) + ln(2.5)),
        (id(3), ln(2.5)),
        (id(2), ln(2.0)),
    ];
    assert_eq!(ranked.len(), 3);
    for ((got, score), (want, value)) in ranked.iter().zip(expected) {
        assert_eq!(got, &want);
        assert!((score - value).abs() < 1e-12, "{score} vs {value}");
    }
    let twins = state(vec![rec(5, "cache", &[]), rec(4, "cache", &[])]);
    let ids: Vec<String> = index::lexical_rank(&twins, "cache")
        .into_iter()
        .map(|(i, _)| i)
        .collect();
    assert_eq!(ids, [id(4), id(5)], "a tie goes by id");
}

#[test]
fn rrf_fuses_lexical_and_entity_ranks_into_at_most_eight_anchors() {
    let anchors = index::anchors(&three(), "cache eviction in src/router.rs");
    // Lexical ranks: 1, 3, 2 (ids 1, 3, 2). Entity ranks: only id 3, first. RRF k = 60.
    let expected = [
        (id(3), 1.0 / 62.0 + 1.0 / 61.0),
        (id(1), 1.0 / 61.0),
        (id(2), 1.0 / 63.0),
    ];
    assert_eq!(anchors.len(), 3);
    for ((got, score), (want, value)) in anchors.iter().zip(expected) {
        assert_eq!(got, &want);
        assert!((score - value).abs() < 1e-12, "{score} vs {value}");
    }
    let many = state((1..=12).map(|n| rec(n, "cache", &[])).collect());
    assert_eq!(index::anchors(&many, "cache").len(), index::MAX_ANCHORS);
    assert_eq!(index::MAX_ANCHORS, 8);
    assert!(index::anchors(&three(), "nothing matches").is_empty());
}
