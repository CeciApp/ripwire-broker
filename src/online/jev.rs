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
    /// `None` (D-155): no usable key, so no call is sent.
    key: Option<Credential>,
    model: String,
    /// Response body bytes read, for `jev_response_bytes`.
    received: std::sync::Mutex<super::metrics::Sized>,
    /// Each request sent, for the status line's `[jev:N]` (D-154).
    activity: Option<std::sync::Arc<crate::server_status::Activities>>,
    /// `--log` (D-155).
    log: Option<std::sync::Arc<super::log::JevLog>>,
}

impl JevClient {
    pub fn new(key: Credential, model: &str, timeout: Duration) -> Result<Self, String> {
        Self::build(ENDPOINT.into(), true, false, Some(key), model, timeout)
    }

    /// No usable key (D-155): every call fails with [`ClassifyError::NoKey`] before anything is
    /// sent.
    pub fn without_key(model: &str, timeout: Duration) -> Result<Self, String> {
        Self::build(ENDPOINT.into(), true, false, None, model, timeout)
    }

    /// Test fixtures only: [`Self::loopback`] without a key.
    #[doc(hidden)]
    pub fn loopback_without_key(port: u16, model: &str, timeout: Duration) -> Result<Self, String> {
        let endpoint = format!("http://127.0.0.1:{port}/v1/systemone");
        Self::build(endpoint, false, false, None, model, timeout)
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
        Self::build(endpoint, false, false, Some(key), model, timeout)
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
        Self::build(endpoint, false, true, Some(key), model, timeout)
    }

    fn build(
        endpoint: String,
        https_only: bool,
        h2c: bool,
        key: Option<Credential>,
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
            activity: None,
            log: None,
        })
    }

    /// Writes every exchange to `log` (`--log`).
    pub fn with_log(mut self, log: std::sync::Arc<super::log::JevLog>) -> Self {
        self.log = Some(log);
        self
    }

    /// Records every request this client sends, whatever its answer, in `activity`.
    pub fn with_activity(
        mut self,
        activity: std::sync::Arc<crate::server_status::Activities>,
    ) -> Self {
        self.activity = Some(activity);
        self
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// A key the header cannot carry fails like a refused key, without an attempt. No key, or
    /// the process-wide `no_jev_api_key` on, fails with `NoKey` (D-155).
    fn bearer(&self) -> Result<HeaderValue, ClassifyError> {
        let key = match &self.key {
            Some(k) if !super::no_jev_api_key() => k,
            _ => return Err(ClassifyError::NoKey),
        };
        let mut v = HeaderValue::from_str(&format!("Bearer {}", key.expose()))
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
        let text = self.post(body, true).await?;
        let ids: Vec<String> = req.questions.0.iter().map(|(id, _)| id.clone()).collect();
        parse_answers(&self.model, &ids, &text).map_err(ClassifyError::Invalid)
    }
}

#[async_trait]
impl super::classifier::MemoryClassifier for JevClient {
    async fn decide(
        &self,
        req: &super::request::StateRequest,
    ) -> Result<Vec<super::response::Decision>, ClassifyError> {
        let body = serde_json::to_vec(req)
            .map_err(|_| ClassifyError::Invalid(InvalidResponse::Malformed))?;
        let text = self.post(body, false).await?;
        super::response::parse_decisions(req, &text).map_err(ClassifyError::Invalid)
    }
}

impl JevClient {
    /// One POST to the fixed endpoint, shared by discovery and memory: Bearer, no redirect or
    /// proxy, the status mapped to a category, and the body capped at [`MAX_RESPONSE_BYTES`].
    /// `discovery`: the response counts in `jev_response_bytes`; memory has its own metrics.
    /// Without a key nothing is sent, counted or logged (D-155). A refused key marks the key
    /// invalid for the status line, and the next success clears it.
    async fn post(&self, body: Vec<u8>, discovery: bool) -> Result<String, ClassifyError> {
        let bearer = self.bearer()?;
        if let Some(a) = &self.activity {
            a.jev.record(crate::hook::now());
        }
        let started = std::time::Instant::now();
        let sent = self.log.as_ref().map(|_| body.clone());
        let (result, status, received) = self.exchange(bearer, body, discovery).await;
        if let Some(a) = &self.activity {
            match &result {
                Ok(_) => a.set_key(crate::server_status::KeyState::Ok),
                Err(ClassifyError::Auth(_)) => a.set_key(crate::server_status::KeyState::Invalid),
                Err(_) => {}
            }
        }
        if let (Some(log), Some(sent)) = (&self.log, sent) {
            let error = result.as_ref().err().map(ClassifyError::category);
            let exchange = super::log::Exchange {
                endpoint: &self.endpoint,
                purpose: if discovery { "discovery" } else { "memory" },
                sent: &sent,
                status,
                received: &received,
                error,
                elapsed: started.elapsed(),
            };
            log.record(&exchange, self.key.as_ref().map(Credential::expose));
        }
        result
    }

    /// The request and its answer. Also returns the status and the body read, which only the
    /// log uses: an error's body is read only when there is a log.
    async fn exchange(
        &self,
        bearer: HeaderValue,
        body: Vec<u8>,
        discovery: bool,
    ) -> (Result<String, ClassifyError>, Option<u16>, Vec<u8>) {
        let sent = self
            .http
            .post(&self.endpoint)
            .header(AUTHORIZATION, bearer)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await;
        let mut resp = match sent {
            Ok(r) => r,
            Err(e) => return (Err(transport(e)), None, Vec::new()),
        };
        let status = resp.status().as_u16();
        let error = match status {
            200 => None,
            401 | 403 => Some(ClassifyError::Auth(status)),
            408 => Some(ClassifyError::Timeout),
            429 => {
                let retry_after = resp
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| {
                        let key = self.key.as_ref().map(Credential::expose);
                        super::redact::remote_text(v, key, 64)
                    });
                Some(ClassifyError::RateLimited { retry_after })
            }
            500..=599 => Some(ClassifyError::Server(status)),
            _ => Some(ClassifyError::Rejected(status)),
        };
        if let Some(e) = error {
            let body = match self.log.is_some() {
                true => read_capped(&mut resp).await.unwrap_or_default(),
                false => Vec::new(),
            };
            return (Err(e), Some(status), body);
        }
        let json = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("application/json"));
        if !json {
            return (
                Err(ClassifyError::Invalid(InvalidResponse::Malformed)),
                Some(status),
                Vec::new(),
            );
        }
        let bytes = match read_capped(&mut resp).await {
            Ok(b) => b,
            Err(e) => return (Err(e), Some(status), Vec::new()),
        };
        if discovery {
            self.received.lock().unwrap().add(bytes.len() as u64);
        }
        let logged = match self.log.is_some() {
            true => bytes.clone(),
            false => Vec::new(),
        };
        let text = String::from_utf8(bytes)
            .map_err(|_| ClassifyError::Invalid(InvalidResponse::Malformed));
        (text, Some(status), logged)
    }
}

/// The body, refused past [`MAX_RESPONSE_BYTES`].
async fn read_capped(resp: &mut reqwest::Response) -> Result<Vec<u8>, ClassifyError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(transport)? {
        if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(ClassifyError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
