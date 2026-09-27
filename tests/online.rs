//! Seam 1 with the `--online` adapter: the Broker core, a `FakeUpstream` and a scripted
//! `FakeClassifier` over a real workspace on disk (PRD §23.4, §23.7, §23.12).
mod common;

use common::classifier::FakeClassifier;
use common::fake::FakeUpstream;
use ripwire_broker::broker::{Broker, BrokerConfig, EditRequest, FinishRequest, TaskRequest};
use ripwire_broker::online::classifier::ClassifyError;
use ripwire_broker::online::{OnlineConfig, SemanticStage};
use ripwire_broker::upstream::UpstreamError;
use serde_json::Value;
use std::sync::Arc;

const AUTH: &str = "def validate_token(token):\n    return token == \"ok\"\n\n\ndef login(user, token):\n    if not validate_token(token):\n        raise ValueError(\"bad token\")\n    return user\n\n\ndef export_report(user, token):\n    login(user, token)\n    return \"report\"\n";
const ROUTES: &str = "from src.auth import export_report, login\n\n\ndef export_route(req):\n    return export_report(req.user, req.token)\n\n\ndef home_route(req):\n    return login(req.user, req.token)\n";
const TASK: &str = "how are the routes authenticated?";

/// The files `explore_export_auth` talks about, so the reader has something to read.
fn workspace() -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    common::write(ws.path(), "src/auth.py", AUTH);
    common::write(ws.path(), "src/routes.py", ROUTES);
    common::write(ws.path(), "docs/auth.md", "# Authentication decision\n");
    common::write(
        ws.path(),
        "tests/test_auth.py",
        "def test_login():\n    pass\n",
    );
    ws
}

struct Setup {
    broker: Broker,
    upstream: Arc<FakeUpstream>,
    classifier: Arc<FakeClassifier>,
    _ws: tempfile::TempDir,
}

async fn online(upstream: FakeUpstream, classifier: FakeClassifier) -> Setup {
    online_in(workspace(), upstream, classifier).await
}

async fn online_in(
    ws: tempfile::TempDir,
    upstream: FakeUpstream,
    classifier: FakeClassifier,
) -> Setup {
    let (upstream, classifier) = (Arc::new(upstream), Arc::new(classifier));
    let mut config = BrokerConfig::new(ws.path());
    config.online = Some(OnlineConfig::new(classifier.clone()));
    let broker = Broker::connect(upstream.clone(), config).await.unwrap();
    Setup {
        broker,
        upstream,
        classifier,
        _ws: ws,
    }
}

fn explore() -> FakeUpstream {
    FakeUpstream::new().answer("explore", "explore_export_auth")
}

/// Admits `src/auth.py` only, and selects the block that defines `login`.
fn login_is_evidence() -> FakeClassifier {
    FakeClassifier::new().rule(|stage, item| match stage {
        SemanticStage::FileAdmission => Some(if item.path == "src/auth.py" { 0.9 } else { 0.1 }),
        SemanticStage::SourceSelection => Some(if item.text.starts_with("def login") {
            0.83
        } else {
            0.1
        }),
    })
}

fn json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap()
}

fn items(out: &Value) -> &Vec<Value> {
    out["items"].as_array().unwrap()
}

fn limitation_kinds(out: &Value) -> Vec<String> {
    out["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["kind"].as_str().unwrap().to_string())
        .collect()
}

async fn offline_items(upstream: FakeUpstream) -> Vec<(Value, Value)> {
    let ws = workspace();
    let b = Broker::connect(Arc::new(upstream), BrokerConfig::new(ws.path()))
        .await
        .unwrap();
    let out = json(&b.context_for_task(TaskRequest::new(TASK)).await.unwrap());
    items(&out)
        .iter()
        .map(|i| (i["path"].clone(), i["symbol"].clone()))
        .collect()
}

// --- S4.20 / S4.21: which calls reach the classifier ---

#[tokio::test]
async fn the_classifier_runs_on_explore_routes() {
    let s = online(explore(), login_is_evidence()).await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    assert!(s.classifier.calls() > 0);
    let o = &out["provenance"]["online"];
    assert_eq!(o["enabled"], true);
    assert_eq!(o["provider"], "typesafe");
    assert_eq!(o["model"], "jev-1.13.0");
    assert_eq!(o["discovery"], "complete");
    assert_eq!(o["incomplete"], false);
    assert_eq!(
        o["requests"].as_u64().unwrap() as usize,
        s.classifier.calls()
    );
}

#[tokio::test]
async fn a_missing_symbol_falls_back_to_explore_and_runs_the_classifier() {
    let upstream = explore().fail(
        "find_symbol",
        UpstreamError::Refused("symbol not found".into()),
    );
    let s = online(upstream, login_is_evidence()).await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new("rename `apply_patch` in routes"))
            .await
            .unwrap(),
    );

    assert!(s.classifier.calls() > 0);
    assert_eq!(out["provenance"]["online"]["discovery"], "complete");
}

#[tokio::test]
async fn other_routes_skip_the_classifier_and_say_so() {
    let trace = "Traceback (most recent call last):\n  File \"src/routes.py\", line 9, in home_route\n    return login(req.user, req.token)\n  File \"src/auth.py\", line 7, in login\n    raise ValueError(\"bad token\")\nValueError: bad token";
    for (upstream, task) in [
        (
            FakeUpstream::new().answer("from_trace", "from_trace_login"),
            trace,
        ),
        (
            FakeUpstream::new()
                .answer("find_symbol", "find_symbol_login")
                .answer("fetch_body", "fetch_body_login"),
            "how does `login` work?",
        ),
        (
            FakeUpstream::new().answer("memory_recall", "memory_recall_auth"),
            "why was the auth decision documented?",
        ),
        (
            FakeUpstream::new().answer("situational_awareness", "situational_awareness_edit"),
            "review my change",
        ),
    ] {
        let s = online(upstream, login_is_evidence()).await;

        let out = json(
            &s.broker
                .context_for_task(TaskRequest::new(task))
                .await
                .unwrap(),
        );

        assert_eq!(s.classifier.calls(), 0, "{task}");
        assert_eq!(
            out["provenance"]["online"]["discovery"], "skipped",
            "{task}"
        );
        assert_eq!(out["provenance"]["online"]["requests"], 0);
        assert!(
            limitation_kinds(&out).contains(&"semantic_skipped".to_string()),
            "{task}: {out:#}"
        );
        assert!(
            items(&out).iter().all(|i| i.get("semantic").is_none()),
            "never claims online use"
        );
    }
}

#[tokio::test]
async fn after_edit_and_before_finish_never_call_the_classifier() {
    let upstream = FakeUpstream::new()
        .answer("situational_awareness", "situational_awareness_edit")
        .answer("quality_delta", "quality_delta_clean")
        .answer("affected", "affected_auth");
    let s = online(upstream, login_is_evidence()).await;

    let edit = json(
        &s.broker
            .context_after_edit(EditRequest::default())
            .await
            .unwrap(),
    );
    let finish = json(
        &s.broker
            .context_before_finish(FinishRequest::default())
            .await
            .unwrap(),
    );

    assert_eq!(s.classifier.calls(), 0);
    assert!(edit["provenance"].get("online").is_none());
    assert!(finish["provenance"].get("online").is_none());
}

// --- S4.22: admission, then selection ---

#[tokio::test]
async fn planner_paths_are_admitted_then_their_units_selected() {
    let s = online(explore(), login_is_evidence()).await;

    s.broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap();

    let admitted = s.classifier.asked(SemanticStage::FileAdmission);
    assert!(
        admitted.contains(&"src/auth.py".to_string())
            && admitted.contains(&"src/routes.py".to_string()),
        "{admitted:?}"
    );
    assert!(
        !admitted.contains(&"docs/auth.md".to_string()),
        "docs are not candidates"
    );
    let selected = s.classifier.asked(SemanticStage::SourceSelection);
    assert!(!selected.is_empty());
    assert!(
        selected.iter().all(|p| p == "src/auth.py"),
        "only admitted files reach selection: {selected:?}"
    );
    assert_eq!(
        s.upstream.called(),
        vec!["explore"],
        "no extra ripwire calls in phase 4"
    );
}

// --- S4.23–S4.25: additive merge ---

#[tokio::test]
async fn a_symbol_found_by_both_is_one_item_with_two_provenances() {
    let s = online(explore(), login_is_evidence()).await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    let logins: Vec<&Value> = items(&out)
        .iter()
        .filter(|i| i["symbol"] == "login")
        .collect();
    assert_eq!(logins.len(), 1, "no duplicate (CA-ONLINE-07)");
    let login = logins[0];
    assert_eq!(
        login["source"]["basis"], "ripwire",
        "the structural provenance stays"
    );
    let sem = &login["semantic"];
    assert_eq!(sem["stage"], "source_selection");
    assert_eq!(sem["state"], "selected_source");
    assert_eq!(sem["probability"], 0.83);
    assert_eq!(sem["threshold"], 0.5);
    assert_eq!(sem["model"], "jev-1.13.0");
    assert_eq!(sem["lines"], serde_json::json!([5, 10]));
    assert!(sem["content_hash"].as_str().unwrap().starts_with("sha256:"));
    assert!(
        sem["request_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(sem["cache_hit"], false);
}

#[tokio::test]
async fn an_unlinked_range_survives_as_a_semantic_location_without_relations() {
    // The import block of routes.py (lines 1-3) holds no ripwire symbol.
    let classifier = FakeClassifier::new().rule(|stage, item| match stage {
        SemanticStage::FileAdmission => Some(0.9),
        SemanticStage::SourceSelection => Some(if item.text.starts_with("from src.auth") {
            0.7
        } else {
            0.1
        }),
    });
    let s = online(explore(), classifier).await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    let loc = items(&out)
        .iter()
        .find(|i| i["kind"] == "semantic_location")
        .unwrap_or_else(|| panic!("{out:#}"));
    assert_eq!(loc["path"], "src/routes.py");
    assert_eq!(loc["line"], 1);
    assert!(loc.get("symbol").is_none());
    assert_eq!(loc["role"], "semantic");
    assert_eq!(
        loc["source"],
        serde_json::json!({"verb": "jev", "basis": "remote_classifier"})
    );
    assert_eq!(loc["semantic"]["state"], "selected_source");
    assert!(
        loc["content"]["untrusted_repository_data"]
            .as_str()
            .unwrap()
            .starts_with("from src.auth")
    );
    assert!(loc["why_included"].as_str().unwrap().contains("0.70"));
    let offline_tests = 0; // explore_export_auth carries no tests; none may be invented
    assert_eq!(out["tests"].as_array().unwrap().len(), offline_tests);
    assert!(
        out["risks"].as_array().unwrap().is_empty(),
        "CA-ONLINE-08: no relation invented"
    );
}

#[tokio::test]
async fn a_reading_lead_has_no_source() {
    let classifier = FakeClassifier::new().rule(|stage, item| match stage {
        SemanticStage::FileAdmission => Some(0.9),
        SemanticStage::SourceSelection => Some(if item.text.starts_with("from src.auth") {
            0.4
        } else {
            0.1
        }),
    });
    let s = online(explore(), classifier).await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    let lead = items(&out)
        .iter()
        .find(|i| i["kind"] == "semantic_location")
        .unwrap();
    assert_eq!(lead["semantic"]["state"], "reading_lead");
    assert!(
        lead.get("content").is_none(),
        "a lead is a location, never source"
    );
}

#[tokio::test]
async fn structural_facts_are_kept_and_never_lowered_when_the_classifier_rejects_them() {
    let before = offline_items(explore()).await;
    let s = online(explore(), FakeClassifier::new().otherwise(0.01)).await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    let after: Vec<(Value, Value)> = items(&out)
        .iter()
        .map(|i| (i["path"].clone(), i["symbol"].clone()))
        .collect();
    assert_eq!(after, before, "same facts, same order");
    let login = items(&out).iter().find(|i| i["symbol"] == "login").unwrap();
    assert_eq!(
        login["semantic"]["state"], "rejected",
        "the disagreement is shown, not hidden"
    );
}

#[tokio::test]
async fn a_selected_structural_symbol_is_promoted_never_demoted() {
    let before = offline_items(explore()).await;
    let s = online(explore(), login_is_evidence()).await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    let pos = |list: &[(Value, Value)], sym: &str| list.iter().position(|(_, s)| s == sym).unwrap();
    let after: Vec<(Value, Value)> = items(&out)
        .iter()
        .map(|i| (i["path"].clone(), i["symbol"].clone()))
        .collect();
    assert!(
        pos(&after, "login") < pos(&before, "login"),
        "{before:?} → {after:?}"
    );
    for (path, sym) in &before {
        assert!(after.contains(&(path.clone(), sym.clone())), "{sym} kept");
    }
}

// --- S4.26: partial failure ---

#[tokio::test]
async fn a_failing_classifier_keeps_the_structural_answer_and_marks_incomplete() {
    let before = offline_items(explore()).await;
    let mut classifier = FakeClassifier::new();
    for id in ["f0", "f1", "f2", "f3"] {
        classifier = classifier.fail(id, ClassifyError::Server(503));
    }
    let s = online(explore(), classifier).await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    assert_eq!(out["status"], "ready");
    let after: Vec<(Value, Value)> = items(&out)
        .iter()
        .map(|i| (i["path"].clone(), i["symbol"].clone()))
        .collect();
    assert_eq!(after, before, "CA-ONLINE-15");
    assert_eq!(out["provenance"]["online"]["incomplete"], true);
    assert_eq!(out["provenance"]["online"]["discovery"], "incomplete");
    assert!(limitation_kinds(&out).contains(&"semantic_incomplete".to_string()));
}

#[tokio::test]
async fn ineligible_candidates_are_never_sent_and_leave_the_answer_incomplete() {
    let ws = workspace();
    common::write(ws.path(), ".gitignore", "src/routes.py\n");
    let s = online_in(ws, explore(), login_is_evidence()).await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    assert!(
        !s.classifier
            .asked(SemanticStage::FileAdmission)
            .contains(&"src/routes.py".to_string())
    );
    let lim = out["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["kind"] == "semantic_not_sent")
        .unwrap();
    assert!(lim["detail"].as_str().unwrap().contains("ignored"), "{lim}");
    assert!(
        !lim["detail"].as_str().unwrap().contains("routes"),
        "no paths in the detail"
    );
}

// --- S5.1: freshness (RF-ONLINE-10, CA-ONLINE-11) ---

/// Edits `src/auth.py` the first time the classifier is asked about `stage`.
fn editing_on(stage: SemanticStage, root: std::path::PathBuf) -> FakeClassifier {
    let done = std::sync::atomic::AtomicBool::new(false);
    login_is_evidence().on_call(move |s| {
        if s == stage && !done.swap(true, std::sync::atomic::Ordering::SeqCst) {
            common::write(&root, "src/auth.py", &AUTH.replace("report", "summary"));
        }
    })
}

#[tokio::test]
async fn a_file_changed_before_sending_is_not_sent_and_marks_incomplete() {
    let ws = workspace();
    let root = ws.path().to_path_buf();
    // The file changes while admission is answered: its selection batch is now stale.
    let s = online_in(
        ws,
        explore(),
        editing_on(SemanticStage::FileAdmission, root),
    )
    .await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    assert!(
        s.classifier
            .asked(SemanticStage::SourceSelection)
            .is_empty(),
        "a batch over changed source is never sent"
    );
    assert_eq!(out["provenance"]["online"]["discovery"], "incomplete");
    let lim = out["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["kind"] == "semantic_incomplete")
        .unwrap();
    assert!(lim["detail"].as_str().unwrap().contains("changed"), "{lim}");
    let login = items(&out).iter().find(|i| i["symbol"] == "login").unwrap();
    assert!(
        login.get("semantic").is_none(),
        "no evidence about a version that no longer exists"
    );
}

#[tokio::test]
async fn evidence_stale_at_output_is_dropped_and_marked_incomplete() {
    let ws = workspace();
    let root = ws.path().to_path_buf();
    // The file changes while selection is answered: the answer is about the old version.
    let s = online_in(
        ws,
        explore(),
        editing_on(SemanticStage::SourceSelection, root),
    )
    .await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    assert!(
        !s.classifier
            .asked(SemanticStage::SourceSelection)
            .is_empty(),
        "it was sent"
    );
    assert_eq!(out["provenance"]["online"]["incomplete"], true);
    assert!(
        items(&out)
            .iter()
            .all(|i| i["path"] != "src/auth.py" || i.get("semantic").is_none()),
        "{out:#}"
    );
    assert!(
        items(&out).iter().any(|i| i["symbol"] == "login"),
        "the structural fact stays"
    );
    let routes = items(&out)
        .iter()
        .find(|i| i["path"] == "src/routes.py")
        .unwrap();
    assert!(
        routes.get("semantic").is_some(),
        "unchanged files keep their evidence"
    );
}

// --- S5.5: discovery deadline and rendered source cap ---

async fn online_with(
    ws: tempfile::TempDir,
    classifier: FakeClassifier,
    tune: impl FnOnce(&mut OnlineConfig),
) -> Setup {
    let (upstream, classifier) = (Arc::new(explore()), Arc::new(classifier));
    let mut config = BrokerConfig::new(ws.path());
    let mut online = OnlineConfig::new(classifier.clone());
    tune(&mut online);
    config.online = Some(online);
    let broker = Broker::connect(upstream.clone(), config).await.unwrap();
    Setup {
        broker,
        upstream,
        classifier,
        _ws: ws,
    }
}

#[tokio::test(start_paused = true)]
async fn the_discovery_deadline_returns_interrupted_with_fresh_evidence() {
    // Admission answers at once; the first selection batch would take ten seconds.
    let classifier = login_is_evidence().delay("u0", std::time::Duration::from_secs(10));
    let s = online_with(workspace(), classifier, |o| {
        o.deadline = std::time::Duration::from_millis(200)
    })
    .await;
    let t0 = tokio::time::Instant::now();

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    assert!(
        tokio::time::Instant::now() < t0 + std::time::Duration::from_secs(1),
        "the answer does not wait"
    );
    assert_eq!(out["status"], "ready");
    assert_eq!(out["provenance"]["online"]["discovery"], "interrupted");
    assert_eq!(out["provenance"]["online"]["incomplete"], true);
    let lim = out["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["kind"] == "semantic_incomplete")
        .unwrap();
    assert!(
        lim["detail"].as_str().unwrap().contains("deadline"),
        "{lim}"
    );
    let login = items(&out).iter().find(|i| i["symbol"] == "login").unwrap();
    assert_eq!(
        login["semantic"]["stage"], "file_admission",
        "admission evidence acquired before the deadline stays"
    );
    assert_eq!(
        s.classifier
            .counters
            .dropped
            .load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the slow request was aborted"
    );
}

#[tokio::test]
async fn a_deadline_that_is_not_reached_changes_nothing() {
    let s = online_with(workspace(), login_is_evidence(), |o| {
        o.deadline = std::time::Duration::from_secs(8)
    })
    .await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    assert_eq!(out["provenance"]["online"]["discovery"], "complete");
}

#[tokio::test]
async fn the_rendered_source_cap_turns_selected_blocks_into_locations() {
    // The import block of routes.py is selected but has no ripwire symbol.
    let classifier = FakeClassifier::new().rule(|stage, item| match stage {
        SemanticStage::FileAdmission => Some(0.9),
        SemanticStage::SourceSelection => Some(if item.text.starts_with("from src.auth") {
            0.7
        } else {
            0.1
        }),
    });
    let s = online_with(workspace(), classifier, |o| o.max_source_bytes = Some(10)).await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    let loc = items(&out)
        .iter()
        .find(|i| i["kind"] == "semantic_location")
        .unwrap();
    assert_eq!(
        loc["semantic"]["state"], "selected_source",
        "it was still evaluated and selected"
    );
    assert!(
        loc.get("content").is_none(),
        "but its source is not rendered"
    );
    assert!(
        loc["why_included"]
            .as_str()
            .unwrap()
            .contains("--jev-max-source-bytes")
    );
    assert!(limitation_kinds(&out).contains(&"semantic_source_capped".to_string()));
}

// --- S5.7–S5.9: one-level lookahead ---

const BUDGET_PY: &str =
    "def fit(envelope, budget):\n    while size(envelope) > budget:\n        envelope.pop()\n";

/// Admits `src/auth.py` and the lookahead file `src/budget.py`, and selects `fit`.
fn budget_is_evidence() -> FakeClassifier {
    FakeClassifier::new().rule(|stage, item| match stage {
        SemanticStage::FileAdmission => Some(
            if item.path == "src/auth.py" || item.path == "src/budget.py" {
                0.9
            } else {
                0.1
            },
        ),
        SemanticStage::SourceSelection => Some(if item.text.starts_with("def fit") {
            0.8
        } else {
            0.1
        }),
    })
}

fn with_siblings() -> tempfile::TempDir {
    let ws = workspace();
    common::write(ws.path(), "src/budget.py", BUDGET_PY);
    common::write(ws.path(), "src/sub/deep.py", "def deep():\n    pass\n");
    common::write(ws.path(), "src/.hidden.py", "x = 1\n");
    common::write(ws.path(), "src/server.pem", "-----BEGIN CERTIFICATE-----\n");
    ws
}

#[tokio::test]
async fn lookahead_admits_eligible_siblings_of_admitted_planner_paths() {
    let s = online_in(with_siblings(), explore(), budget_is_evidence()).await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    let admitted = s.classifier.asked(SemanticStage::FileAdmission);
    assert!(
        admitted.contains(&"src/budget.py".to_string()),
        "a sibling of admitted src/auth.py: {admitted:?}"
    );
    assert!(
        !admitted.contains(&"src/sub/deep.py".to_string()),
        "one level only, no descent"
    );
    assert_eq!(
        admitted.iter().filter(|p| *p == "src/routes.py").count(),
        1,
        "planner paths are not asked twice"
    );
    assert!(
        !admitted
            .iter()
            .any(|p| p.starts_with("tests/") && p != "tests/test_auth.py"),
        "tests/ was not admitted"
    );
    let found = items(&out)
        .iter()
        .find(|i| i["path"] == "src/budget.py")
        .unwrap_or_else(|| panic!("{out:#}"));
    assert_eq!(found["kind"], "semantic_location");
    assert_eq!(found["semantic"]["state"], "selected_source");
    assert!(
        found["why_included"].as_str().unwrap().contains("beside"),
        "{found}"
    );
    assert!(
        found.get("symbol").is_none() && out["tests"].as_array().unwrap().is_empty(),
        "no relation invented"
    );
}

#[tokio::test]
async fn lookahead_respects_eligibility_and_its_cap() {
    let ws = with_siblings();
    for n in 0..10 {
        common::write(ws.path(), &format!("src/extra_{n:02}.py"), "x = 1\n");
    }
    let s = online_with(ws, budget_is_evidence(), |o| o.lookahead_max = 3).await;

    s.broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap();

    let admitted = s.classifier.asked(SemanticStage::FileAdmission);
    for never in ["src/.hidden.py", "src/server.pem"] {
        assert!(
            !admitted.contains(&never.to_string()),
            "{never} is never sent"
        );
    }
    let lookahead: Vec<&String> = admitted
        .iter()
        .filter(|p| !["src/routes.py", "src/auth.py", "tests/test_auth.py"].contains(&p.as_str()))
        .collect();
    assert_eq!(
        lookahead,
        vec!["src/budget.py", "src/extra_00.py", "src/extra_01.py"],
        "path order, capped at 3"
    );
}

#[tokio::test]
async fn lookahead_files_are_admitted_with_short_previews() {
    let ws = with_siblings();
    let big: String = (0..300)
        .map(|n| format!("    step_{n} = shrink(envelope)  # keep it small\n"))
        .collect();
    common::write(ws.path(), "src/budget.py", &format!("{BUDGET_PY}{big}"));
    common::write(ws.path(), "src/auth.py", &format!("{AUTH}{big}"));
    let s = online_in(ws, explore(), budget_is_evidence()).await;

    s.broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap();

    let seen = s.classifier.seen.lock().unwrap().clone();
    let sent = |path: &str| -> usize {
        seen.iter()
            .filter(|r| FakeClassifier::stage(r) == SemanticStage::FileAdmission)
            .flat_map(|r| r.state.items.iter())
            .find(|i| i.path == path)
            .map(|i| i.text.len())
            .unwrap()
    };
    assert!(
        sent("src/budget.py") <= 4096,
        "lookahead: {}",
        sent("src/budget.py")
    );
    assert!(
        sent("src/auth.py") > 4096,
        "planner keeps the 16 KiB preview: {}",
        sent("src/auth.py")
    );
}

#[tokio::test]
async fn selection_starts_with_the_most_likely_admitted_file() {
    let classifier = FakeClassifier::new().rule(|stage, item| match stage {
        SemanticStage::FileAdmission => Some(match item.path.as_str() {
            "src/budget.py" => 0.95,
            "src/auth.py" => 0.6,
            _ => 0.1,
        }),
        SemanticStage::SourceSelection => Some(0.8),
    });
    let s = online_in(with_siblings(), explore(), classifier).await;

    s.broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap();

    let selection = s.classifier.asked(SemanticStage::SourceSelection);
    assert_eq!(
        selection.first().map(String::as_str),
        Some("src/budget.py"),
        "{selection:?}"
    );
}

#[tokio::test]
async fn semantic_locations_are_ordered_by_probability_within_their_band() {
    // The planner's routes.py import block is selected at 0.6, the lookahead budget.py at 0.9.
    let classifier = FakeClassifier::new().rule(|stage, item| match stage {
        SemanticStage::FileAdmission => Some(if item.path == "tests/test_auth.py" {
            0.1
        } else {
            0.9
        }),
        SemanticStage::SourceSelection => Some(if item.text.starts_with("def fit") {
            0.9
        } else if item.text.starts_with("from src.auth") {
            0.6
        } else {
            0.1
        }),
    });
    let s = online_in(with_siblings(), explore(), classifier).await;

    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    let order: Vec<&str> = items(&out)
        .iter()
        .filter(|i| i["kind"] == "semantic_location")
        .map(|i| i["path"].as_str().unwrap())
        .collect();
    assert_eq!(order.first(), Some(&"src/budget.py"), "{order:?}");
    let before = offline_items(explore()).await;
    let structural: Vec<(Value, Value)> = items(&out)
        .iter()
        .filter(|i| i["kind"] != "semantic_location")
        .map(|i| (i["path"].clone(), i["symbol"].clone()))
        .collect();
    assert_eq!(structural, before, "ripwire's items keep their order");
}

#[tokio::test]
async fn a_semantic_only_candidate_is_reported_as_gain_beyond_ripwire() {
    let s = online_in(with_siblings(), explore(), budget_is_evidence()).await;
    s.broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap();
    assert_eq!(
        json(&s.broker.status().await)["online"]["semantic_only_candidates"],
        1
    );

    let none = online_in(with_siblings(), explore(), login_is_evidence()).await;
    none.broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap();
    assert_eq!(
        json(&none.broker.status().await)["online"]["semantic_only_candidates"],
        0,
        "a rejected sibling is no gain"
    );
}

#[tokio::test]
async fn without_ripwire_no_semantic_discovery_is_invented() {
    let s = online(
        FakeUpstream::new().fail("explore", UpstreamError::Unavailable("down".into())),
        budget_is_evidence(),
    )
    .await;

    let err = s
        .broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap_err();

    assert_eq!(
        err.error, "upstream_unavailable",
        "CA-07: a structured error, no fabricated context"
    );
    assert_eq!(
        s.classifier.calls(),
        0,
        "no candidates without ripwire, so no lookahead either"
    );
}

// --- S4.27: budget ---

#[tokio::test]
async fn online_answers_never_exceed_the_budget_and_report_omissions() {
    let s = online(explore(), FakeClassifier::new().otherwise(0.9)).await;
    for budget in (512..=4000).step_by(118) {
        let mut req = TaskRequest::new(TASK);
        req.budget_tokens = budget;

        let out = json(&s.broker.context_for_task(req).await.unwrap());

        let est = out["budget"]["estimated_tokens"].as_u64().unwrap();
        assert!(est <= budget as u64, "CA-ONLINE-14: {est} > {budget}");
        if out["budget"]["truncated"] == true {
            assert!(out["budget"]["omitted"].as_u64().unwrap() > 0);
        }
    }
}

#[tokio::test]
async fn an_online_task_needs_room_for_its_provenance() {
    let s = online(explore(), FakeClassifier::new()).await;
    let mut req = TaskRequest::new(TASK);
    req.budget_tokens = 511;

    let err = s.broker.context_for_task(req).await.unwrap_err();

    assert_eq!(err.error, "invalid_input");
    assert!(err.message.contains("512"), "{}", err.message);
    assert_eq!(s.classifier.calls(), 0, "refused before any request");
    let edit = s
        .broker
        .context_after_edit(EditRequest {
            budget_tokens: 256,
            ..Default::default()
        })
        .await;
    assert!(
        !matches!(&edit, Err(e) if e.error == "invalid_input"),
        "the other tools keep the offline floor: {edit:?}"
    );
}

#[tokio::test]
async fn the_worst_case_online_limitations_fit_the_online_floor() {
    // Every online limitation at once: a policy exclusion, failures and the uncertain route.
    let ws = workspace();
    common::write(ws.path(), ".gitignore", "src/routes.py\n");
    let mut classifier = FakeClassifier::new();
    for id in ["f0", "f1", "f2"] {
        classifier = classifier.fail(id, ClassifyError::Server(503));
    }
    let s = online_in(ws, explore(), classifier).await;
    let mut req = TaskRequest::new(TASK);
    req.budget_tokens = 512;

    let out = json(&s.broker.context_for_task(req).await.unwrap());

    assert!(limitation_kinds(&out).contains(&"semantic_not_sent".to_string()));
    assert!(limitation_kinds(&out).contains(&"semantic_incomplete".to_string()));
    assert!(
        out["budget"]["estimated_tokens"].as_u64().unwrap() <= 512,
        "{out:#}"
    );
}

// --- S4.28: envelope extensions ---

#[tokio::test]
async fn offline_envelopes_have_no_online_fields() {
    let ws = workspace();
    let b = Broker::connect(Arc::new(explore()), BrokerConfig::new(ws.path()))
        .await
        .unwrap();

    let text =
        serde_json::to_string(&b.context_for_task(TaskRequest::new(TASK)).await.unwrap()).unwrap();

    for key in [
        "\"online\"",
        "\"semantic\"",
        "semantic_location",
        "remote_classifier",
        "semantic_skipped",
    ] {
        assert!(!text.contains(key), "{key} in an offline envelope");
    }
}

// --- S4.29: cache ---

#[tokio::test]
async fn a_cached_decision_skips_the_classifier() {
    let s = online(explore(), login_is_evidence()).await;
    let first = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );
    let calls = s.classifier.calls();

    let second = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    assert_eq!(
        s.classifier.calls(),
        calls,
        "every decision came from the cache"
    );
    assert_eq!(second["provenance"]["online"]["requests"], 0);
    assert!(
        second["provenance"]["online"]["cache_hits"]
            .as_u64()
            .unwrap()
            > 0
    );
    let login = items(&second)
        .iter()
        .find(|i| i["symbol"] == "login")
        .unwrap();
    assert_eq!(login["semantic"]["cache_hit"], true);
    assert_eq!(login["semantic"]["probability"], 0.83);
    assert_eq!(items(&first).len(), items(&second).len());
}

#[tokio::test]
async fn the_cache_misses_when_the_source_changes() {
    let ws = workspace();
    let root = ws.path().to_path_buf();
    let s = online_in(ws, explore(), login_is_evidence()).await;
    s.broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap();
    let calls = s.classifier.calls();

    common::write(&root, "src/auth.py", &AUTH.replace("report", "summary"));
    s.broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap();

    let asked_again: Vec<String> = s.classifier.asked(SemanticStage::FileAdmission);
    assert!(s.classifier.calls() > calls);
    assert_eq!(
        asked_again.iter().filter(|p| *p == "src/routes.py").count(),
        1,
        "unchanged file: cache hit"
    );
    assert_eq!(
        asked_again.iter().filter(|p| *p == "src/auth.py").count(),
        2,
        "changed file: asked again"
    );
}

// --- S4.30: status ---

#[tokio::test]
async fn the_status_reports_online_health_without_content() {
    let s = online(explore(), login_is_evidence()).await;
    s.broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap();

    let status = json(&s.broker.status().await);

    assert_eq!(status["offline"], false);
    let o = &status["online"];
    assert_eq!(o["enabled"], true);
    assert_eq!(o["provider"], "typesafe");
    assert_eq!(o["model"], "jev-1.13.0");
    assert_eq!(o["endpoint_host"], "api.typesafe.ai");
    assert_eq!(o["max_in_flight"], 4);
    assert_eq!(o["request_limit"], 24);
    assert_eq!(
        o["requests"].as_u64().unwrap() as usize,
        s.classifier.calls()
    );
    assert!(o["cached_decisions"].as_u64().unwrap() > 0);
    assert_eq!(o["last_error"], Value::Null);
    let text = status.to_string();
    for secret in ["routes are authenticated", "src/auth.py", "def login"] {
        assert!(!text.contains(secret), "{secret} leaked into the status");
    }
}

// --- S5.10 / S5.11: metrics and stages (PRD §23.11) ---

const METRICS: [&str; 15] = [
    "jev_requests_total",
    "jev_questions_total",
    "jev_in_flight",
    "jev_batch_items",
    "jev_request_bytes",
    "jev_response_bytes",
    "jev_latency_ms",
    "jev_cache_hits_total",
    "jev_rate_limit_total",
    "jev_retry_total",
    "jev_split_total",
    "semantic_candidates_total",
    "semantic_selected_ranges_total",
    "semantic_only_candidates_total",
    "online_context_tokens_estimated",
];

#[tokio::test]
async fn online_metrics_are_counts_and_times_only() {
    let s = online_in(with_siblings(), explore(), budget_is_evidence()).await;
    s.broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap();

    let status = json(&s.broker.status().await);
    let m = &status["online"]["metrics"];

    for key in METRICS {
        assert!(m.get(key).is_some(), "{key} missing: {m:#}");
    }
    let seen = s.classifier.seen.lock().unwrap().clone();
    assert_eq!(m["jev_requests_total"], seen.len());
    assert_eq!(
        m["jev_questions_total"],
        seen.iter().map(|r| r.questions.0.len()).sum::<usize>()
    );
    assert_eq!(m["jev_in_flight"], 0);
    assert_eq!(
        m["jev_batch_items"]["max"],
        seen.iter().map(|r| r.state.items.len()).max().unwrap()
    );
    assert_eq!(
        m["jev_request_bytes"]["total"],
        seen.iter()
            .map(|r| serde_json::to_string(r).unwrap().len())
            .sum::<usize>()
    );
    let lat = &m["jev_latency_ms"];
    assert!(lat["p50"].as_f64().unwrap() <= lat["p95"].as_f64().unwrap());
    assert!(lat["p95"].as_f64().unwrap() <= lat["p99"].as_f64().unwrap());
    assert_eq!(m["jev_retry_total"], 0);
    assert!(
        m["semantic_candidates_total"].as_u64().unwrap() >= 4,
        "planner and lookahead files"
    );
    assert!(m["semantic_selected_ranges_total"].as_u64().unwrap() >= 1);
    assert_eq!(m["semantic_only_candidates_total"], 1);
    assert!(m["online_context_tokens_estimated"].as_u64().unwrap() > 0);
    let text = m.to_string();
    for secret in ["routes are authenticated", "src/", "def fit", "budget.py"] {
        assert!(!text.contains(secret), "{secret} in the metrics");
    }
}

#[tokio::test]
async fn retries_splits_and_rate_limits_are_counted() {
    let classifier = budget_is_evidence()
        .fail_times("f0", ClassifyError::Server(503), 1)
        .fail_times(
            "f1",
            ClassifyError::RateLimited {
                retry_after: Some("0".into()),
            },
            1,
        );
    let s = online_in(with_siblings(), explore(), classifier).await;
    s.broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap();

    let m = json(&s.broker.status().await)["online"]["metrics"].clone();

    assert_eq!(
        m["jev_split_total"], 1,
        "the multi-file admission batch split after a 503"
    );
    assert!(m["jev_retry_total"].as_u64().unwrap() >= 1, "{m}");
    assert_eq!(m["jev_rate_limit_total"], 1);
}

#[tokio::test]
async fn each_stage_is_recorded_under_the_request_id() {
    let s = online_in(with_siblings(), explore(), budget_is_evidence()).await;
    let out = json(
        &s.broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap(),
    );

    let status = json(&s.broker.status().await);
    let last = status["metrics"]["recent_requests"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();

    assert_eq!(last["request_id"], out["provenance"]["request_id"]);
    let stages: Vec<&str> = last["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["stage"].as_str().unwrap())
        .collect();
    assert_eq!(
        stages,
        vec![
            "semantic.navigation.batch",
            "semantic.selection.batch",
            "semantic.discovery",
            "context.merge",
            "context.budget"
        ]
    );
    for stage in last["stages"].as_array().unwrap() {
        assert!(stage["us"].as_u64().is_some());
    }
    assert!(last["stages"][0]["batches"].as_u64().unwrap() >= 1);
    assert!(!last.to_string().contains("src/") && !last.to_string().contains("authenticated"));
}

#[tokio::test]
async fn offline_requests_have_no_stages() {
    let ws = workspace();
    let b = Broker::connect(Arc::new(explore()), BrokerConfig::new(ws.path()))
        .await
        .unwrap();
    b.context_for_task(TaskRequest::new(TASK)).await.unwrap();

    let status = json(&b.status().await);

    assert!(
        status["metrics"]["recent_requests"][0]
            .get("stages")
            .is_none()
    );
}

#[tokio::test]
async fn the_status_keeps_the_last_error_category_only() {
    let s = online(
        explore(),
        FakeClassifier::new().fail("f0", ClassifyError::Server(503)),
    )
    .await;
    s.broker
        .context_for_task(TaskRequest::new(TASK))
        .await
        .unwrap();

    let status = json(&s.broker.status().await);

    assert_eq!(status["online"]["last_error"], "server");
}

#[tokio::test]
async fn an_offline_status_has_no_online_block() {
    let ws = workspace();
    let b = Broker::connect(Arc::new(explore()), BrokerConfig::new(ws.path()))
        .await
        .unwrap();

    let status = json(&b.status().await);

    assert_eq!(status["offline"], true);
    assert!(status.get("online").is_none());
}

#[test]
fn the_published_schema_carries_the_online_floor() {
    let floor = |online: bool, tool: &str| -> Value {
        let tools = json(&ripwire_broker::mcp::tools(online));
        let t = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == tool)
            .unwrap()
            .clone();
        t["inputSchema"]["properties"]["budget_tokens"]["minimum"].clone()
    };
    assert_eq!(floor(false, "context_for_task"), 256);
    assert_eq!(floor(true, "context_for_task"), 512);
    assert_eq!(
        floor(true, "context_after_edit"),
        256,
        "the classifier never runs there"
    );
    assert_eq!(floor(true, "context_before_finish"), 256);
}

/// Stop point 2 measurement (PRD §23.5): local batching and merge overhead, without ripwire
/// latency or network. `cargo test --release --test online overhead -- --ignored --nocapture`
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn overhead_of_batching_and_merge() {
    let ws = tempfile::tempdir().unwrap();
    let mut ctx = String::from("<ctx schema=\"ripwire.pack-task/v1\" task=\"t\"><sigs>");
    for f in 0..16 {
        let body: String = (0..400)
            .map(|l| format!("    value_{l} = compute({l}) + {f}\n"))
            .collect();
        common::write(
            ws.path(),
            &format!("src/m{f}.py"),
            &format!("def f{f}():\n{body}"),
        );
        ctx.push_str(&format!(
            "<d l=\"1\" n=\"f{f}\" p=\"src/m{f}.py\" r=\"{}\">def f{f}():</d>",
            f + 1
        ));
    }
    ctx.push_str("</sigs></ctx>");
    let classifier = Arc::new(FakeClassifier::new().otherwise(0.6));
    let mut config = BrokerConfig::new(ws.path());
    let mut online = OnlineConfig::new(classifier.clone());
    online.cache = false;
    online.request_limit = 1000;
    config.online = Some(online);
    let broker = Broker::connect(
        Arc::new(FakeUpstream::new().answer_text("explore", &ctx)),
        config,
    )
    .await
    .unwrap();
    let mut took = vec![];
    for _ in 0..200 {
        let start = std::time::Instant::now();
        let out = broker
            .context_for_task(TaskRequest::new(TASK))
            .await
            .unwrap();
        took.push(start.elapsed());
        assert_eq!(
            out.provenance.online.as_ref().unwrap().discovery,
            "complete"
        );
    }
    took.sort();
    let (p50, p95) = (took[100], took[190]);
    println!(
        "16 files x ~16 KiB, {} requests per call: p50 {p50:?}, p95 {p95:?}",
        classifier.calls() / 200
    );
    assert!(p95 < std::time::Duration::from_millis(75), "p95 {p95:?}");
}
