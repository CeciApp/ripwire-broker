//! MCP facade (rust-mcp-sdk 2.0.0): three tools and one status resource over stdio.
//! The only module that knows about the SDK's server types; the core stays SDK-agnostic.

use crate::broker::{
    Broker, BrokerConfig, BrokerError, EditRequest, FinishRequest, MAX_BUDGET_TOKENS,
    MIN_BUDGET_TOKENS, MIN_ONLINE_BUDGET_TOKENS, Mode, TaskRequest,
};
use crate::model::SCHEMA_VERSION;
use crate::upstream::{RipwireUpstream, UpstreamConfig};
use async_trait::async_trait;
use rust_mcp_sdk::McpObserver;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::schema_utils::{CallToolError, ClientMessage, ServerMessage};
use rust_mcp_sdk::schema::{
    CallToolRequestParams, CallToolResult, CancelledNotificationParams, ListResourcesResult,
    ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams, RpcError, ServerResult,
    Tool,
};
use rust_mcp_sdk::{McpServer, RequestContext};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

pub const STATUS_URI: &str = "ripwire-broker://status";

/// Everything needed to (re)build the broker if ripwire was not reachable at startup.
#[derive(Clone)]
pub struct Settings {
    pub upstream: UpstreamConfig,
    pub broker: BrokerConfig,
}

pub struct BrokerServer {
    settings: Settings,
    broker: Mutex<Option<Arc<Broker>>>,
    last_connect_error: Mutex<Option<BrokerError>>,
    inflight: Arc<Inflight>,
}

/// Tool calls in flight by JSON-RPC id, so `notifications/cancelled` can stop one (RF-14).
/// The SDK hands handlers no request id, so the message observer, which sees every raw
/// request, queues each `tools/call` id under its (tool, arguments) key and the handler
/// takes it back. Two identical concurrent calls may swap ids; cancelling either then stops
/// identical work.
#[derive(Default)]
struct Inflight {
    waiting: std::sync::Mutex<HashMap<String, VecDeque<String>>>,
    running: std::sync::Mutex<HashMap<String, Arc<Notify>>>,
    /// Cancelled before its handler registered.
    early: std::sync::Mutex<HashSet<String>>,
}

fn call_key(name: &str, arguments: &Value) -> String {
    format!("{name} {arguments}")
}

impl Inflight {
    fn received(&self, message: &ClientMessage) {
        let Ok(v) = serde_json::to_value(message) else {
            return;
        };
        if v["method"] != "tools/call" {
            return;
        }
        // Only what a handler can actually receive. The SDK answers a call whose params do
        // not convert into `CallToolRequestParams` — a missing or non-string `name`, a
        // non-object `arguments` — before any handler runs, so queuing its id would leave
        // an entry, and the task text in its key, that no `start()` can ever claim (D-092).
        let arguments = &v["params"]["arguments"];
        let Some(name) = v["params"]["name"].as_str() else {
            return;
        };
        if !(arguments.is_null() || arguments.is_object()) {
            return;
        }
        let key = call_key(name, arguments);
        self.waiting
            .lock()
            .unwrap()
            .entry(key)
            .or_default()
            .push_back(v["id"].to_string());
    }

    /// Claims this call's id and a signal that fires if the client cancels it.
    fn start(&self, params: &CallToolRequestParams) -> Option<(String, Arc<Notify>)> {
        let arguments = params
            .arguments
            .clone()
            .map(Value::Object)
            .unwrap_or(Value::Null);
        let key = call_key(&params.name, &arguments);
        let id = {
            let mut waiting = self.waiting.lock().unwrap();
            let queue = waiting.get_mut(&key)?;
            let id = queue.pop_front()?;
            if queue.is_empty() {
                // Keys hold task text: never keep one longer than its call (D-052).
                waiting.remove(&key);
            }
            id
        };
        let signal = Arc::new(Notify::new());
        if self.early.lock().unwrap().remove(&id) {
            signal.notify_one();
        }
        self.running
            .lock()
            .unwrap()
            .insert(id.clone(), signal.clone());
        Some((id, signal))
    }

    fn finish(&self, id: &str) {
        self.running.lock().unwrap().remove(id);
    }

    fn cancel(&self, id: String) {
        if let Some(signal) = self.running.lock().unwrap().get(&id) {
            signal.notify_one();
            return;
        }
        // Keep an early cancel only for a call still waiting for its handler; a cancel for
        // a call that already finished (the usual race) is dropped, not stored forever.
        let waiting = self.waiting.lock().unwrap();
        if waiting.values().any(|q| q.contains(&id)) {
            self.early.lock().unwrap().insert(id);
        }
    }

    /// Counts for the status resource; the keys themselves hold task text.
    fn counts(&self) -> Value {
        json!({
            "tracked_calls": self.waiting.lock().unwrap().len() + self.running.lock().unwrap().len(),
            "early_cancels": self.early.lock().unwrap().len(),
        })
    }
}

/// Feeds raw requests to `Inflight`; see there.
struct CancelObserver(Arc<Inflight>);

impl McpObserver<ClientMessage, ServerMessage> for CancelObserver {
    fn on_receive(&self, message: &ClientMessage) {
        self.0.received(message);
    }
}

impl BrokerServer {
    pub async fn start(settings: Settings) -> Self {
        let server = Self {
            settings,
            broker: Mutex::new(None),
            last_connect_error: Mutex::new(None),
            inflight: Arc::default(),
        };
        let _ = server.broker().await;
        server
    }

    /// The connected broker, connecting on demand while ripwire is unavailable (degraded mode).
    async fn broker(&self) -> Result<Arc<Broker>, BrokerError> {
        let mut slot = self.broker.lock().await;
        if let Some(b) = slot.as_ref() {
            return Ok(b.clone());
        }
        let connected = match RipwireUpstream::spawn(self.settings.upstream.clone()).await {
            Ok(up) => Broker::connect(Arc::new(up), self.settings.broker.clone()).await,
            Err(e) => Err(e.into()),
        };
        match connected {
            Ok(b) => {
                let b = Arc::new(b);
                *slot = Some(b.clone());
                Ok(b)
            }
            Err(e) => {
                *self.last_connect_error.lock().await = Some(e.clone());
                Err(e)
            }
        }
    }

    /// Pass it as the server's `message_observer`: it lets client cancellations reach tool calls.
    pub fn observer(&self) -> Arc<dyn McpObserver<ClientMessage, ServerMessage>> {
        Arc::new(CancelObserver(self.inflight.clone()))
    }

    async fn status_json(&self) -> Value {
        // `broker()` holds this lock while it reconnects, up to the upstream timeout; the
        // status never waits for that (RF-13, D-052).
        let (connected, reconnecting) = match self.broker.try_lock() {
            Ok(slot) => (slot.clone(), false),
            Err(_) => (None, true),
        };
        let mut status = match connected {
            Some(b) => serde_json::to_value(b.status().await).unwrap_or(Value::Null),
            None => {
                let err = self.last_connect_error.lock().await.clone();
                json!({
                    "broker_version": env!("CARGO_PKG_VERSION"),
                    "schema_version": SCHEMA_VERSION,
                    "offline": self.settings.broker.online.is_none(),
                    "telemetry": "none",
                    "workspace": if self.settings.broker.redact_workspace { "<redacted>".to_string() } else { self.settings.broker.workspace.display().to_string() },
                    "upstream": {
                        "ripwire_version": self.settings.broker.ripwire_version,
                        "available": false,
                        "busy": false,
                        "reconnecting": reconnecting,
                        "restarts": 0,
                        "last_error": err.map(|e| e.error),
                    },
                })
            }
        };
        // Without a connected broker the online block has no totals yet, but the mode is
        // still published (RF-ONLINE-15).
        if let (Some(o), None) = (&self.settings.broker.online, status.get("online")) {
            status["online"] = json!({
                "enabled": true,
                "provider": o.provider,
                "model": o.classifier.model(),
                "endpoint_host": o.endpoint_host,
                "max_in_flight": o.max_in_flight,
                "request_limit": o.request_limit,
            });
        }
        status["inflight"] = self.inflight.counts();
        status
    }
}

fn budget_schema(default: u32) -> Value {
    budget_schema_from(default, MIN_BUDGET_TOKENS)
}

fn budget_schema_from(default: u32, minimum: u32) -> Value {
    json!({"type": "integer", "minimum": minimum, "maximum": MAX_BUDGET_TOKENS, "default": default,
           "description": "Upper bound for the estimated tokens of the whole answer (4 bytes of JSON per token)."})
}

fn include_seen_schema() -> Value {
    json!({"type": "boolean", "default": false,
           "description": "Resend items this session already received (they otherwise come back as short references)."})
}

fn tool(name: &str, title: &str, description: &str, properties: Value, required: &[&str]) -> Tool {
    serde_json::from_value(json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        },
        "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false},
    }))
    .expect("static tool definition")
}

/// The public tools; `online` raises the `context_for_task` budget floor (D-072).
pub fn tools(online: bool) -> Vec<Tool> {
    let task_floor = match online {
        true => MIN_ONLINE_BUDGET_TOKENS,
        false => MIN_BUDGET_TOKENS,
    };
    vec![
        tool(
            "context_for_task",
            "Context for a task",
            "Call FIRST, before searching or opening files: returns the smallest sufficient code context for the task \
             (ranked symbols, key bodies, callers, tests, risks, limitations) under a token budget. Accepts a request in \
             plain words, a symbol name, or a pasted stack trace.",
            json!({
                "task": {"type": "string", "minLength": 1, "description": "The request in natural language, a symbol, or a raw stack trace."},
                "budget_tokens": budget_schema_from(2500, task_floor),
                "mode": {"type": "string", "enum": ["auto", "orient", "debug", "change", "review"], "default": "auto",
                         "description": "auto routes by content; set it to force a route."},
                "include_docs": {"type": "boolean", "default": true, "description": "Allow related documentation."},
                "include_bodies": {"type": "boolean", "default": true, "description": "Allow full bodies within the budget."},
                "include_seen": include_seen_schema(),
            }),
            &["task"],
        ),
        tool(
            "context_after_edit",
            "Context after an edit",
            "Call after a relevant change: what the edit may have affected (changed contracts, incompatible callers, \
             reached files, missing co-change partners, tests) without repeating the initial context.",
            json!({
                "files": {"type": "array", "items": {"type": "string"}, "description": "Changed files; omit for the working tree."},
                "symbols": {"type": "array", "items": {"type": "string"}, "description": "Symbols deliberately modified."},
                "budget_tokens": budget_schema(1500),
                "include_seen": include_seen_schema(),
            }),
            &[],
        ),
        tool(
            "context_before_finish",
            "Gate before finishing",
            "Call before declaring the task done: quality regressions, tests to run and open obligations. \
             status is ready | attention_required | unknown; ready means no open obligation was detected, not that the code is correct.",
            json!({
                "budget_tokens": budget_schema(1800),
                "include_test_commands": {"type": "boolean", "default": true, "description": "Include run commands when known."},
                "strict": {"type": "boolean", "default": false, "description": "Also treat minor findings as blocking."},
            }),
            &[],
        ),
    ]
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ForTaskArgs {
    task: String,
    budget_tokens: Option<u32>,
    #[serde(default)]
    mode: Mode,
    #[serde(default = "yes")]
    include_docs: bool,
    #[serde(default = "yes")]
    include_bodies: bool,
    #[serde(default)]
    include_seen: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AfterEditArgs {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    symbols: Vec<String>,
    budget_tokens: Option<u32>,
    #[serde(default)]
    include_seen: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BeforeFinishArgs {
    budget_tokens: Option<u32>,
    #[serde(default = "yes")]
    include_test_commands: bool,
    #[serde(default)]
    strict: bool,
}

fn parse<T: DeserializeOwned>(params: &CallToolRequestParams) -> Result<T, BrokerError> {
    let args = Value::Object(params.arguments.clone().unwrap_or_default());
    serde_json::from_value(args).map_err(|e| BrokerError {
        error: "invalid_input",
        message: e.to_string(),
    })
}

fn tool_result(result: Result<Value, BrokerError>) -> CallToolResult {
    let (structured, is_error) = match result {
        Ok(v) => (v, false),
        Err(e) => (serde_json::to_value(&e).unwrap_or(Value::Null), true),
    };
    let text = structured.to_string();
    let mut res = CallToolResult::text_content(vec![text.into()]);
    res.structured_content = Some(structured);
    res.is_error = Some(is_error);
    res
}

impl BrokerServer {
    async fn dispatch(&self, params: &CallToolRequestParams) -> Result<Value, BrokerError> {
        let to_value = |env| {
            serde_json::to_value(env).map_err(|e| BrokerError {
                error: "internal",
                message: e.to_string(),
            })
        };
        match params.name.as_str() {
            "context_for_task" => {
                let a: ForTaskArgs = parse(params)?;
                let mut req = TaskRequest::new(&a.task);
                req.budget_tokens = a.budget_tokens.unwrap_or(req.budget_tokens);
                req.mode = a.mode;
                req.include_docs = a.include_docs;
                req.include_bodies = a.include_bodies;
                req.include_seen = a.include_seen;
                to_value(self.broker().await?.context_for_task(req).await?)
            }
            "context_after_edit" => {
                let a: AfterEditArgs = parse(params)?;
                let mut req = EditRequest {
                    files: a.files,
                    symbols: a.symbols,
                    include_seen: a.include_seen,
                    ..EditRequest::default()
                };
                req.budget_tokens = a.budget_tokens.unwrap_or(req.budget_tokens);
                to_value(self.broker().await?.context_after_edit(req).await?)
            }
            "context_before_finish" => {
                let a: BeforeFinishArgs = parse(params)?;
                let mut req = FinishRequest {
                    include_test_commands: a.include_test_commands,
                    strict: a.strict,
                    ..FinishRequest::default()
                };
                req.budget_tokens = a.budget_tokens.unwrap_or(req.budget_tokens);
                to_value(self.broker().await?.context_before_finish(req).await?)
            }
            other => Err(BrokerError {
                error: "invalid_input",
                message: format!("unknown tool '{other}'"),
            }),
        }
    }
}

#[async_trait]
impl ServerHandler for BrokerServer {
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListToolsResult, RpcError> {
        serde_json::from_value(json!({"tools": tools(self.settings.broker.online.is_some()), "cacheScope": "private", "ttlMs": 0, "resultType": "complete"}))
            .map_err(|e| RpcError::internal_error().with_message(e.to_string()))
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ServerResult, CallToolError> {
        let Some((id, cancelled)) = self.inflight.start(&params) else {
            return Ok(tool_result(self.dispatch(&params).await).into());
        };
        // Dropping `dispatch` on cancellation stops the rest of its upstream work; the
        // broker records the call as `cancelled` (RF-14).
        let result = tokio::select! {
            r = self.dispatch(&params) => r,
            _ = cancelled.notified() => Err(BrokerError {
                error: "cancelled",
                message: "cancelled by the client".into(),
            }),
        };
        self.inflight.finish(&id);
        Ok(tool_result(result).into())
    }

    async fn handle_cancelled_notification(
        &self,
        params: CancelledNotificationParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<(), RpcError> {
        if let Ok(id) = serde_json::to_value(&params.request_id) {
            self.inflight.cancel(id.to_string());
        }
        Ok(())
    }

    async fn handle_list_resources_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListResourcesResult, RpcError> {
        serde_json::from_value(json!({
            "resources": [{
                "uri": STATUS_URI,
                "name": "status",
                "title": "ripwire-broker status",
                "description": "Operational status: versions, upstream availability, restarts, budgets and local metrics. Never contains prompts, code or responses.",
                "mimeType": "application/json",
            }],
            "cacheScope": "private", "ttlMs": 0, "resultType": "complete",
        }))
        .map_err(|e| RpcError::internal_error().with_message(e.to_string()))
    }

    async fn handle_read_resource_request(
        &self,
        params: ReadResourceRequestParams,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ServerResult, RpcError> {
        if params.uri != STATUS_URI {
            return Err(
                RpcError::invalid_params().with_message(format!("unknown resource {}", params.uri))
            );
        }
        let text = self.status_json().await.to_string();
        let result: rust_mcp_sdk::schema::ReadResourceResult = serde_json::from_value(json!({
            "contents": [{"uri": STATUS_URI, "mimeType": "application/json", "text": text}],
            "cacheScope": "private", "ttlMs": 0, "resultType": "complete",
        }))
        .map_err(|e| RpcError::internal_error().with_message(e.to_string()))?;
        Ok(result.into())
    }
}
