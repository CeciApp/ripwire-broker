//! Seam 7, live: the real provider. Off by default so the suite stays offline (CA-10);
//! opt in with the credential in the environment:
//!
//! ```text
//! RIPWIRE_BROKER_JEV_API_KEY=... cargo test --features online --test online_live -- --ignored
//! ```
//!
//! Only invented content is sent (the S4.0b corpus and `common::sample_repo`), and only
//! digests, probabilities and timings are printed (PRD §23.14).
#![cfg(feature = "online")]

mod common;
#[path = "common/jev_corpus.rs"]
mod jev_corpus;

use ripwire_broker::broker::{Broker, BrokerConfig, TaskRequest};
use ripwire_broker::online::OnlineConfig;
use ripwire_broker::online::classifier::Classifier;
use ripwire_broker::online::credential::Credential;
use ripwire_broker::online::jev::JevClient;
use ripwire_broker::upstream::{RipwireUpstream, UpstreamConfig};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// These tests only run when asked for with `--ignored`; asked for without a key, they fail
/// instead of passing without checking anything.
fn live_client() -> JevClient {
    let key = Credential::from_env()
        .expect("the live tests need RIPWIRE_BROKER_JEV_API_KEY in the environment");
    JevClient::new(key, "jev-1.13.0", Duration::from_secs(15)).unwrap()
}

#[tokio::test]
#[ignore = "calls the real provider; set RIPWIRE_BROKER_JEV_API_KEY"]
async fn a_real_provider_classifies_the_synthetic_corpus() {
    let client = live_client();
    let [admission, selection] = <[_; 2]>::try_from(jev_corpus::requests()).unwrap();

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
    config.online = Some(OnlineConfig::new(Arc::new(client)));
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
