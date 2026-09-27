//! Seam 3 (Phase 0 spike): the upstream client against a real `ripwire --mcp`.
mod common;

use ripwire_broker::upstream::{RipwireUpstream, Upstream, UpstreamConfig, UpstreamError};
use serde_json::json;

macro_rules! require_ripwire {
    () => {
        if !common::ripwire_available() {
            eprintln!("skipping: ripwire not on PATH");
            return;
        }
    };
}

#[tokio::test]
async fn explore_returns_a_task_bundle() {
    require_ripwire!();
    let repo = common::sample_repo();
    let up = RipwireUpstream::spawn(UpstreamConfig::new(repo.path()))
        .await
        .unwrap();

    let text = up
        .call("explore", json!({"task": "validate the login token"}))
        .await
        .unwrap();

    assert!(text.contains("<ctx"), "unexpected explore payload: {text}");
    assert!(text.contains("validate_token"));
}

#[tokio::test]
async fn diff_verbs_see_a_working_tree_edit() {
    require_ripwire!();
    let repo = common::sample_repo();
    common::write(
        repo.path(),
        "src/auth.py",
        "def validate_token(token, strict):\n    return token == \"ok\"\n\n\ndef login(user, token):\n    if not validate_token(token):\n        raise ValueError(\"bad token\")\n    return user\n",
    );
    let up = RipwireUpstream::spawn(UpstreamConfig::new(repo.path()))
        .await
        .unwrap();

    let sa: serde_json::Value =
        serde_json::from_str(&up.call("situational_awareness", json!({})).await.unwrap()).unwrap();
    assert_eq!(sa["changed_files"][0]["file"], "src/auth.py");

    let qd: serde_json::Value =
        serde_json::from_str(&up.call("quality_delta", json!({})).await.unwrap()).unwrap();
    assert!(
        qd.get("regressions").is_some(),
        "unexpected quality_delta payload: {qd}"
    );
}

#[tokio::test]
async fn a_bad_argument_is_a_refusal_not_an_outage() {
    require_ripwire!();
    let repo = common::sample_repo();
    let up = RipwireUpstream::spawn(UpstreamConfig::new(repo.path()))
        .await
        .unwrap();

    let err = up
        .call("find_symbol", json!({"symbol": "does_not_exist_anywhere"}))
        .await
        .unwrap_err();

    assert!(matches!(err, UpstreamError::Refused(_)), "got {err:?}");
}

#[tokio::test]
async fn a_hung_upstream_times_out_and_is_restarted() {
    require_ripwire!();
    let repo = common::sample_repo();
    let bin = tempfile::tempdir().unwrap();
    let mut config = UpstreamConfig::new(repo.path());
    config.binary = common::flaky_ripwire(bin.path(), "exec sleep 600");
    config.timeout = std::time::Duration::from_millis(500);
    let up = RipwireUpstream::spawn(config).await.unwrap();

    let first = up.call("quality_delta", json!({})).await;
    assert_eq!(first, Err(UpstreamError::Timeout));

    let second = up.call("quality_delta", json!({})).await;
    assert!(
        second.is_ok(),
        "restarted upstream should answer: {second:?}"
    );
    assert_eq!(up.restarts(), 1);
}

#[tokio::test]
async fn a_crashed_upstream_gets_one_restart_within_the_call() {
    require_ripwire!();
    let repo = common::sample_repo();
    let bin = tempfile::tempdir().unwrap();
    let mut config = UpstreamConfig::new(repo.path());
    config.binary = common::flaky_ripwire(bin.path(), "exit 3");
    config.timeout = std::time::Duration::from_secs(5);
    let up = RipwireUpstream::spawn(config).await.unwrap();

    let res = up.call("quality_delta", json!({})).await;

    assert!(res.is_ok(), "one restart should recover: {res:?}");
    assert_eq!(up.restarts(), 1);
}

#[tokio::test]
async fn a_missing_binary_is_reported_unavailable() {
    let repo = tempfile::tempdir().unwrap();
    let mut config = UpstreamConfig::new(repo.path());
    config.binary = "/nonexistent/ripwire".into();

    let res = match RipwireUpstream::spawn(config).await {
        Err(e) => Err(e),
        Ok(up) => up.call("quality_delta", json!({})).await,
    };

    assert!(
        matches!(res, Err(UpstreamError::Unavailable(_))),
        "got {res:?}"
    );
}

#[tokio::test]
async fn capabilities_are_discovered_from_a_pre_2026_server() {
    require_ripwire!();
    let repo = common::sample_repo();
    let up = RipwireUpstream::spawn(UpstreamConfig::new(repo.path()))
        .await
        .unwrap();

    let tools = up.list_tools().await.unwrap();

    for verb in [
        "explore",
        "situational_awareness",
        "quality_delta",
        "edit_check",
    ] {
        assert!(
            tools.iter().any(|t| t == verb),
            "missing {verb} in {tools:?}"
        );
    }
}
