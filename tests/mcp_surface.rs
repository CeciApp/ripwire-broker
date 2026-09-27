//! Seam 2: the broker binary as an MCP server over stdio, driven by an SDK client.
mod common;

use async_trait::async_trait;
use rust_mcp_sdk::mcp_client::{ClientHandler, ClientRuntime, McpClientOptions, client_runtime};
use rust_mcp_sdk::schema::{
    CallToolRequestParams, ClientCapabilities, Implementation, ReadResourceContent,
    ReadResourceRequestParams,
};
use rust_mcp_sdk::{
    ClientDetails, McpClient, StdioTransport, ToMcpClientHandler, TransportOptions,
};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;

macro_rules! require_ripwire {
    () => {
        if !common::ripwire_available() {
            eprintln!("skipping: ripwire not on PATH");
            return;
        }
    };
}

struct Quiet;
#[async_trait]
impl ClientHandler for Quiet {}

async fn start_broker(workspace: &Path, extra: &[&str]) -> Arc<ClientRuntime> {
    let mut args = vec!["--workspace".to_string(), workspace.display().to_string()];
    args.extend(extra.iter().map(|s| s.to_string()));
    let transport = StdioTransport::create_with_server_launch(
        env!("CARGO_BIN_EXE_ripwire-broker"),
        args,
        None,
        TransportOptions::default(),
    )
    .unwrap();
    let details = ClientDetails {
        client_info: Implementation {
            name: "e2e".into(),
            version: "0".into(),
            title: None,
            description: None,
            icons: vec![],
            website_url: None,
        },
        capabilities: ClientCapabilities::default(),
    };
    let client = client_runtime::create_client(McpClientOptions::new(
        details,
        transport,
        Quiet.to_mcp_client_handler(),
    ));
    client.clone().start().await.unwrap();
    client
}

async fn call(client: &ClientRuntime, tool: &str, args: Value) -> (bool, Value) {
    let res = client
        .call_tool(CallToolRequestParams {
            name: tool.into(),
            arguments: args.as_object().cloned(),
            input_responses: None,
            meta: Default::default(),
            request_state: None,
        })
        .await
        .unwrap();
    let structured = res.structured_content.clone().expect("structured content");
    (res.is_error.unwrap_or(false), structured)
}

async fn status(client: &ClientRuntime) -> Value {
    let res = client
        .request_resource_read(ReadResourceRequestParams {
            uri: "ripwire-broker://status".into(),
            input_responses: None,
            meta: Default::default(),
            request_state: None,
        })
        .await
        .unwrap();
    match &res.contents[0] {
        ReadResourceContent::TextResourceContents(t) => serde_json::from_str(&t.text).unwrap(),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn publishes_three_read_only_tools_and_a_status_resource() {
    require_ripwire!();
    let repo = common::sample_repo();
    let client = start_broker(repo.path(), &[]).await;

    let tools = client.request_tool_list(None).await.unwrap().tools;
    let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "context_after_edit",
            "context_before_finish",
            "context_for_task"
        ]
    );
    for t in &tools {
        let schema = serde_json::to_value(&t.input_schema).unwrap();
        assert_eq!(
            schema["additionalProperties"], false,
            "{} schema must be strict: {schema}",
            t.name
        );
        assert_eq!(
            t.annotations.as_ref().and_then(|a| a.read_only_hint),
            Some(true),
            "{}",
            t.name
        );
    }
    let resources = client.request_resource_list(None).await.unwrap().resources;
    assert_eq!(
        resources.iter().map(|r| r.uri.as_str()).collect::<Vec<_>>(),
        vec!["ripwire-broker://status"]
    );
    client.shut_down().await.unwrap();
}

#[tokio::test]
async fn context_for_task_answers_from_the_real_ripwire_under_budget() {
    require_ripwire!();
    let repo = common::sample_repo();
    let client = start_broker(repo.path(), &[]).await;

    let (is_error, out) = call(
        &client,
        "context_for_task",
        json!({"task": "how is the login token validated?", "budget_tokens": 600}),
    )
    .await;

    assert!(!is_error, "{out}");
    assert_eq!(out["schema_version"], "ripwire-broker.context/v1");
    assert!(out["budget"]["estimated_tokens"].as_u64().unwrap() <= 600);
    assert!(
        out["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["symbol"] == "validate_token"),
        "{out:#}"
    );
    let st = status(&client).await;
    assert_eq!(st["upstream"]["available"], true);
    assert_eq!(st["metrics"]["tools"]["context_for_task"]["calls"], 1);
    client.shut_down().await.unwrap();
}

#[tokio::test]
async fn invalid_arguments_and_outside_paths_are_structured_errors() {
    require_ripwire!();
    let repo = common::sample_repo();
    let client = start_broker(repo.path(), &[]).await;

    let (is_error, out) = call(
        &client,
        "context_after_edit",
        json!({"files": ["../../etc/passwd"]}),
    )
    .await;
    assert!(is_error);
    assert_eq!(out["error"], "workspace_violation");

    let (is_error, out) = call(
        &client,
        "context_for_task",
        json!({"task": "x", "mode": "yolo"}),
    )
    .await;
    assert!(is_error);
    assert_eq!(out["error"], "invalid_input");

    let (is_error, out) = call(
        &client,
        "context_for_task",
        json!({"task": "x", "unexpected": 1}),
    )
    .await;
    assert!(is_error);
    assert_eq!(out["error"], "invalid_input");
    client.shut_down().await.unwrap();
}

#[tokio::test]
async fn without_ripwire_the_tools_fail_explicitly_and_status_says_why() {
    let repo = tempfile::tempdir().unwrap();
    let client = start_broker(repo.path(), &["--ripwire", "/nonexistent/ripwire"]).await;

    let (is_error, out) = call(
        &client,
        "context_for_task",
        json!({"task": "how does auth work?"}),
    )
    .await;

    assert!(is_error);
    assert_eq!(out["error"], "upstream_unavailable");
    assert!(out.get("items").is_none(), "no fabricated context");
    let st = status(&client).await;
    assert_eq!(st["upstream"]["available"], false);
    client.shut_down().await.unwrap();
}

#[test]
fn the_build_has_no_network_stack() {
    // CA-10: stdio only; no HTTP client or server crate may be linked in the default build.
    let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock")).unwrap();
    for net in [
        "name = \"reqwest\"",
        "name = \"hyper\"",
        "name = \"axum\"",
        "name = \"rustls\"",
    ] {
        assert!(
            !lock.contains(net),
            "network crate in dependency tree: {net}"
        );
    }
}

fn references(out: &Value) -> usize {
    out["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| {
            i["why_included"]
                .as_str()
                .unwrap()
                .contains("already delivered")
        })
        .count()
}

#[tokio::test]
async fn the_server_is_stateless_by_default_and_incremental_on_request() {
    require_ripwire!();
    let repo = common::sample_repo();
    let ask = json!({"task": "how is login validated?", "mode": "orient"});

    let stateless = start_broker(repo.path(), &[]).await;
    let (_, a) = call(&stateless, "context_for_task", ask.clone()).await;
    let (_, b) = call(&stateless, "context_for_task", ask.clone()).await;
    assert!(!a["items"].as_array().unwrap().is_empty(), "{a}");
    assert_eq!(a["items"], b["items"], "no session unless --incremental");
    stateless.shut_down().await.unwrap();

    let session = start_broker(repo.path(), &["--incremental"]).await;
    let tools = session.request_tool_list(None).await.unwrap().tools;
    for name in ["context_for_task", "context_after_edit"] {
        let t = tools.iter().find(|t| t.name == name).unwrap();
        let schema = serde_json::to_value(&t.input_schema).unwrap();
        assert_eq!(
            schema["properties"]["include_seen"]["type"], "boolean",
            "{name}"
        );
    }
    let (_, first) = call(&session, "context_for_task", ask.clone()).await;
    let (_, repeat) = call(&session, "context_for_task", ask.clone()).await;
    let mut again = ask.clone();
    again["include_seen"] = json!(true);
    let (_, full) = call(&session, "context_for_task", again).await;
    assert_eq!(references(&first), 0);
    assert_eq!(
        references(&repeat),
        repeat["items"].as_array().unwrap().len(),
        "{repeat}"
    );
    assert_eq!(full["items"], first["items"]);
    let st = status(&session).await;
    assert!(st["metrics"]["session_hits"].as_u64().unwrap() > 0, "{st}");
    session.shut_down().await.unwrap();
}

#[tokio::test]
async fn the_server_adds_notes_with_a_command_summarizer() {
    require_ripwire!();
    use std::os::unix::fs::PermissionsExt;
    let repo = common::sample_repo();
    let bin = tempfile::tempdir().unwrap();
    let script = bin.path().join("fake-llm");
    std::fs::write(
        &script,
        "#!/bin/sh\ncat >/dev/null\necho 'Authentication helpers and their callers.'\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let client = start_broker(
        repo.path(),
        &[
            "--summarizer-cmd",
            script.to_str().unwrap(),
            "--summarizer-wait-ms",
            "10000",
        ],
    )
    .await;

    let (is_error, out) = call(
        &client,
        "context_for_task",
        json!({"task": "how is login validated?", "mode": "orient"}),
    )
    .await;

    assert!(!is_error, "{out}");
    let notes = out["notes"].as_array().unwrap_or_else(|| panic!("{out}"));
    assert!(!notes.is_empty());
    assert_eq!(
        notes[0]["text"]["untrusted_repository_data"],
        "Authentication helpers and their callers."
    );
    assert_eq!(notes[0]["generated"], true);
    let st = status(&client).await;
    assert_eq!(st["summarizer"]["enabled"], true);
    assert_eq!(st["summarizer"]["program"], "fake-llm");
    assert!(!st.to_string().contains("Authentication helpers"), "{st}");
    client.shut_down().await.unwrap();
}

/// The broker binary spoken to in raw JSON-RPC, so the test owns the request ids.
struct Raw {
    child: std::process::Child,
    lines: std::sync::mpsc::Receiver<Value>,
}

impl Raw {
    fn start(args: &[&str]) -> Self {
        use std::io::BufRead;
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_ripwire-broker"))
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let out = child.stdout.take().unwrap();
        let (tx, lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str(&line) {
                    let _ = tx.send(v);
                }
            }
        });
        Self { child, lines }
    }

    fn send(&mut self, msg: Value) {
        use std::io::Write;
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, id: i64, method: &str, mut params: Value) {
        params["_meta"] = json!({
            "io.modelcontextprotocol/clientCapabilities": {},
            "io.modelcontextprotocol/clientInfo": {"name": "raw", "version": "0"},
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        });
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
    }

    fn response(&self, id: i64, within: std::time::Duration) -> Option<Value> {
        let deadline = std::time::Instant::now() + within;
        while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
            match self.lines.recv_timeout(left) {
                Ok(v) if v["id"] == id => return Some(v),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
        None
    }
}

impl Drop for Raw {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[test]
fn a_client_cancellation_stops_the_tool_call() {
    let ws = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let ripwire = common::slow_ripwire(bin.path());
    let mut broker = Raw::start(&[
        "--workspace",
        ws.path().to_str().unwrap(),
        "--ripwire",
        ripwire.to_str().unwrap(),
    ]);

    broker.request(
        7,
        "tools/call",
        json!({"name": "context_for_task", "arguments": {"task": "slow task", "mode": "orient"}}),
    );
    // Cancel only once ripwire is actually busy with this call.
    let busy = bin.path().join("busy");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !busy.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "the slow call never reached ripwire"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    broker.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/cancelled",
        "params": {"requestId": 7, "reason": "user pressed escape"},
    }));

    let answer = broker
        .response(7, std::time::Duration::from_secs(5))
        .expect("the cancelled call returns at once, not after the 30 s upstream call");
    let out = &answer["result"]["structuredContent"];
    assert_eq!(out["error"], "cancelled", "{answer}");

    broker.request(
        8,
        "resources/read",
        json!({"uri": "ripwire-broker://status"}),
    );
    let status = broker
        .response(8, std::time::Duration::from_secs(5))
        .expect("the status answers even while ripwire is busy (RF-13)");
    let text = status["result"]["contents"][0]["text"].as_str().unwrap();
    let status: Value = serde_json::from_str(text).unwrap();
    assert_eq!(
        status["metrics"]["tools"]["context_for_task"]["cancelled"], 1,
        "{status}"
    );
    let last = status["metrics"]["recent_requests"]
        .as_array()
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    assert_eq!(last["outcome"], "cancelled");
    // ripwire is still busy with the abandoned call: the status says so instead of hanging.
    assert_eq!(status["upstream"]["busy"], true, "{status}");
    assert_eq!(status["upstream"]["available"], false);
}

// --- D-052 #8: a hanging reconnect must not hold the status resource ---

#[test]
fn the_status_answers_while_a_reconnect_hangs() {
    use std::os::unix::fs::PermissionsExt;
    let ws = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let marker = bin.path().join("launched-once");
    let hanging = bin.path().join("hanging");
    let ripwire = bin.path().join("hanging-ripwire");
    // First launch fails at once (degraded start); every later launch hangs.
    std::fs::write(
        &ripwire,
        format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'ripwire 0.6.4'; exit 0; fi\n\
             if [ ! -f '{m}' ]; then touch '{m}'; exit 3; fi\ntouch '{h}'\nexec sleep 60\n",
            m = marker.display(),
            h = hanging.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&ripwire, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut broker = Raw::start(&[
        "--workspace",
        ws.path().to_str().unwrap(),
        "--ripwire",
        ripwire.to_str().unwrap(),
        "--timeout-ms",
        "20000",
    ]);

    broker.request(
        1,
        "tools/call",
        json!({"name": "context_for_task", "arguments": {"task": "anything", "mode": "orient"}}),
    );
    // Ask only once the reconnect is really under way.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !hanging.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "no reconnect was attempted"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    broker.request(
        2,
        "resources/read",
        json!({"uri": "ripwire-broker://status"}),
    );

    let status = broker
        .response(2, std::time::Duration::from_secs(3))
        .expect("the status answers during a reconnect (RF-13)");
    let text = status["result"]["contents"][0]["text"].as_str().unwrap();
    let status: Value = serde_json::from_str(text).unwrap();
    assert_eq!(status["upstream"]["available"], false, "{status}");
    assert_eq!(status["upstream"]["reconnecting"], true, "{status}");
}

// --- D-052 #9: the cancellation registry keeps nothing once calls are over ---

#[test]
fn finished_calls_and_late_cancels_leave_nothing_behind() {
    let ws = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let ripwire = common::slow_ripwire(bin.path());
    let mut broker = Raw::start(&[
        "--workspace",
        ws.path().to_str().unwrap(),
        "--ripwire",
        ripwire.to_str().unwrap(),
    ]);
    let second = std::time::Duration::from_secs(5);

    for (id, task) in [(1, "first quick task"), (2, "second quick task")] {
        broker.request(
            id,
            "tools/call",
            json!({"name": "context_for_task", "arguments": {"task": task, "mode": "orient"}}),
        );
        broker.response(id, second).expect("a quick call answers");
    }
    // A cancel that arrives after its call finished (the common race).
    broker.send(
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 1}}),
    );
    std::thread::sleep(std::time::Duration::from_millis(200));
    broker.request(
        3,
        "resources/read",
        json!({"uri": "ripwire-broker://status"}),
    );

    let status = broker.response(3, second).unwrap();
    let status: Value =
        serde_json::from_str(status["result"]["contents"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        status["inflight"],
        json!({"tracked_calls": 0, "early_cancels": 0}),
        "{status}"
    );
    assert!(
        !status.to_string().contains("quick task"),
        "no task text in the status"
    );
}
