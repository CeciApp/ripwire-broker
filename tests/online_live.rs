//! Seam 7, live: the real provider. Off by default so the suite stays offline (CA-10);
//! opt in with the credential in the environment:
//!
//! ```text
//! RIPWIRE_BROKER_JEV_API_KEY=... cargo test --features online --test online_live -- --ignored
//! ```
//!
//! The same tests run against Cloudflare's Clef (D-166), with a Workers AI token in the same
//! variable. These three are read by the tests only; the binary has no such variables:
//!
//! ```text
//! RIPWIRE_BROKER_LIVE_PROVIDER=cloudflare RIPWIRE_BROKER_LIVE_ACCOUNT_ID=... \
//!   [RIPWIRE_BROKER_LIVE_MODEL=clef] RIPWIRE_BROKER_JEV_API_KEY=... \
//!   cargo test --features online --test online_live -- --ignored
//! ```
//!
//! Only invented content is sent (the S4.0b corpus and `common::sample_repo`), and only
//! digests, probabilities and timings are printed (PRD §23.14).
#![cfg(feature = "online")]

mod common;
#[path = "common/jev_corpus.rs"]
mod jev_corpus;

use ripwire_broker::broker::{Broker, BrokerConfig, TaskRequest};
use ripwire_broker::online::classifier::Classifier;
use ripwire_broker::online::credential::Credential;
use ripwire_broker::online::jev::JevClient;
use ripwire_broker::online::{JevProvider, OnlineConfig};
use ripwire_broker::upstream::{RipwireUpstream, UpstreamConfig};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// These tests only run when asked for with `--ignored`; asked for without a key, they fail
/// instead of passing without checking anything.
fn live_client() -> JevClient {
    let key = Credential::from_env()
        .expect("the live tests need RIPWIRE_BROKER_JEV_API_KEY in the environment");
    let (provider, account, model) = live_target();
    let timeout = Duration::from_secs(15);
    JevClient::for_provider(provider, account.as_deref(), Some(key), &model, timeout).unwrap()
}

/// Who the live tests ask: TypeSafe's pinned model unless `RIPWIRE_BROKER_LIVE_PROVIDER` says
/// otherwise.
fn live_target() -> (JevProvider, Option<String>, String) {
    let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let provider = match var("RIPWIRE_BROKER_LIVE_PROVIDER") {
        None => JevProvider::TypeSafe,
        Some(name) => JevProvider::parse(&name).expect("typesafe or cloudflare"),
    };
    let model = var("RIPWIRE_BROKER_LIVE_MODEL").unwrap_or(provider.default_model().into());
    (provider, var("RIPWIRE_BROKER_LIVE_ACCOUNT_ID"), model)
}

#[tokio::test]
#[ignore = "calls the real provider; set RIPWIRE_BROKER_JEV_API_KEY"]
async fn a_real_provider_classifies_the_synthetic_corpus() {
    let client = live_client();
    let [mut admission, mut selection] = <[_; 2]>::try_from(jev_corpus::requests()).unwrap();
    for req in [&mut admission, &mut selection] {
        req.model = live_target().2;
    }

    for req in [&admission, &selection] {
        let started = Instant::now();
        let answers = client.classify(req).await.unwrap();
        let digest = format!("{:x}", Sha256::digest(serde_json::to_string(req).unwrap()));
        eprintln!(
            "sha256:{digest} {:?} in {} ms",
            answers,
            started.elapsed().as_millis()
        );
        assert_eq!(answers.len(), req.questions.0.len());
        assert!(
            answers
                .iter()
                .all(|p| p.is_some_and(|p| (0.0..=1.0).contains(&p)))
        );
    }
    let a = client.classify(&admission).await.unwrap();
    assert!(a[0].unwrap() > 0.25, "auth.py is admitted");
    assert!(a[1].unwrap() > 0.25, "its test is admitted");
    assert!(a[2].unwrap() <= 0.25, "an unrelated CSV writer is not");
    let s = client.classify(&selection).await.unwrap();
    assert!(s[0].unwrap() > 0.5, "validate_token is evidence");
    assert!(client.response_bytes().total > 0);
}

#[tokio::test]
#[ignore = "calls the real provider and ripwire; set RIPWIRE_BROKER_JEV_API_KEY"]
async fn a_real_provider_enriches_a_synthetic_repository() {
    assert!(
        common::ripwire_available(),
        "the live tests need ripwire on PATH"
    );
    let client = live_client();
    let repo = common::sample_repo();
    let upstream = RipwireUpstream::spawn(UpstreamConfig::new(repo.path()))
        .await
        .unwrap();
    let mut config = BrokerConfig::new(repo.path());
    config.online = Some(OnlineConfig::new(Arc::new(client)).with_provider(live_target().0));
    let broker = Broker::connect(Arc::new(upstream), config).await.unwrap();

    let started = Instant::now();
    let env = broker
        .context_for_task(TaskRequest::new("how are login tokens validated?"))
        .await
        .unwrap();

    let online = env.provenance.online.clone().unwrap();
    eprintln!(
        "discovery {} with {} requests in {} ms",
        online.discovery,
        online.requests,
        started.elapsed().as_millis()
    );
    assert_eq!(online.discovery, "complete");
    assert!(online.requests > 0);
    assert!(
        env.items.iter().any(|i| i.semantic.is_some()),
        "some item carries semantic evidence"
    );
    assert!(env.budget.estimated_tokens <= env.budget.requested_tokens);
}

/// jev-mem T2.0 (V14): the pinned model answers a Choice in the shape PRD jev-mem §7 expects.
/// Until this has run once and its result is in the changelog, implicit time (T2.7) and the
/// representation decision (Phase 4) rest on the documented contract only.
#[tokio::test]
#[ignore = "calls the real provider; set RIPWIRE_BROKER_JEV_API_KEY"]
async fn the_pinned_model_answers_a_choice() {
    use ripwire_broker::online::classifier::MemoryClassifier;
    use ripwire_broker::online::request::{JevQuestion, StateRequest};
    use ripwire_broker::online::response::Decision;

    let req = StateRequest::new(
        &live_target().2,
        serde_json::json!({
            "new_memory": {"id": "m2", "content": "Mira bought a new bicycle on 16 May."},
            "candidates": [{"id": "m1", "content": "Mira's old bicycle broke on 14 May."}]
        }),
        vec![JevQuestion::choice(
            "Which temporal relation holds from new_memory.content to candidates[0].content? \
             Judge only from the supplied accounts.",
            &[
                ("before", "new_memory happened before candidates[0]."),
                ("after", "new_memory happened after candidates[0]."),
                ("unknown", "The accounts do not support any relation."),
            ],
        )],
    );
    let started = Instant::now();
    let got = live_client().decide(&req).await.expect("a valid response");
    let Decision::Choice {
        selected,
        probabilities,
        ..
    } = &got[0]
    else {
        panic!("not a valid Choice: {:?}", got[0]);
    };
    eprintln!(
        "choice: {selected}, probabilities {probabilities:?}, {} ms",
        started.elapsed().as_millis()
    );
    assert_eq!(selected, "after", "invented content with an explicit order");
}
