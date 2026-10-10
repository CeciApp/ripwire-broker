//! Seam 4: the real `JevClient` against a local HTTP/1.1 fixture server (PRD §23.14). Needs
//! `--features online`; the fixture only listens on 127.0.0.1.
//!
//! Production speaks HTTPS and may negotiate HTTP/2, where a cancel resets a stream instead of
//! closing a TCP connection. The cancel is also checked against an HTTP/2 fixture (h2c: HTTP/2
//! without TLS, so without ALPN); the live tests (tests/online_live.rs) exercise TLS and ALPN.
#![cfg(feature = "online")]

use ripwire_broker::online::SemanticStage;
use ripwire_broker::online::classifier::{Classifier, ClassifyError};
use ripwire_broker::online::credential::Credential;
use ripwire_broker::online::jev::{JevClient, MAX_RESPONSE_BYTES};
use ripwire_broker::online::request::{JevRequest, StateItem, build};
use ripwire_broker::online::response::InvalidResponse;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Clone)]
struct Captured {
    head: String,
    body: Vec<u8>,
}

impl Captured {
    fn header(&self, name: &str) -> Option<String> {
        self.head.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case(name).then(|| v.trim().to_string())
        })
    }
}

/// What the fixture answers: status, extra headers, body. `None` accepts and never answers.
type Reply = Option<(u16, Vec<(&'static str, String)>, Vec<u8>)>;

struct Fixture {
    port: u16,
    seen: Arc<Mutex<Vec<Captured>>>,
}

async fn fixture(replies: Vec<Reply>) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(vec![]));
    let log = seen.clone();
    tokio::spawn(async move {
        for reply in replies {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let mut buf = vec![];
            let mut chunk = [0u8; 4096];
            let head_end = loop {
                let n = sock.read(&mut chunk).await.unwrap_or(0);
                if n == 0 {
                    break None;
                }
                buf.extend_from_slice(&chunk[..n]);
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(i + 4);
                }
            };
            let Some(head_end) = head_end else { continue };
            let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
            let length: usize = head
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse().unwrap())
                })
                .unwrap_or(0);
            while buf.len() < head_end + length {
                let n = sock.read(&mut chunk).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            log.lock().unwrap().push(Captured {
                head,
                body: buf[head_end..].to_vec(),
            });
            let Some((status, headers, body)) = reply else {
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            };
            let mut out = format!(
                "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n",
                body.len()
            );
            for (k, v) in headers {
                out.push_str(&format!("{k}: {v}\r\n"));
            }
            out.push_str("\r\n");
            let _ = sock.write_all(out.as_bytes()).await;
            let _ = sock.write_all(&body).await;
            let _ = sock.shutdown().await;
        }
    });
    Fixture { port, seen }
}

fn json(status: u16, body: &str) -> Reply {
    Some((
        status,
        vec![("content-type", "application/json".into())],
        body.as_bytes().to_vec(),
    ))
}

fn client(port: u16, timeout: Duration) -> JevClient {
    let key = Credential::from_env_value(Some("tok-123")).unwrap();
    JevClient::loopback(port, key, "jev-1.13.0", timeout).unwrap()
}

fn request() -> JevRequest {
    build(
        "jev-1.13.0",
        "how are login tokens validated?",
        SemanticStage::FileAdmission,
        vec![
            StateItem {
                id: "i0".into(),
                path: "src/auth.py".into(),
                text: "def validate_token(t): ...".into(),
            },
            StateItem {
                id: "i1".into(),
                path: "src/csv.py".into(),
                text: "def write(rows): ...".into(),
            },
        ],
    )
}

const OK: &str = r#"{"model":"jev-1.13.0","answers":{"q0":{"type":"noul","noul":0.81},"q1":{"type":"noul","noul":0.03}},"usage":{"input_tokens":10,"output_tokens":2}}"#;

#[tokio::test]
async fn the_client_posts_the_exact_body_with_a_bearer_to_systemone() {
    let f = fixture(vec![json(200, OK)]).await;
    let req = request();

    let answers = client(f.port, Duration::from_secs(5))
        .classify(&req)
        .await
        .unwrap();

    assert_eq!(answers, vec![Some(0.81), Some(0.03)]);
    let seen = f.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    let got = &seen[0];
    assert!(
        got.head.starts_with("POST /v1/systemone HTTP/1.1\r\n"),
        "{}",
        got.head
    );
    assert_eq!(
        got.header("authorization").as_deref(),
        Some("Bearer tok-123")
    );
    assert_eq!(
        got.header("content-type").as_deref(),
        Some("application/json")
    );
    assert_eq!(
        got.body,
        serde_json::to_string(&req).unwrap().into_bytes(),
        "body sent verbatim"
    );
}

#[tokio::test]
async fn http_failures_are_classified() {
    let secret_remote = r#"{"error":"remote says tok-123 is bad"}"#;
    let cases: Vec<(Reply, ClassifyError)> = vec![
        (json(401, secret_remote), ClassifyError::Auth(401)),
        (json(403, secret_remote), ClassifyError::Auth(403)),
        (json(408, secret_remote), ClassifyError::Timeout),
        (json(409, secret_remote), ClassifyError::Rejected(409)),
        (
            Some((429, vec![("retry-after", "7".into())], vec![])),
            ClassifyError::RateLimited {
                retry_after: Some("7".into()),
            },
        ),
        (json(500, secret_remote), ClassifyError::Server(500)),
        (json(503, secret_remote), ClassifyError::Server(503)),
        (
            json(200, "{not json"),
            ClassifyError::Invalid(InvalidResponse::Malformed),
        ),
        (
            Some((
                200,
                vec![("content-type", "text/html".into())],
                OK.as_bytes().to_vec(),
            )),
            ClassifyError::Invalid(InvalidResponse::Malformed),
        ),
        (
            Some((
                200,
                vec![("content-type", "application/json".into())],
                vec![b' '; MAX_RESPONSE_BYTES + 1],
            )),
            ClassifyError::TooLarge,
        ),
    ];
    for (reply, expected) in cases {
        let f = fixture(vec![reply]).await;

        let err = client(f.port, Duration::from_secs(5))
            .classify(&request())
            .await
            .unwrap_err();

        assert_eq!(err, expected);
        let shown = format!("{err} {err:?}");
        assert!(
            !shown.contains("tok-123") && !shown.contains("remote says"),
            "{shown}"
        );
    }
}

#[tokio::test]
async fn redirects_are_refused() {
    let elsewhere = fixture(vec![json(200, OK)]).await;
    let f = fixture(vec![Some((
        302,
        vec![(
            "location",
            format!("http://127.0.0.1:{}/v1/systemone", elsewhere.port),
        )],
        vec![],
    ))])
    .await;

    let err = client(f.port, Duration::from_secs(5))
        .classify(&request())
        .await
        .unwrap_err();

    assert_eq!(err, ClassifyError::Rejected(302));
    assert!(
        elsewhere.seen.lock().unwrap().is_empty(),
        "the redirect target is never contacted"
    );
}

#[tokio::test]
async fn the_client_never_retries_on_its_own() {
    let f = fixture(vec![json(503, "{}"), json(200, OK)]).await;
    let err = client(f.port, Duration::from_secs(5))
        .classify(&request())
        .await
        .unwrap_err();
    assert_eq!(err, ClassifyError::Server(503));
    assert_eq!(
        f.seen.lock().unwrap().len(),
        1,
        "one attempt; retries belong to the scheduler"
    );

    let hung = fixture(vec![None, json(200, OK)]).await;
    let err = client(hung.port, Duration::from_millis(300))
        .classify(&request())
        .await
        .unwrap_err();
    assert_eq!(err, ClassifyError::Timeout);
    assert_eq!(hung.seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_closed_connection_is_a_network_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (sock, _) = listener.accept().await.unwrap();
        drop(sock);
    });

    let err = client(port, Duration::from_secs(5))
        .classify(&request())
        .await
        .unwrap_err();

    assert_eq!(err, ClassifyError::Network);
    assert!(err.is_transient());
}

// --- S4.31: the broker end to end through the real client ---

mod common;

/// A fixture that answers every question of every request with `p`, for `n` connections.
async fn answering(n: usize, p: f64) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(vec![]));
    let log = seen.clone();
    tokio::spawn(async move {
        for _ in 0..n {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let mut buf = vec![];
            let mut chunk = [0u8; 8192];
            let (head_end, length) = loop {
                let got = sock.read(&mut chunk).await.unwrap_or(0);
                if got == 0 {
                    return;
                }
                buf.extend_from_slice(&chunk[..got]);
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| {
                            l.strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    break (i + 4, len);
                }
            };
            while buf.len() < head_end + length {
                let got = sock.read(&mut chunk).await.unwrap_or(0);
                if got == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..got]);
            }
            let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
            let body = buf[head_end..].to_vec();
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let answers: serde_json::Map<String, serde_json::Value> = req["questions"]
                .as_object()
                .unwrap()
                .keys()
                .map(|k| (k.clone(), serde_json::json!({"type": "noul", "noul": p})))
                .collect();
            log.lock().unwrap().push(Captured { head, body });
            let out = serde_json::json!({"model": req["model"], "answers": answers, "usage": {"input_tokens": 1, "output_tokens": 1}})
                .to_string();
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{out}",
                out.len()
            );
            let _ = sock.write_all(reply.as_bytes()).await;
            let _ = sock.shutdown().await;
        }
    });
    Fixture { port, seen }
}

#[tokio::test]
async fn the_broker_enriches_a_task_through_the_real_client() {
    use ripwire_broker::broker::{Broker, BrokerConfig, TaskRequest};
    use ripwire_broker::online::OnlineConfig;

    let ws = tempfile::tempdir().unwrap();
    common::write(
        ws.path(),
        "src/auth.py",
        "def validate_token(token):\n    return token == \"ok\"\n\n\ndef login(user, token):\n    return user\n\n\ndef export_report(user, token):\n    return \"report\"\n",
    );
    common::write(
        ws.path(),
        "src/routes.py",
        "def export_route(req):\n    pass\n",
    );
    common::write(
        ws.path(),
        "tests/test_auth.py",
        "def test_login():\n    pass\n",
    );
    let f = answering(20, 0.9).await;
    let mut config = BrokerConfig::new(ws.path());
    config.online = Some(OnlineConfig::new(Arc::new(client(
        f.port,
        Duration::from_secs(5),
    ))));
    let upstream =
        Arc::new(common::fake::FakeUpstream::new().answer("explore", "explore_export_auth"));
    let broker = Broker::connect(upstream, config).await.unwrap();

    let out = serde_json::to_value(
        broker
            .context_for_task(TaskRequest::new("how are the routes authenticated?"))
            .await
            .unwrap(),
    )
    .unwrap();

    let online = &out["provenance"]["online"];
    assert_eq!(online["discovery"], "complete", "{out:#}");
    let sent = f.seen.lock().unwrap().clone();
    assert_eq!(online["requests"].as_u64().unwrap() as usize, sent.len());
    assert!(sent.len() >= 2, "admission, then selection");
    assert!(
        sent.iter()
            .all(|c| c.header("authorization").as_deref() == Some("Bearer tok-123"))
    );
    let body = String::from_utf8(sent[0].body.clone()).unwrap();
    assert!(
        !body.contains(ws.path().to_str().unwrap()),
        "never the absolute root (PRD §23.9)"
    );
    let login = out["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["symbol"] == "login")
        .unwrap();
    assert_eq!(login["semantic"]["state"], "selected_source");
    assert_eq!(login["semantic"]["probability"], 0.9);
    let status = serde_json::to_value(broker.status().await).unwrap();
    let wire: usize = sent.iter().map(|c| c.body.len()).sum();
    assert_eq!(
        status["online"]["metrics"]["jev_request_bytes"]["total"], wire,
        "request bytes match what the provider received"
    );
    let received = &status["online"]["metrics"]["jev_response_bytes"]["total"];
    assert!(
        received.as_u64().unwrap() > 0,
        "the real client counts response bytes: {status}"
    );
}

// --- S5.4: an MCP cancel reaches the HTTP request (CA-ONLINE-12) ---

/// A provider that reads each request and never answers; `closed` counts connections the
/// client dropped while waiting.
async fn silent() -> (u16, Arc<Mutex<usize>>, Arc<std::sync::atomic::AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(0));
    let closed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (log, gone) = (seen.clone(), closed.clone());
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let (log, gone) = (log.clone(), gone.clone());
            tokio::spawn(async move {
                let mut chunk = [0u8; 8192];
                let mut first = true;
                // Reads the request, then waits for the client to hang up.
                loop {
                    match sock.read(&mut chunk).await {
                        Ok(0) | Err(_) => {
                            gone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            return;
                        }
                        Ok(_) if first => {
                            first = false;
                            *log.lock().unwrap() += 1;
                        }
                        Ok(_) => {}
                    }
                }
            });
        }
    });
    (port, seen, closed)
}

/// The same provider over HTTP/2: it accepts each stream and never answers; `reset` counts
/// streams the client reset with `CANCEL`. A client that spoke HTTP/1.1 would fail the
/// handshake and never reach `seen`.
async fn silent_h2() -> (u16, Arc<Mutex<usize>>, Arc<std::sync::atomic::AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(0));
    let reset = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (log, cancelled) = (seen.clone(), reset.clone());
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = listener.accept().await else {
                return;
            };
            let (log, cancelled) = (log.clone(), cancelled.clone());
            tokio::spawn(async move {
                let Ok(mut conn) = h2::server::handshake(sock).await else {
                    return;
                };
                // `accept` also drives the connection, so resets are read while it waits.
                while let Some(Ok((_request, mut respond))) = conn.accept().await {
                    *log.lock().unwrap() += 1;
                    let cancelled = cancelled.clone();
                    tokio::spawn(async move {
                        let why = std::future::poll_fn(|cx| respond.poll_reset(cx)).await;
                        if matches!(why, Ok(h2::Reason::CANCEL)) {
                            cancelled.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                    });
                }
            });
        }
    });
    (port, seen, reset)
}

/// Starts a task whose online stage asks `client`, waits for the first request to reach the
/// provider, then aborts the call the way the MCP handler does on `notifications/cancelled`
/// (RF-14). Returns how many requests had reached the provider.
async fn cancel_a_task_in_flight(client: JevClient, seen: &Mutex<usize>) -> usize {
    use ripwire_broker::broker::{Broker, BrokerConfig, TaskRequest};
    use ripwire_broker::online::OnlineConfig;

    let ws = tempfile::tempdir().unwrap();
    for (path, body) in [
        ("src/auth.py", "def login(user, token):\n    return user\n"),
        ("src/routes.py", "def export_route(req):\n    pass\n"),
        ("tests/test_auth.py", "def test_login():\n    pass\n"),
    ] {
        common::write(ws.path(), path, body);
    }
    let mut config = BrokerConfig::new(ws.path());
    config.online = Some(OnlineConfig::new(Arc::new(client)));
    let upstream =
        Arc::new(common::fake::FakeUpstream::new().answer("explore", "explore_export_auth"));
    let broker = Arc::new(Broker::connect(upstream, config).await.unwrap());

    let call = tokio::spawn({
        let b = broker.clone();
        async move {
            b.context_for_task(TaskRequest::new("how are the routes authenticated?"))
                .await
        }
    });
    let until = tokio::time::Instant::now() + Duration::from_secs(5);
    while *seen.lock().unwrap() == 0 && tokio::time::Instant::now() < until {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(*seen.lock().unwrap() > 0, "a request reached the provider");

    call.abort();
    let _ = call.await;
    *seen.lock().unwrap()
}

/// Waits until `count` reaches `want`, for at most two seconds. Generous on purpose: the
/// 250 ms bound of v0.1 §20.1 is proven with controlled time in tests/online_scheduler.rs;
/// here the point is that the abort reaches the wire at all.
async fn settle(count: &std::sync::atomic::AtomicUsize, want: usize) {
    let until = tokio::time::Instant::now() + Duration::from_secs(2);
    while count.load(std::sync::atomic::Ordering::SeqCst) < want
        && tokio::time::Instant::now() < until
    {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
async fn an_mcp_cancel_aborts_http_requests_in_flight() {
    use std::sync::atomic::Ordering::SeqCst;

    let (port, seen, closed) = silent().await;
    let sent = cancel_a_task_in_flight(client(port, Duration::from_secs(30)), &seen).await;
    settle(&closed, sent).await;

    assert_eq!(
        closed.load(SeqCst),
        sent,
        "every HTTP request in flight was aborted"
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        *seen.lock().unwrap(),
        sent,
        "no retry or new request after the cancel"
    );
}

#[tokio::test]
async fn an_mcp_cancel_resets_http2_streams_in_flight() {
    use std::sync::atomic::Ordering::SeqCst;

    let (port, seen, reset) = silent_h2().await;
    let key = Credential::from_env_value(Some("tok-123")).unwrap();
    let h2 = JevClient::loopback_h2(port, key, "jev-1.13.0", Duration::from_secs(30)).unwrap();
    let sent = cancel_a_task_in_flight(h2, &seen).await;
    settle(&reset, sent).await;

    // Over HTTP/2 the connection is shared and stays open: closing it is not the signal, a
    // RST_STREAM per request is.
    assert_eq!(
        reset.load(SeqCst),
        sent,
        "every HTTP/2 stream in flight was reset with CANCEL"
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        *seen.lock().unwrap(),
        sent,
        "no retry or new request after the cancel"
    );
}

// --- S5.6: no error carries the credential or raw remote text ---

#[tokio::test]
async fn remote_messages_are_sanitized_capped_and_redact_the_credential() {
    let echo = r#"{"error":"invalid token tok-123","detail":"\u001b[31mtok-123"}"#;
    let replies: Vec<Reply> = vec![
        Some((
            429,
            vec![("retry-after", "12 tok-123".into())],
            echo.as_bytes().to_vec(),
        )),
        Some((
            429,
            vec![("retry-after", format!("5 {}", "x".repeat(300)))],
            vec![],
        )),
        json(401, echo),
        json(403, echo),
        json(500, echo),
        json(409, echo),
        json(200, echo),
        Some((
            200,
            vec![("content-type", "text/plain; tok-123".into())],
            echo.as_bytes().to_vec(),
        )),
    ];
    for reply in replies {
        let f = fixture(vec![reply]).await;

        let err = client(f.port, Duration::from_secs(5))
            .classify(&request())
            .await
            .unwrap_err();

        let shown = format!("{err} {err:?} {}", err.category());
        assert!(!shown.contains("tok-123"), "{shown}");
        assert!(
            !shown.contains("invalid token"),
            "no remote message: {shown}"
        );
        assert!(
            shown.chars().all(|c| c == ' ' || c.is_ascii_graphic()),
            "{shown:?}"
        );
        if let ripwire_broker::online::classifier::ClassifyError::RateLimited {
            retry_after: Some(v),
        } = &err
        {
            assert!(v.len() <= 64, "{v}");
        }
    }
}

// --- S5.12: the probe over the wire ---

#[tokio::test]
async fn the_jev_probe_goes_over_the_wire_once() {
    let f = answering(5, 0.97).await;

    let check = ripwire_broker::doctor::jev_probe(&client(f.port, Duration::from_secs(5))).await;

    assert_eq!(
        check.status,
        ripwire_broker::doctor::Outcome::Ok,
        "{}",
        check.detail
    );
    let sent = f.seen.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    let body = String::from_utf8(sent[0].body.clone()).unwrap();
    assert!(body.contains(ripwire_broker::doctor::PROBE_PATH));
}

// --- review #6: pooled keep-alive connections ---

/// A provider that keeps HTTP/1.1 connections alive, answers every question of every request
/// with `p`, and counts the connections it accepted.
async fn keep_alive(p: f64) -> (u16, Arc<std::sync::atomic::AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let connections = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = connections.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tokio::spawn(async move {
                let mut buf: Vec<u8> = vec![];
                let mut chunk = [0u8; 8192];
                loop {
                    // One request: head, then its body.
                    let head_end = loop {
                        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            break i + 4;
                        }
                        match sock.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        }
                    };
                    let head = String::from_utf8_lossy(&buf[..head_end]).to_ascii_lowercase();
                    let length: usize = head
                        .lines()
                        .find_map(|l| {
                            l.strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap())
                        })
                        .unwrap_or(0);
                    while buf.len() < head_end + length {
                        match sock.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        }
                    }
                    let req: serde_json::Value =
                        serde_json::from_slice(&buf[head_end..head_end + length]).unwrap();
                    buf.drain(..head_end + length);
                    let answers: serde_json::Map<String, serde_json::Value> = req["questions"]
                        .as_object()
                        .unwrap()
                        .keys()
                        .map(|k| (k.clone(), serde_json::json!({"type": "noul", "noul": p})))
                        .collect();
                    let out =
                        serde_json::json!({"model": req["model"], "answers": answers}).to_string();
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{out}",
                        out.len()
                    );
                    if sock.write_all(reply.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    (port, connections)
}

#[tokio::test]
async fn the_client_reuses_a_pooled_connection() {
    let (port, connections) = keep_alive(0.7).await;
    let c = client(port, Duration::from_secs(5));

    for _ in 0..3 {
        assert_eq!(
            c.classify(&request()).await.unwrap(),
            vec![Some(0.7), Some(0.7)]
        );
    }

    assert_eq!(
        connections.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "one pooled connection serves sequential requests"
    );
}

// --- jev-mem T2.4: one transport for discovery and memory (PRD jev-mem §7) ---

use ripwire_broker::online::classifier::MemoryClassifier;
use ripwire_broker::online::request::{JevQuestion, StateRequest};
use ripwire_broker::online::response::Decision;

fn memory_request() -> StateRequest {
    StateRequest::new(
        "jev-1.13.0",
        serde_json::json!({"observation": "Evento: análise após edição."}),
        vec![
            JevQuestion::noul("episodic?"),
            JevQuestion::noul("semantic?"),
        ],
    )
}

#[tokio::test]
async fn the_memory_transport_posts_through_the_same_client() {
    let ok = r#"{"model":"jev-1.13.0","answers":{"q0":{"type":"noul","noul":0.9},"q1":{"type":"noul","noul":0.1}}}"#;
    let big = vec![b' '; MAX_RESPONSE_BYTES + 1];
    let f = fixture(vec![
        json(200, ok),
        json(401, "{}"),
        Some((200, vec![("content-type", "application/json".into())], big)),
    ])
    .await;
    let c = client(f.port, Duration::from_secs(5));
    let req = memory_request();

    let got = c.decide(&req).await.unwrap();
    assert_eq!(
        got,
        vec![
            Decision::Noul { probability: 0.9 },
            Decision::Noul { probability: 0.1 }
        ]
    );
    let seen = f.seen.lock().unwrap().clone();
    assert!(
        seen[0].head.starts_with("POST /v1/systemone HTTP/1.1\r\n"),
        "{}",
        seen[0].head
    );
    assert_eq!(
        seen[0].header("authorization").as_deref(),
        Some("Bearer tok-123")
    );
    assert_eq!(
        seen[0].body,
        serde_json::to_vec(&req).unwrap(),
        "body sent verbatim"
    );

    assert_eq!(c.decide(&req).await, Err(ClassifyError::Auth(401)));
    assert_eq!(
        c.decide(&req).await,
        Err(ClassifyError::TooLarge),
        "the same 256 KiB cap"
    );

    // Discovery and memory share one pooled connection.
    let (port, connections) = keep_alive(0.6).await;
    let c = client(port, Duration::from_secs(5));
    c.classify(&request()).await.unwrap();
    assert_eq!(
        c.decide(&memory_request()).await.unwrap()[0].probability(),
        Some(0.6)
    );
    assert_eq!(connections.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn memory_responses_are_not_counted_as_discovery_bytes() {
    let ok = r#"{"model":"jev-1.13.0","answers":{"q0":{"type":"noul","noul":0.9},"q1":{"type":"noul","noul":0.1}}}"#;
    let f = fixture(vec![json(200, ok)]).await;
    let c = client(f.port, Duration::from_secs(5));
    c.decide(&memory_request()).await.unwrap();
    assert_eq!(
        Classifier::response_bytes(&c).total,
        0,
        "jev_response_bytes stays discovery's"
    );
}

#[tokio::test]
async fn every_request_sent_is_recorded_for_the_status_line_even_a_refused_one() {
    use ripwire_broker::server_status::Activities;
    let f = fixture(vec![json(200, OK), json(500, "{}")]).await;
    let activity = Arc::new(Activities::default());
    let c = client(f.port, Duration::from_secs(5)).with_activity(activity.clone());
    let req = request();
    c.classify(&req).await.unwrap();
    let _ = c.classify(&req).await.unwrap_err();
    let now = ripwire_broker::hook::now();
    let total: u32 = activity.jev.calls(now).iter().map(|(_, n)| n).sum();
    assert_eq!(total, 2);
}

#[tokio::test]
async fn a_client_without_a_key_sends_nothing_and_says_so() {
    use ripwire_broker::server_status::{Activities, KeyState};
    let f = fixture(vec![json(200, OK)]).await;
    let activity = Arc::new(Activities::default());
    let c = JevClient::loopback_without_key(f.port, "jev-1.13.0", Duration::from_secs(5))
        .unwrap()
        .with_activity(activity.clone());
    let err = c.classify(&request()).await.unwrap_err();
    assert_eq!(err, ClassifyError::NoKey);
    assert_eq!(err.category(), "no_key");
    assert!(!err.is_transient(), "never retried");
    assert!(f.seen.lock().unwrap().is_empty(), "no request left");
    let now = ripwire_broker::hook::now();
    assert!(
        activity.jev.calls(now).is_empty(),
        "a skipped call is not a call"
    );
    assert_eq!(
        activity.key(),
        KeyState::Ok,
        "the server sets Missing, not the client"
    );
}

#[tokio::test]
async fn a_refused_key_marks_the_key_invalid_until_an_answer_succeeds() {
    use ripwire_broker::server_status::{Activities, KeyState};
    let f = fixture(vec![json(401, "{}"), json(200, OK)]).await;
    let activity = Arc::new(Activities::default());
    let c = client(f.port, Duration::from_secs(5)).with_activity(activity.clone());
    assert_eq!(
        c.classify(&request()).await.unwrap_err(),
        ClassifyError::Auth(401)
    );
    assert_eq!(activity.key(), KeyState::Invalid);
    c.classify(&request()).await.unwrap();
    assert_eq!(activity.key(), KeyState::Ok);
}

#[tokio::test]
async fn the_log_shows_each_call_sent_received_and_timed_without_the_key() {
    use ripwire_broker::online::log::JevLog;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("jev.log");
    let f = fixture(vec![
        json(200, OK),
        json(401, r#"{"error":"bad key tok-123"}"#),
    ])
    .await;
    let c = client(f.port, Duration::from_secs(5)).with_log(Arc::new(JevLog::open(&file).unwrap()));
    c.classify(&request()).await.unwrap();
    let _ = c.classify(&request()).await.unwrap_err();

    let text = std::fs::read_to_string(&file).unwrap();
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(text.matches("Jev call #").count(), 2, "{text}");
    for section in ["── enviado ", "── recebido ", "── duração "] {
        assert_eq!(text.matches(section).count(), 2, "{section}: {text}");
    }
    assert!(text.contains("HTTP 200"), "{text}");
    assert!(text.contains("HTTP 401"), "{text}");
    // Pretty printed: one key per line, indented.
    assert!(text.contains("\n  \"model\": \"jev-1.13.0\""), "{text}");
    assert!(text.contains("\"noul\": 0.81"), "{text}");
    assert!(text.contains(" ms\n"), "{text}");
    assert!(
        !text.contains("tok-123"),
        "the key never reaches the log: {text}"
    );
    assert!(text.contains("Bearer [redacted]"), "{text}");
}

// ---- `--jev-provider cloudflare` (D-166): the same protocol behind another transport ----

use ripwire_broker::online::JevProvider;

/// What Workers AI answers over REST: Cloudflare's v4 envelope around the `systemone` body.
const CF_OK: &str = r#"{"result":{"model":"clef-flash","answers":{"q0":{"type":"noul","noul":0.81},"q1":{"type":"noul","noul":0.03}},"usage":{"input_tokens":10,"output_tokens":0}},"success":true,"errors":[],"messages":[]}"#;

fn cloudflare(port: u16) -> JevClient {
    let key = Credential::from_env_value(Some("tok-123")).unwrap();
    let timeout = Duration::from_secs(5);
    JevClient::loopback_with(
        port,
        JevProvider::Cloudflare,
        Some("abc123"),
        key,
        "clef-flash",
        timeout,
    )
    .unwrap()
}

fn clef_request() -> JevRequest {
    let mut req = request();
    req.model = "clef-flash".into();
    req
}

#[tokio::test]
async fn a_cloudflare_envelope_is_unwrapped_before_the_answers() {
    let f = fixture(vec![json(200, CF_OK)]).await;

    let answers = cloudflare(f.port).classify(&clef_request()).await;

    assert_eq!(answers, Ok(vec![Some(0.81), Some(0.03)]));
}

#[tokio::test]
async fn the_memory_path_unwraps_the_cloudflare_envelope_too() {
    let body = r#"{"result":{"model":"clef-flash","answers":{"q0":{"type":"noul","noul":0.9},"q1":{"type":"noul","noul":0.1}}},"success":true,"errors":[],"messages":[]}"#;
    let f = fixture(vec![json(200, body)]).await;
    let mut req = memory_request();
    req.model = "clef-flash".into();

    let got = cloudflare(f.port).decide(&req).await;

    assert_eq!(
        got,
        Ok(vec![
            Decision::Noul { probability: 0.9 },
            Decision::Noul { probability: 0.1 }
        ])
    );
}

const CF_REFUSED: &str = r#"{"result":null,"success":false,"errors":[{"code":5006,"message":"model not found for tok-123"}],"messages":[]}"#;

#[tokio::test]
async fn a_cloudflare_refusal_is_invalid_with_its_errors() {
    let f = fixture(vec![json(200, CF_REFUSED)]).await;

    let err = cloudflare(f.port)
        .classify(&clef_request())
        .await
        .unwrap_err();

    let ClassifyError::Invalid(InvalidResponse::Refused(why)) = &err else {
        panic!("{err:?}");
    };
    assert!(
        why.contains("5006") && why.contains("model not found"),
        "{why}"
    );
    assert!(!why.contains("tok-123"), "the key is redacted: {why}");
    assert!(why.len() <= 256, "capped: {}", why.len());
    assert_eq!(err.category(), "invalid_response");
    assert!(err.to_string().contains("5006"), "{err}");
}

#[tokio::test]
async fn a_cloudflare_answer_without_the_envelope_is_still_accepted() {
    let bare = r#"{"model":"clef-flash","answers":{"q0":{"type":"noul","noul":0.81},"q1":{"type":"noul","noul":0.03}}}"#;
    let f = fixture(vec![json(200, bare)]).await;

    let answers = cloudflare(f.port).classify(&clef_request()).await;

    assert_eq!(answers, Ok(vec![Some(0.81), Some(0.03)]));
}

/// The envelope is opened, never re-serialized: the parser still sees a question answered twice.
#[tokio::test]
async fn a_question_answered_twice_inside_the_envelope_is_still_seen() {
    let twice = r#"{"result":{"model":"clef-flash","answers":{"q0":{"type":"noul","noul":0.1},"q0":{"type":"noul","noul":0.9}}},"success":true,"errors":[],"messages":[]}"#;
    let f = fixture(vec![json(200, twice)]).await;

    let answers = cloudflare(f.port).classify(&clef_request()).await;

    assert_eq!(
        answers,
        Err(ClassifyError::Invalid(InvalidResponse::DuplicateQuestion))
    );
}

/// TypeSafe never answers with an envelope, so one is not opened for it.
#[tokio::test]
async fn the_typesafe_client_does_not_open_envelopes() {
    let f = fixture(vec![json(200, CF_OK), json(200, CF_REFUSED)]).await;
    let c = client(f.port, Duration::from_secs(5));
    let malformed = Err(ClassifyError::Invalid(InvalidResponse::Malformed));

    assert_eq!(c.classify(&request()).await, malformed);
    assert_eq!(c.classify(&request()).await, malformed);
}

/// What Workers AI does in practice: a refused request is a 4xx, with the envelope as its body.
/// The status decides the category, as for any provider, and the errors are in the log.
#[tokio::test]
async fn a_cloudflare_4xx_keeps_its_status_and_logs_its_errors() {
    use ripwire_broker::online::log::JevLog;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("jev.log");
    let f = fixture(vec![json(422, CF_REFUSED)]).await;
    let c = cloudflare(f.port).with_log(Arc::new(JevLog::open(&file).unwrap()));

    assert_eq!(
        c.classify(&clef_request()).await,
        Err(ClassifyError::Rejected(422))
    );

    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("HTTP 422") && text.contains("5006"), "{text}");
    assert!(text.contains("erro: rejected"), "{text}");
    assert!(!text.contains("tok-123"), "{text}");
}

/// The path Workers AI serves, with the same body and bearer as TypeSafe's.
#[tokio::test]
async fn the_cloudflare_client_posts_to_the_run_path_of_its_account_and_model() {
    let f = fixture(vec![json(200, CF_OK)]).await;
    let req = clef_request();

    cloudflare(f.port).classify(&req).await.unwrap();

    let seen = f.seen.lock().unwrap().clone();
    assert!(
        seen[0].head.starts_with(
            "POST /client/v4/accounts/abc123/ai/run/@cf/cloudflare/clef-flash HTTP/1.1\r\n"
        ),
        "{}",
        seen[0].head
    );
    assert_eq!(
        seen[0].header("authorization").as_deref(),
        Some("Bearer tok-123")
    );
    assert_eq!(seen[0].body, serde_json::to_vec(&req).unwrap());
}

type ForProvider =
    fn(JevProvider, Option<&str>, Option<Credential>, &str, Duration) -> Result<JevClient, String>;

/// The only production constructor takes a provider's name, never a URL: adding one fails to
/// compile here. So the bearer can only go to one of the two allowlisted hosts, over HTTPS; the
/// client's `https_only` and no-redirect settings are defence in depth behind this (the refused
/// redirect is exercised in `redirects_are_refused`).
#[test]
fn the_provider_client_cannot_be_pointed_anywhere_but_an_allowlisted_https_endpoint() {
    let new: ForProvider = JevClient::for_provider;
    let key = || Credential::from_env_value(Some("tok-123")).ok();
    let timeout = Duration::from_secs(15);

    let cf = new(
        JevProvider::Cloudflare,
        Some("abc123"),
        key(),
        "clef",
        timeout,
    )
    .unwrap();
    assert_eq!(
        cf.endpoint(),
        "https://api.cloudflare.com/client/v4/accounts/abc123/ai/run/@cf/cloudflare/clef"
    );
    let ts = new(JevProvider::TypeSafe, None, key(), "jev-1.13.0", timeout).unwrap();
    assert_eq!(ts.endpoint(), "https://api.typesafe.ai/v1/systemone");

    for account in ["evil.example/x", "a@evil.example", "..", "a/../../b"] {
        let refused = new(
            JevProvider::Cloudflare,
            Some(account),
            key(),
            "clef",
            timeout,
        );
        assert!(refused.is_err(), "{account:?}");
    }
    assert!(new(JevProvider::Cloudflare, None, key(), "clef", timeout).is_err());
    assert!(
        new(
            JevProvider::Cloudflare,
            Some("abc123"),
            key(),
            "jev-1.13.0",
            timeout
        )
        .is_err()
    );
}

/// D-166: the account is not a credential, but it names the tenant and the log is made to be
/// read and pasted. It is written redacted, like the bearer.
#[tokio::test]
async fn the_log_redacts_the_cloudflare_account() {
    use ripwire_broker::online::log::JevLog;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("jev.log");
    // An answer that echoes the URL it was asked on, as an error message may.
    let echo = r#"{"result":null,"success":false,"errors":[{"code":7003,"message":"no route for /client/v4/accounts/abc123/ai/run"}],"messages":[]}"#;
    let f = fixture(vec![json(200, CF_OK), json(404, echo)]).await;
    let c = cloudflare(f.port).with_log(Arc::new(JevLog::open(&file).unwrap()));

    c.classify(&clef_request()).await.unwrap();
    let _ = c.classify(&clef_request()).await.unwrap_err();

    let text = std::fs::read_to_string(&file).unwrap();
    let port = f.port;
    assert!(
        text.contains(&format!(
            "POST http://127.0.0.1:{port}/client/v4/accounts/[redacted]/ai/run/@cf/cloudflare/clef-flash\n"
        )),
        "{text}"
    );
    assert!(
        !text.contains("abc123"),
        "the account never reaches the log: {text}"
    );
    // The envelope is logged as it came: unwrapping is the client's, after the log.
    assert!(
        text.contains("\"success\": true") && text.contains("\"noul\": 0.81"),
        "{text}"
    );
    assert!(text.contains("HTTP 404") && text.contains("7003"), "{text}");
}

/// The default provider's log is what it was before there was a second one (D-166).
#[tokio::test]
async fn the_typesafe_log_is_what_it_always_was() {
    use ripwire_broker::online::log::JevLog;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("jev.log");
    let f = fixture(vec![json(200, OK)]).await;
    let c = client(f.port, Duration::from_secs(5)).with_log(Arc::new(JevLog::open(&file).unwrap()));
    let req = request();

    c.classify(&req).await.unwrap();

    let text = std::fs::read_to_string(&file).unwrap();
    let pretty = |json: &str| {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        serde_json::to_string_pretty(&v).unwrap()
    };
    let rule = |c: &str| c.repeat(72);
    let section = |title: &str| {
        let head = format!("── {title} ");
        format!("{head}{}\n", "─".repeat(72 - head.chars().count()))
    };
    let (head, rest) = text.split_once("\n\n").unwrap();
    let stamp = head
        .lines()
        .nth(1)
        .and_then(|l| l.strip_prefix("Jev call #1 · "))
        .and_then(|l| l.strip_suffix(" · discovery"))
        .unwrap_or_else(|| panic!("{head}"));
    assert_eq!(stamp.len(), "2026-10-06 18:52:01 UTC".len(), "{stamp}");
    assert_eq!(
        head,
        format!(
            "{}\nJev call #1 · {stamp} · discovery\n{}",
            rule("═"),
            rule("═")
        )
    );
    let (body, took) = rest.split_once(&section("duração")).unwrap();
    let expected = format!(
        "{}POST http://127.0.0.1:{}/v1/systemone\nAuthorization: Bearer [redacted]\nContent-Type: application/json\n\n{}\n\n{}HTTP 200\n\n{}\n\n",
        section("enviado"),
        f.port,
        pretty(&serde_json::to_string(&req).unwrap()),
        section("recebido"),
        pretty(OK),
    );
    assert_eq!(body, expected);
    assert!(
        took.ends_with(" ms\n\n") && took.trim_end_matches(" ms\n\n").parse::<u64>().is_ok(),
        "{took:?}"
    );
}
