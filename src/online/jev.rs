//! `JevClient` (PRD §23.2, §23.9): the TypeSafe `systemone` protocol over one shared
//! `reqwest::Client` (rustls, pooling, HTTP/2 when negotiated). HTTPS to the allowlisted
//! endpoint only, no redirects, no proxy, no retries of its own.

use super::classifier::{Classifier, ClassifyError};
use super::credential::Credential;
use super::request::JevRequest;
use super::response::{InvalidResponse, parse_answers};
use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue, RETRY_AFTER};
use std::time::Duration;

/// The only endpoint of the first increment (v0.1 §13.4): no URL comes from configuration.
pub const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
/// Responses carry a few probabilities; anything bigger is refused unread.
pub const MAX_RESPONSE_BYTES: usize = 256 * 1024;

pub struct JevClient {
    http: reqwest::Client,
    endpoint: String,
    key: Credential,
    model: String,
    /// Response body bytes read, for `jev_response_bytes`.
    received: std::sync::Mutex<super::metrics::Sized>,
}

impl JevClient {
    pub fn new(key: Credential, model: &str, timeout: Duration) -> Result<Self, String> {
        Self::build(ENDPOINT.into(), true, false, key, model, timeout)
    }

    /// Test fixtures only: plain HTTP to `127.0.0.1`. No command line option reaches it.
    #[doc(hidden)]
    pub fn loopback(
        port: u16,
        key: Credential,
        model: &str,
        timeout: Duration,
    ) -> Result<Self, String> {
        let endpoint = format!("http://127.0.0.1:{port}/v1/systemone");
        Self::build(endpoint, false, false, key, model, timeout)
    }

    /// Test fixtures only: like [`Self::loopback`], but HTTP/2 from the first byte (h2c),
    /// since a plain-HTTP fixture has no ALPN to negotiate it.
    #[doc(hidden)]
    pub fn loopback_h2(
        port: u16,
        key: Credential,
        model: &str,
        timeout: Duration,
    ) -> Result<Self, String> {
        let endpoint = format!("http://127.0.0.1:{port}/v1/systemone");
        Self::build(endpoint, false, true, key, model, timeout)
    }

    fn build(
        endpoint: String,
        https_only: bool,
        h2c: bool,
        key: Credential,
        model: &str,
        timeout: Duration,
    ) -> Result<Self, String> {
        let mut http = reqwest::Client::builder();
        if h2c {
            http = http.http2_prior_knowledge();
        }
        let http = http
            .https_only(https_only)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(timeout)
            .connect_timeout(timeout.min(Duration::from_secs(5)))
            .user_agent(concat!("ripwire-broker/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| format!("HTTP client: {e}"))?;
        Ok(Self {
            http,
            endpoint,
            key,
            model: model.into(),
            received: std::sync::Mutex::default(),
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// A key the header cannot carry fails like a refused key, without an attempt.
    fn bearer(&self) -> Result<HeaderValue, ClassifyError> {
        let mut v = HeaderValue::from_str(&format!("Bearer {}", self.key.expose()))
            .map_err(|_| ClassifyError::Auth(0))?;
        v.set_sensitive(true);
        Ok(v)
    }
}

fn transport(e: reqwest::Error) -> ClassifyError {
    match e.is_timeout() {
        true => ClassifyError::Timeout,
        false => ClassifyError::Network,
    }
}

#[async_trait]
impl Classifier for JevClient {
    fn model(&self) -> &str {
        &self.model
    }

    fn response_bytes(&self) -> super::metrics::Sized {
        self.received.lock().unwrap().clone()
    }

    async fn classify(&self, req: &JevRequest) -> Result<Vec<Option<f64>>, ClassifyError> {
        let body = serde_json::to_vec(req)
            .map_err(|_| ClassifyError::Invalid(InvalidResponse::Malformed))?;
        let mut resp = self
            .http
            .post(&self.endpoint)
            .header(AUTHORIZATION, self.bearer()?)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(transport)?;
        let status = resp.status().as_u16();
        match status {
            200 => {}
            401 | 403 => return Err(ClassifyError::Auth(status)),
            408 => return Err(ClassifyError::Timeout),
            429 => {
                let retry_after = resp
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| super::redact::remote_text(v, Some(self.key.expose()), 64));
                return Err(ClassifyError::RateLimited { retry_after });
            }
            500..=599 => return Err(ClassifyError::Server(status)),
            _ => return Err(ClassifyError::Rejected(status)),
        }
        let json = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("application/json"));
        if !json {
            return Err(ClassifyError::Invalid(InvalidResponse::Malformed));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(transport)? {
            if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(ClassifyError::TooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        self.received.lock().unwrap().add(bytes.len() as u64);
        let text = String::from_utf8(bytes)
            .map_err(|_| ClassifyError::Invalid(InvalidResponse::Malformed))?;
        let ids: Vec<String> = req.questions.0.iter().map(|(id, _)| id.clone()).collect();
        parse_answers(&self.model, &ids, &text).map_err(ClassifyError::Invalid)
    }
}
