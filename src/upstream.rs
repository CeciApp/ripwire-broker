//! Upstream MCP client: owns the `ripwire <workspace> --mcp` child process.

use async_trait::async_trait;
use rust_mcp_sdk::mcp_client::{ClientHandler, ClientRuntime, McpClientOptions, client_runtime};
use rust_mcp_sdk::schema::{
    CallToolRequestParams, ClientCapabilities, ContentBlock, Implementation,
    PaginatedRequestParams, ServerResult, schema_utils::RequestFromClient,
};
use rust_mcp_sdk::{
    ClientDetails, McpClient, StdioTransport, ToMcpClientHandler, TransportOptions,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Debug, Clone, PartialEq)]
pub enum UpstreamError {
    /// The process could not be started or died and could not be restarted.
    Unavailable(String),
    /// Ripwire refused the request (bad argument, unknown symbol...). Never retried.
    Refused(String),
    Timeout,
}

impl std::fmt::Display for UpstreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(m) => write!(f, "ripwire unavailable: {m}"),
            Self::Refused(m) => write!(f, "ripwire refused the request: {m}"),
            Self::Timeout => write!(f, "ripwire did not answer before the timeout"),
        }
    }
}

/// What the broker needs from Ripwire. Real process in production, fixtures in tests.
#[async_trait]
pub trait Upstream: Send + Sync {
    async fn list_tools(&self) -> Result<Vec<String>, UpstreamError>;
    /// Calls a read-only verb and returns its text payload.
    async fn call(&self, tool: &str, args: Value) -> Result<String, UpstreamError>;
    /// Child process replacements so far (0 for in-process fakes).
    fn restarts(&self) -> u32 {
        0
    }
}

#[derive(Debug, Clone)]
pub struct UpstreamConfig {
    pub binary: PathBuf,
    pub workspace: PathBuf,
    /// `(broker binary, MiB)`: start ripwire through `__supervise` with that memory limit.
    pub supervisor: Option<(PathBuf, u64)>,
    pub timeout: Duration,
}

impl UpstreamConfig {
    pub fn new(workspace: &Path) -> Self {
        Self {
            binary: PathBuf::from("ripwire"),
            workspace: workspace.to_path_buf(),
            timeout: Duration::from_secs(60),
            supervisor: None,
        }
    }
}

struct NoopHandler;
impl ClientHandler for NoopHandler {}

pub struct RipwireUpstream {
    config: UpstreamConfig,
    client: Mutex<Arc<ClientRuntime>>,
    restarts: AtomicU32,
}

impl RipwireUpstream {
    pub async fn spawn(config: UpstreamConfig) -> Result<Self, UpstreamError> {
        let client = launch(&config).await?;
        Ok(Self {
            config,
            client: Mutex::new(client),
            restarts: AtomicU32::new(0),
        })
    }

    async fn current(&self) -> Arc<ClientRuntime> {
        self.client.lock().await.clone()
    }

    /// Kills `dead` and launches a fresh process, unless another caller already did.
    async fn restart(&self, dead: &Arc<ClientRuntime>) -> Result<(), UpstreamError> {
        let mut slot = self.client.lock().await;
        if !Arc::ptr_eq(&slot, dead) {
            return Ok(());
        }
        let _ = dead.shut_down().await;
        *slot = launch(&self.config).await?;
        self.restarts.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn call_once(
        &self,
        client: &Arc<ClientRuntime>,
        params: CallToolRequestParams,
    ) -> Result<String, UpstreamError> {
        match tokio::time::timeout(self.config.timeout, client.call_tool(params)).await {
            Err(_) => Err(UpstreamError::Timeout),
            Ok(Err(e)) => Err(classify(e)),
            Ok(Ok(res)) => Ok(text_of(&res.content)),
        }
    }
}

async fn launch(config: &UpstreamConfig) -> Result<Arc<ClientRuntime>, UpstreamError> {
    // Arguments are passed as an array, never through a shell (PRD 14.3 / 15.3).
    let mut program = config.binary.to_string_lossy().into_owned();
    let mut args = vec![
        config.workspace.to_string_lossy().into_owned(),
        "--mcp".into(),
    ];
    if let Some((broker, mb)) = &config.supervisor {
        args.splice(
            0..0,
            [
                "__supervise".into(),
                "--max-rss-mb".into(),
                mb.to_string(),
                "--".into(),
                program,
            ],
        );
        program = broker.to_string_lossy().into_owned();
    }
    // Our own timeout (in `call_once`) is authoritative; the transport's is only a backstop.
    let options = TransportOptions {
        timeout: config.timeout * 2,
        ..Default::default()
    };
    // The SDK only adds variables to the inherited environment: the key is overridden with an
    // empty value, which reads as unset, so ripwire never receives it (D-146).
    let env = std::collections::HashMap::from([(crate::online::KEY_VAR.into(), String::new())]);
    let transport = StdioTransport::create_with_server_launch(program, args, Some(env), options)
        .map_err(|e| UpstreamError::Unavailable(e.to_string()))?;
    let details = ClientDetails {
        client_info: Implementation {
            name: "ripwire-broker".into(),
            version: env!("CARGO_PKG_VERSION").into(),
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
        NoopHandler.to_mcp_client_handler(),
    ));
    client
        .clone()
        .start()
        .await
        .map_err(|e| UpstreamError::Unavailable(e.to_string()))?;
    Ok(client)
}

/// A JSON-RPC error means ripwire answered and said no; everything else means it is gone.
fn classify(err: rust_mcp_sdk::error::McpSdkError) -> UpstreamError {
    use rust_mcp_sdk::TransportError;
    use rust_mcp_sdk::error::McpSdkError;
    match err {
        McpSdkError::RpcError(e) | McpSdkError::Transport(TransportError::JsonrpcError(e)) => {
            UpstreamError::Refused(e.message)
        }
        other => UpstreamError::Unavailable(other.to_string()),
    }
}

fn text_of(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::TextContent(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[async_trait]
impl Upstream for RipwireUpstream {
    async fn list_tools(&self) -> Result<Vec<String>, UpstreamError> {
        // ripwire 0.6.4 answers with a 2025-06-18 ListToolsResult (no cacheScope/ttlMs), which the
        // 2026-07-28 typed conversion rejects; it then lands in the generic `Result` variant.
        let request = RequestFromClient::ListToolsRequest(PaginatedRequestParams::default());
        let client = self.current().await;
        let res = tokio::time::timeout(self.config.timeout, client.request(request, None))
            .await
            .map_err(|_| UpstreamError::Timeout)?
            .map_err(classify)?;
        let tools: Vec<Value> = match res {
            ServerResult::ListToolsResult(r) => {
                return Ok(r.tools.into_iter().map(|t| t.name).collect());
            }
            ServerResult::Result(generic) => generic
                .extra
                .and_then(|mut m| m.remove("tools"))
                .and_then(|t| t.as_array().cloned())
                .unwrap_or_default(),
            _ => vec![],
        };
        Ok(tools
            .iter()
            .filter_map(|t| t["name"].as_str().map(str::to_string))
            .collect())
    }

    fn restarts(&self) -> u32 {
        self.restarts.load(Ordering::Relaxed)
    }

    async fn call(&self, tool: &str, args: Value) -> Result<String, UpstreamError> {
        let arguments = match args {
            Value::Object(m) => Some(m),
            _ => None,
        };
        let params = CallToolRequestParams {
            name: tool.into(),
            arguments,
            input_responses: None,
            meta: Default::default(), // replaced by the runtime on send
            request_state: None,
        };
        let retry = params.clone();
        let client = self.current().await;
        match self.call_once(&client, params).await {
            // A hung process is replaced, but the slow call is not repeated.
            Err(UpstreamError::Timeout) => {
                self.restart(&client).await?;
                Err(UpstreamError::Timeout)
            }
            // One recoverable restart per call (CA-07); a second failure is reported.
            Err(UpstreamError::Unavailable(_)) => {
                self.restart(&client).await?;
                self.call_once(&self.current().await, retry).await
            }
            other => other,
        }
    }
}

/// How long `ripwire --version` may take: it runs before `serve` answers its host and in every
/// hook that asks ripwire, so a binary that hangs there must not hang them (D-146).
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// `binary` as an absolute path: taken as given when it names a directory component, looked up on
/// `PATH` otherwise. `None` when nothing executable answers to it.
pub fn resolve(binary: &Path) -> Option<PathBuf> {
    if binary.components().count() > 1 {
        return binary.canonicalize().ok();
    }
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join(binary))
            .find(|candidate| candidate.is_file())
            .and_then(|found| found.canonicalize().ok())
    })
}

/// What identifies the binary we last asked: its resolved path, size and modification time. Not a
/// content hash on purpose — hashing several megabytes would cost more than the process it saves
/// (D-105). Getting this wrong reports a stale version for the rest of a session, which weakens a
/// **compatibility** check (`check_version`), not the eligibility guard of `is_fresh`, where the
/// same shortcut was refused because it would be the consent boundary (D-104).
pub fn binary_stamp(binary: &Path) -> Option<(String, u64, u64)> {
    let path = resolve(binary)?;
    let meta = std::fs::metadata(&path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some((path.to_string_lossy().into_owned(), meta.len(), mtime))
}

/// `ripwire --version` → "0.6.4", or "unavailable", also after [`VERSION_TIMEOUT`]. Run with an
/// argument array, never a shell.
pub fn ripwire_version(binary: &std::path::Path) -> String {
    let mut command = std::process::Command::new(binary);
    command.arg("--version").env_remove(crate::online::KEY_VAR);
    let deadline = std::time::Instant::now() + VERSION_TIMEOUT;
    crate::bounded::output(command, deadline)
        .ok()
        .and_then(|(_, out)| String::from_utf8(out).ok())
        .and_then(|s| s.split_whitespace().nth(1).map(str::to_string))
        .unwrap_or_else(|| "unavailable".into())
}
