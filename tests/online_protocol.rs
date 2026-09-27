//! Seam 4: the real `JevClient` against a local HTTP/1.1 fixture server (PRD §23.14). Needs
//! `--features online`; the fixture only listens on 127.0.0.1.
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

#[test]
fn the_production_client_accepts_only_the_allowlisted_https_endpoint() {
    let key = Credential::from_env_value(Some("tok-123")).unwrap();
    let c = JevClient::new(key, "jev-1.13.0", Duration::from_secs(15)).unwrap();
    assert_eq!(c.endpoint(), "https://api.typesafe.ai/v1/systemone");
    assert_eq!(c.model(), "jev-1.13.0");
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

#[tokio::test]
async fn an_mcp_cancel_aborts_http_requests_in_flight() {
    use ripwire_broker::broker::{Broker, BrokerConfig, TaskRequest};
    use ripwire_broker::online::OnlineConfig;
    use std::sync::atomic::Ordering::SeqCst;

    let ws = tempfile::tempdir().unwrap();
    for (path, body) in [
        ("src/auth.py", "def login(user, token):\n    return user\n"),
        ("src/routes.py", "def export_route(req):\n    pass\n"),
        ("tests/test_auth.py", "def test_login():\n    pass\n"),
    ] {
        common::write(ws.path(), path, body);
    }
    let (port, seen, closed) = silent().await;
    let mut config = BrokerConfig::new(ws.path());
    config.online = Some(OnlineConfig::new(Arc::new(client(
        port,
        Duration::from_secs(30),
    ))));
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
    let sent = *seen.lock().unwrap();
    assert!(sent > 0, "a request reached the provider");

    call.abort(); // what the MCP handler does on notifications/cancelled (RF-14)
    let until = tokio::time::Instant::now() + Duration::from_millis(250);
    while closed.load(SeqCst) < sent && tokio::time::Instant::now() < until {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    assert_eq!(
        closed.load(SeqCst),
        sent,
        "every HTTP request in flight was aborted within 250 ms"
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
