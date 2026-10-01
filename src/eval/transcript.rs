//! An agent transcript (Claude Code `--output-format stream-json`) reduced to counts. Each line
//! is stored with the runner's clock, `{"at_ms": N, "event": {...}}`, because the stream itself
//! carries no per-event time.
//!
//! A transcript that lacks the `init` or the `result` event is **invalid**, never zero: a parser
//! that read nothing must not pass for an agent that spent nothing.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Calls {
    pub search: u64,
    pub read: u64,
    pub edit: u64,
    pub mcp: u64,
    pub other: u64,
}

impl Calls {
    /// Searches and reads: the "chamadas exploratórias" of §17 and §23.15.
    pub fn exploratory(&self) -> u64 {
        self.search + self.read
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Tokens {
    pub input: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
    pub output: u64,
}

impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.cache_creation + self.cache_read + self.output
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    /// Why the transcript cannot be measured; `None` when it can.
    pub invalid: Option<String>,
    /// MCP servers the session started with, and whether each connected.
    pub mcp_servers: Vec<String>,
    pub mcp_status: HashMap<String, String>,
    /// MCP tools the session offered (`mcp__server__tool`).
    pub mcp_tools: Vec<String>,
    /// Hooks that ran in the session (`--include-hook-events`). A repository can commit hooks of
    /// its own, and they inject context into every arm.
    pub hooks: Vec<String>,
    /// Context tools the agent ran from the shell (`graft`, `ripwire`), which no MCP listing
    /// shows. A repository can ship another tool's index that tells the agent to run it.
    pub shell_tools: Vec<String>,
    pub calls: Calls,
    pub tokens: Tokens,
    pub cost_usd: f64,
    pub duration_ms: u64,
    /// The agent ended in error (turn limit, budget...): a failed task, still a measurement.
    pub is_error: bool,
    pub first_edit_ms: Option<u64>,
    pub mcp_result_bytes: u64,
    /// Files the broker presented, in the order it first presented them.
    pub presented_files: Vec<String>,
    pub presented_tests: Vec<String>,
    /// The agent's shell commands, for telling which reference tests it ran. Scoring only;
    /// never written to the results.
    pub commands: Vec<String>,
}

impl Summary {
    pub fn valid(&self) -> bool {
        self.invalid.is_none()
    }
}

/// Reads a transcript the runner wrote. Lines that do not parse are skipped.
pub fn read(path: &Path) -> Vec<(u64, Value)> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|v| (v["at_ms"].as_u64().unwrap_or(0), v["event"].clone()))
        .collect()
}

const SEARCH: &[&str] = &[
    "grep", "rg", "ag", "ack", "find", "fd", "ls", "tree", "locate",
];
const READ: &[&str] = &[
    "cat", "head", "tail", "sed", "less", "more", "bat", "nl", "awk", "wc",
];
/// Context tools an agent could run from the shell. `ripwire-broker` is not one: in its own
/// repository, running the binary under development is the work.
pub const SHELL_TOOLS: &[&str] = &["graft", "ripwire"];

const BROKER_TOOLS: &[&str] = &[
    "context_for_task",
    "context_after_edit",
    "context_before_finish",
];

enum Class {
    Search,
    Read,
    Edit,
    Mcp,
    Other,
}

/// The program each segment of a shell line runs: split on `&&`, `||`, `;` and `|`.
fn programs(command: &str) -> Vec<String> {
    command
        .split(['|', ';', '&'])
        .filter_map(|seg| {
            let mut words = seg.split_whitespace();
            let first = words.next()?;
            // `git grep` and `git ls-files` search; other git subcommands do not.
            if first == "git" {
                return words.next().map(|w| format!("git {w}"));
            }
            Some(first.rsplit('/').next().unwrap_or(first).to_string())
        })
        .collect()
}

fn bash_class(command: &str) -> Class {
    let progs = programs(command);
    let any = |set: &[&str]| progs.iter().any(|p| set.contains(&p.as_str()));
    if any(SEARCH) || progs.iter().any(|p| p == "git grep" || p == "git ls-files") {
        Class::Search
    } else if any(READ) {
        Class::Read
    } else {
        Class::Other
    }
}

fn classify(name: &str, input: &Value) -> Class {
    match name {
        "Grep" | "Glob" | "LS" => Class::Search,
        "Read" | "NotebookRead" => Class::Read,
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => Class::Edit,
        "Bash" => bash_class(input["command"].as_str().unwrap_or("")),
        n if n.starts_with("mcp__") => Class::Mcp,
        _ => Class::Other,
    }
}

/// A tool result's text: a string, or the text blocks of an array.
fn result_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn push_new(list: &mut Vec<String>, value: &str) {
    if !list.iter().any(|v| v == value) {
        list.push(value.to_string());
    }
}

fn on_init(s: &mut Summary, e: &Value) {
    for server in e["mcp_servers"].as_array().into_iter().flatten() {
        if let Some(name) = server["name"].as_str() {
            push_new(&mut s.mcp_servers, name);
            let status = server["status"].as_str().unwrap_or("").to_string();
            s.mcp_status.insert(name.to_string(), status);
        }
    }
    for tool in e["tools"].as_array().into_iter().flatten() {
        if let Some(t) = tool.as_str().filter(|t| t.starts_with("mcp__")) {
            push_new(&mut s.mcp_tools, t);
        }
    }
}

/// One tool call: counted by class, its name kept for the result that answers it.
fn on_tool_use(s: &mut Summary, names: &mut HashMap<String, String>, at: u64, block: &Value) {
    let name = block["name"].as_str().unwrap_or("");
    if let Some(id) = block["id"].as_str() {
        names.insert(id.to_string(), name.to_string());
    }
    match classify(name, &block["input"]) {
        Class::Search => s.calls.search += 1,
        Class::Read => s.calls.read += 1,
        Class::Edit => {
            s.calls.edit += 1;
            s.first_edit_ms.get_or_insert(at);
        }
        Class::Mcp => s.calls.mcp += 1,
        Class::Other => s.calls.other += 1,
    }
    if name == "Bash"
        && let Some(c) = block["input"]["command"].as_str()
    {
        for p in programs(c) {
            if SHELL_TOOLS.contains(&p.as_str()) {
                push_new(&mut s.shell_tools, &p);
            }
        }
        s.commands.push(c.to_string());
    }
}

/// The answer to an MCP call: its size, and what a broker envelope presented.
fn on_mcp_result(s: &mut Summary, name: &str, text: &str) {
    s.mcp_result_bytes += text.len() as u64;
    if !BROKER_TOOLS
        .iter()
        .any(|t| name.ends_with(&format!("__{t}")))
    {
        return;
    }
    let Ok(env) = serde_json::from_str::<Value>(text) else {
        return;
    };
    for (key, list) in [
        ("items", &mut s.presented_files),
        ("tests", &mut s.presented_tests),
    ] {
        for entry in env[key].as_array().into_iter().flatten() {
            if let Some(p) = entry["path"].as_str() {
                push_new(list, p);
            }
        }
    }
}

fn on_result(s: &mut Summary, e: &Value) {
    let u = &e["usage"];
    let n = |k: &str| u[k].as_u64().unwrap_or(0);
    s.tokens = Tokens {
        input: n("input_tokens"),
        cache_creation: n("cache_creation_input_tokens"),
        cache_read: n("cache_read_input_tokens"),
        output: n("output_tokens"),
    };
    s.cost_usd = e["total_cost_usd"].as_f64().unwrap_or(0.0);
    s.duration_ms = e["duration_ms"].as_u64().unwrap_or(0);
    s.is_error = e["is_error"].as_bool().unwrap_or(false);
}

/// The content blocks of an `assistant` or `user` message that have type `kind`.
fn blocks<'a>(e: &'a Value, kind: &'a str) -> impl Iterator<Item = &'a Value> {
    e["message"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(move |b| b["type"] == kind)
}

pub fn summarize(events: &[(u64, Value)]) -> Summary {
    let mut s = Summary::default();
    let (mut saw_init, mut saw_result) = (false, false);
    let mut names: HashMap<String, String> = HashMap::new();
    for (at, e) in events {
        match e["type"].as_str() {
            Some("system") if e["subtype"] == "init" => {
                saw_init = true;
                on_init(&mut s, e);
            }
            Some("system") if e["subtype"].as_str().is_some_and(|t| t.contains("hook")) => {
                let name = ["hook_name", "hook_event", "subtype"]
                    .iter()
                    .find_map(|k| e[*k].as_str())
                    .unwrap_or("hook");
                push_new(&mut s.hooks, name);
            }
            Some("assistant") => {
                for block in blocks(e, "tool_use") {
                    on_tool_use(&mut s, &mut names, *at, block);
                }
            }
            Some("user") => {
                for block in blocks(e, "tool_result") {
                    let id = block["tool_use_id"].as_str().unwrap_or("");
                    if let Some(name) = names.get(id).filter(|n| n.starts_with("mcp__")) {
                        on_mcp_result(&mut s, name, &result_text(&block["content"]));
                    }
                }
            }
            Some("result") => {
                saw_result = true;
                on_result(&mut s, e);
            }
            _ => {}
        }
    }
    s.invalid = match (saw_init, saw_result) {
        (true, true) => None,
        (false, _) => Some("no init event: the agent did not start a session".into()),
        (true, false) => Some("no result event: the agent did not finish".into()),
    };
    s
}
