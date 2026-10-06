//! The process-wide `no_jev_api_key` switch (D-155). It is global, so it lives in its own test
//! binary: no other test shares this process.
#![cfg(feature = "online")]

use ripwire_broker::online::SemanticStage;
use ripwire_broker::online::classifier::{Classifier, ClassifyError};
use ripwire_broker::online::credential::Credential;
use ripwire_broker::online::jev::JevClient;
use ripwire_broker::online::request::{StateItem, build};
use std::time::Duration;

#[tokio::test]
async fn with_the_switch_on_even_a_client_with_a_key_sends_nothing() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let key = Credential::from_env_value(Some("tok-123")).unwrap();
    let client = JevClient::loopback(port, key, "jev-1.13.0", Duration::from_secs(2)).unwrap();
    let req = build(
        "jev-1.13.0",
        "q",
        SemanticStage::FileAdmission,
        vec![StateItem {
            id: "i0".into(),
            path: "a.rs".into(),
            text: "fn a() {}".into(),
        }],
    );

    ripwire_broker::online::set_no_jev_api_key(true);
    assert!(ripwire_broker::online::no_jev_api_key());
    assert_eq!(
        client.classify(&req).await.unwrap_err(),
        ClassifyError::NoKey
    );
    let accepted = tokio::time::timeout(Duration::from_millis(200), listener.accept()).await;
    assert!(accepted.is_err(), "no connection was opened");
}
