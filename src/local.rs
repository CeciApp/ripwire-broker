//! One-shot commands (`hook`, `prompt`, `doctor`): start ripwire and a broker in-process.

use crate::broker::{Broker, BrokerConfig, BrokerError, TaskRequest};
use crate::cli::{PromptArgs, UpstreamArgs};
use crate::upstream::{RipwireUpstream, UpstreamConfig, ripwire_version};
use std::path::Path;
use std::sync::Arc;

/// A broker over a fresh ripwire process for `workspace`. `version` is a reading the caller
/// already has for this exact binary; without one, a `ripwire --version` process is started to
/// get it (D-105).
pub async fn launch(
    workspace: &Path,
    upstream: &UpstreamArgs,
    incremental: bool,
    version: Option<String>,
) -> Result<Broker, BrokerError> {
    let workspace = workspace.canonicalize().map_err(|e| BrokerError {
        error: "workspace_violation",
        message: format!("workspace {}: {e}", workspace.display()),
    })?;
    let mut up = UpstreamConfig::new(&workspace);
    up.binary = upstream.ripwire.clone();
    up.timeout = upstream.timeout;
    let mut config = BrokerConfig::new(&workspace);
    config.incremental = incremental;
    config.ripwire_version = version.unwrap_or_else(|| ripwire_version(&up.binary));
    let process = RipwireUpstream::spawn(up).await?;
    Broker::connect(Arc::new(process), config).await
}

pub const CONTEXT_OPEN: &str = "<ripwire-broker-context untrusted=\"true\">";
pub const CONTEXT_CLOSE: &str = "</ripwire-broker-context>";

/// The wrapper for clients without hooks (PRD 8.4 level 3): the task, a blank line, then the
/// context as delimited untrusted data. On failure, the task alone and the reason.
pub async fn prompt(args: &PromptArgs) -> (String, Option<BrokerError>) {
    let context = async {
        let broker = launch(&args.workspace, &args.upstream, false, None).await?;
        let mut req = TaskRequest::new(&args.task);
        if let Some(b) = args.budget_tokens {
            req.budget_tokens = b;
        }
        broker.context_for_task(req).await
    };
    match context.await {
        Ok(env) => (
            format!(
                "{}\n\n{CONTEXT_OPEN}\n{}\n{CONTEXT_CLOSE}\n",
                args.task,
                // `<` and `>` only occur inside JSON strings; as \u escapes the payload can
                // never contain the closing tag of its own block (prompt injection, D-052).
                serde_json::to_string(&env)
                    .unwrap_or_default()
                    .replace('<', "\\u003c")
                    .replace('>', "\\u003e")
            ),
            None,
        ),
        Err(e) => (format!("{}\n", args.task), Some(e)),
    }
}
