//! The arms of the A/B (PRD §16.2 and §23.15): what MCP server, if any, the agent gets.

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
    /// The agent with `ripwire-broker serve --online`.
    BrokerOnline,
}

/// The binaries an arm starts.
#[derive(Debug, Clone)]
pub struct Tools {
    pub broker: PathBuf,
    pub ripwire: PathBuf,
}

pub const ALL: [Arm; 4] = [Arm::None, Arm::Ripwire, Arm::Broker, Arm::BrokerOnline];

impl Arm {
    pub fn name(self) -> &'static str {
        match self {
            Arm::None => "none",
            Arm::Ripwire => "ripwire",
            Arm::Broker => "broker",
            Arm::BrokerOnline => "broker-online",
        }
    }

    pub fn parse(s: &str) -> Option<Arm> {
        ALL.into_iter().find(|a| a.name() == s)
    }

    /// The one MCP server this arm adds, by the name its tools carry (`mcp__NAME__tool`).
    pub fn server(self) -> Option<&'static str> {
        match self {
            Arm::None => None,
            Arm::Ripwire => Some("ripwire"),
            Arm::Broker | Arm::BrokerOnline => Some("ripwire-broker"),
        }
    }

    /// The agent's MCP configuration, in the `mcpServers` shape Claude Code reads. Arguments
    /// are an array, never a shell line.
    pub fn mcp_config(self, tools: &Tools, workdir: &Path) -> Value {
        let ws = workdir.to_string_lossy();
        let rw = tools.ripwire.to_string_lossy();
        let broker = |online: bool| {
            let mut args = vec![
                json!("serve"),
                json!("--workspace"),
                json!(ws),
                json!("--ripwire"),
                json!(rw),
            ];
            if online {
                args.push(json!("--online"));
            }
            json!({"command": tools.broker.to_string_lossy(), "args": args})
        };
        let servers = match self {
            Arm::None => json!({}),
            Arm::Ripwire => json!({"ripwire": {"command": rw, "args": [ws, "--mcp"]}}),
            Arm::Broker => json!({"ripwire-broker": broker(false)}),
            Arm::BrokerOnline => json!({"ripwire-broker": broker(true)}),
        };
        json!({"mcpServers": servers})
    }

    /// Why a run cannot count for this arm: an MCP server the arm did not declare (the user's
    /// global configuration leaking in), or the arm's own server not connected (the arm would
    /// silently become `none`).
    pub fn contamination(self, s: &Summary) -> Option<String> {
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
