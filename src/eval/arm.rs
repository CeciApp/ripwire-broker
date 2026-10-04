//! The arms of the A/B (PRD §16.2 and §23.15) and of the memory evaluation (PRD jev-mem §14,
//! D-141): what MCP server, if any, the agent gets.

use super::transcript::Summary;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    /// The agent alone.
    None,
    /// The agent with `ripwire --mcp`, no broker.
    Ripwire,
    /// The agent with `ripwire-broker serve`.
    Broker,
    /// The agent with `ripwire-broker serve --online`: arm A of the memory evaluation.
    BrokerOnline,
    /// Arm B: `serve --memory`, memory controlled by the classifier.
    BrokerMemory,
    /// Arm C: `serve --memory --memory-selection deterministic`, the same collection and store
    /// with the local ranking and no classifier for memory.
    BrokerMemoryDeterministic,
}

/// The binaries an arm starts.
#[derive(Debug, Clone)]
pub struct Tools {
    pub broker: PathBuf,
    pub ripwire: PathBuf,
}

pub const ALL: [Arm; 6] = [
    Arm::None,
    Arm::Ripwire,
    Arm::Broker,
    Arm::BrokerOnline,
    Arm::BrokerMemory,
    Arm::BrokerMemoryDeterministic,
];

impl Arm {
    pub fn name(self) -> &'static str {
        match self {
            Arm::None => "none",
            Arm::Ripwire => "ripwire",
            Arm::Broker => "broker",
            Arm::BrokerOnline => "broker-online",
            Arm::BrokerMemory => "broker-memory",
            Arm::BrokerMemoryDeterministic => "broker-memory-deterministic",
        }
    }

    /// Whether its server sends to the provider (`--memory` implies `--online`), and so needs
    /// the credential and the consent of PRD §23.6.
    pub fn needs_credential(self) -> bool {
        matches!(
            self,
            Arm::BrokerOnline | Arm::BrokerMemory | Arm::BrokerMemoryDeterministic
        )
    }

    pub fn parse(s: &str) -> Option<Arm> {
        ALL.into_iter().find(|a| a.name() == s)
    }

    /// The one MCP server this arm adds, by the name its tools carry (`mcp__NAME__tool`).
    pub fn server(self) -> Option<&'static str> {
        match self {
            Arm::None => None,
            Arm::Ripwire => Some("ripwire"),
            Arm::Broker
            | Arm::BrokerOnline
            | Arm::BrokerMemory
            | Arm::BrokerMemoryDeterministic => Some("ripwire-broker"),
        }
    }

    /// The agent's MCP configuration, in the `mcpServers` shape Claude Code reads. Arguments
    /// are an array, never a shell line. A memory arm's server keeps its store under `state`
    /// (`XDG_STATE_HOME`), so that every round starts from its own.
    pub fn mcp_config(self, tools: &Tools, workdir: &Path, state: &Path) -> Value {
        let ws = workdir.to_string_lossy();
        let rw = tools.ripwire.to_string_lossy();
        let broker = |extra: &[&str]| {
            let mut args = vec![
                json!("serve"),
                json!("--workspace"),
                json!(ws),
                json!("--ripwire"),
                json!(rw),
            ];
            args.extend(extra.iter().map(|a| json!(a)));
            let mut server = json!({"command": tools.broker.to_string_lossy(), "args": args});
            if extra.contains(&"--memory") {
                server["env"] = json!({"XDG_STATE_HOME": state.to_string_lossy()});
            }
            server
        };
        let servers = match self {
            Arm::None => json!({}),
            Arm::Ripwire => json!({"ripwire": {"command": rw, "args": [ws, "--mcp"]}}),
            Arm::Broker => json!({"ripwire-broker": broker(&[])}),
            Arm::BrokerOnline => json!({"ripwire-broker": broker(&["--online"])}),
            Arm::BrokerMemory => json!({"ripwire-broker": broker(&["--memory"])}),
            Arm::BrokerMemoryDeterministic => json!({"ripwire-broker": broker(&[
                "--memory",
                "--memory-selection",
                "deterministic"
            ])}),
        };
        json!({"mcpServers": servers})
    }

    /// Why a run cannot count for this arm: a hook that ran (it injects into every arm), a context
    /// tool run from the shell that the arm does not include, an MCP
    /// server the arm did not declare (the user's global configuration leaking in), or the arm's
    /// own server not connected (the arm would silently become `none`).
    pub fn contamination(self, s: &Summary) -> Option<String> {
        if !s.hooks.is_empty() {
            return Some(format!(
                "hook(s) ran in the session: {}",
                s.hooks.join(", ")
            ));
        }
        let foreign_cli: Vec<&str> = s
            .shell_tools
            .iter()
            .map(String::as_str)
            .filter(|t| !(self == Arm::Ripwire && *t == "ripwire"))
            .collect();
        if !foreign_cli.is_empty() {
            return Some(format!(
                "context tool(s) run from the shell outside arm {}: {}",
                self.name(),
                foreign_cli.join(", ")
            ));
        }
        let mine = self.server();
        let mut foreign: Vec<&str> = s
            .mcp_servers
            .iter()
            .map(String::as_str)
            .chain(
                s.mcp_tools
                    .iter()
                    .filter_map(|t| t.strip_prefix("mcp__")?.split("__").next()),
            )
            .filter(|name| Some(*name) != mine)
            .collect();
        foreign.sort_unstable();
        foreign.dedup();
        if !foreign.is_empty() {
            return Some(format!(
                "MCP server(s) {} not declared by arm {}",
                foreign.join(", "),
                self.name()
            ));
        }
        let mine = mine?;
        match s.mcp_status.get(mine).map(String::as_str) {
            Some("connected") => None,
            other => Some(format!(
                "the arm's server {mine} not connected ({})",
                other.unwrap_or("absent")
            )),
        }
    }
}
