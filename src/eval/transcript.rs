//! An agent transcript (Claude Code `--output-format stream-json`) reduced to counts. Each line
//! is stored with the runner's clock, `{"at_ms": N, "event": {...}}`, because the stream itself
//! carries no per-event time.
//!
//! A transcript that lacks the `init` or the `result` event is **invalid**, never zero: a parser
//! that read nothing must not pass for an agent that spent nothing.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

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
    /// The agent's version and model as its session announced them (`claude_code_version`,
    /// `model` in the init event).
    pub agent: Option<String>,
    /// `context_for_task` calls, and the time from each call to its answer (structure and
    /// memory read together, as the agent waited for them).
    pub context_calls: u64,
    pub context_ms: u64,
    /// What the broker's memory reads reported in `provenance.memory`, summed, and the memories
    /// it delivered. `None` when no answer carried a memory read.
    pub memory_read: Option<MemoryRead>,
    /// Files the broker presented, in the order it first presented them.
    pub presented_files: Vec<String>,
    pub presented_tests: Vec<String>,
    /// The agent's shell commands, for telling which reference tests it ran. Scoring only;
    /// never written to the results.
    pub commands: Vec<String>,
}

/// The memory reads of a session (PRD jev-mem §14: retrieval apart from ingestion).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct MemoryRead {
    pub reads: u64,
    pub requests: u64,
    pub questions: u64,
    pub delivered: u64,
}

impl Summary {
    pub fn valid(&self) -> bool {
        self.invalid.is_none()
    }
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

/// A shell line split into commands (on `|`, `;`, `&`, newlines and parentheses outside quotes),
/// each split into words with the quotes removed. Enough shell to find the program a command runs,
/// not a shell: a `$(...)` inside double quotes is not looked into.
fn commands(line: &str) -> Vec<Vec<String>> {
    let (mut all, mut words, mut word) = (vec![], vec![], String::new());
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = line.chars();
    let flush = |word: &mut String, in_word: &mut bool, words: &mut Vec<String>| {
        if *in_word {
            words.push(std::mem::take(word));
            *in_word = false;
        }
    };
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => word.extend(chars.next()),
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                in_word = true;
            }
            (None, '\\') => {
                word.extend(chars.next());
                in_word = true;
            }
            (None, '|' | ';' | '&' | '\n' | '(' | ')' | '`') => {
                flush(&mut word, &mut in_word, &mut words);
                if !words.is_empty() {
                    all.push(std::mem::take(&mut words));
                }
            }
            (None, c) if c.is_whitespace() => flush(&mut word, &mut in_word, &mut words),
            (None, c) => {
                word.push(c);
                in_word = true;
            }
        }
    }
    flush(&mut word, &mut in_word, &mut words);
    if !words.is_empty() {
        all.push(words);
    }
    all
}

/// `NAME=value`, a variable set for the command that follows.
fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        name.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// Commands that run the next word as the program.
const WRAPPERS: &[&str] = &[
    "env", "command", "exec", "nohup", "time", "nice", "sudo", "builtin", "timeout", "stdbuf",
    "xargs",
];
const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash"];
/// Shell words that open a command without being its program.
const KEYWORDS: &[&str] = &[
    "if", "then", "else", "elif", "while", "until", "do", "{", "!",
];

/// A wrapper's options that take the next word as their value (`nice -n 5`).
fn takes_value(wrapper: &str, option: &str) -> bool {
    let options: &[&str] = match wrapper {
        "env" => &["-u", "-C", "--unset", "--chdir"],
        "nice" => &["-n", "--adjustment"],
        "sudo" => &[
            "-u", "-g", "-C", "-D", "-h", "-p", "-r", "-t", "-U", "--user", "--group",
        ],
        "timeout" => &["-s", "-k", "--signal", "--kill-after"],
        "stdbuf" => &["-i", "-o", "-e"],
        "xargs" => &["-n", "-I", "-L", "-P", "-s", "-d", "-E", "-a"],
        "time" => &["-f", "-o"],
        "exec" => &["-a"],
        _ => &[],
    };
    options.contains(&option)
}

/// `-c`, alone or among other short flags (`bash -lc`).
fn runs_script(word: &str) -> bool {
    word.strip_prefix('-')
        .is_some_and(|flags| !flags.starts_with('-') && flags.contains('c'))
}

/// The program one command runs, past assignments and wrappers, and into `sh -c '...'`.
fn executables(words: &[String]) -> Vec<String> {
    let mut rest = words;
    while let Some((first, tail)) = rest.split_first() {
        if is_assignment(first) || KEYWORDS.contains(&first.as_str()) {
            rest = tail;
            continue;
        }
        let name = first.rsplit('/').next().unwrap_or(first);
        if WRAPPERS.contains(&name) {
            rest = tail;
            while let Some((w, t)) = rest.split_first() {
                if takes_value(name, w) {
                    rest = t.get(1..).unwrap_or_default();
                } else if w.starts_with('-') || is_assignment(w) {
                    rest = t;
                } else {
                    break;
                }
            }
            // `timeout 30s cmd`: the duration comes before the program.
            if name == "timeout" {
                rest = rest.get(1..).unwrap_or_default();
            }
            continue;
        }
        if SHELLS.contains(&name)
            && let Some(script) = tail
                .iter()
                .position(|w| runs_script(w))
                .and_then(|p| tail.get(p + 1))
        {
            return programs(script);
        }
        // `find -exec cmd {} ;` runs cmd as well as searching.
        if name == "find" {
            let exec = ["-exec", "-execdir", "-ok", "-okdir"];
            let mut found = vec![name.to_string()];
            if let Some(p) = tail.iter().position(|w| exec.contains(&w.as_str())) {
                found.extend(executables(&tail[p + 1..]));
            }
            return found;
        }
        // `git grep` and `git ls-files` search; other git subcommands do not.
        if name == "git" {
            return tail
                .first()
                .map(|w| format!("git {w}"))
                .into_iter()
                .collect();
        }
        return vec![name.to_string()];
    }
    vec![]
}

/// The programs a shell line runs, one per command.
fn programs(command: &str) -> Vec<String> {
    commands(command)
        .iter()
        .flat_map(|c| executables(c))
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
    let announced: Vec<&str> = ["claude_code_version", "model"]
        .iter()
        .filter_map(|k| e[*k].as_str())
        .collect();
    if !announced.is_empty() {
        s.agent = Some(announced.join(" "));
    }
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

/// One tool call: counted by class, its name and time kept for the result that answers it.
fn on_tool_use(
    s: &mut Summary,
    names: &mut HashMap<String, (String, u64)>,
    at: u64,
    block: &Value,
) {
    let name = block["name"].as_str().unwrap_or("");
    if let Some(id) = block["id"].as_str() {
        names.insert(id.to_string(), (name.to_string(), at));
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

/// The answer to an MCP call, `waited` after it: its size, what a broker envelope presented,
/// and its memory read.
fn on_mcp_result(s: &mut Summary, name: &str, text: &str, waited: u64) {
    s.mcp_result_bytes += text.len() as u64;
    if !BROKER_TOOLS
        .iter()
        .any(|t| name.ends_with(&format!("__{t}")))
    {
        return;
    }
    if name.ends_with("__context_for_task") {
        s.context_calls += 1;
        s.context_ms += waited;
    }
    // The envelope's JSON comes first; with memories, the readable section follows it.
    let Some(Ok(env)) = serde_json::Deserializer::from_str(text)
        .into_iter::<Value>()
        .next()
    else {
        return;
    };
    let read = &env["provenance"]["memory"];
    if read.is_object() {
        let m = s.memory_read.get_or_insert_with(MemoryRead::default);
        m.reads += 1;
        m.requests += read["requests"].as_u64().unwrap_or(0);
        m.questions += read["questions"].as_u64().unwrap_or(0);
        m.delivered += env["memories"].as_array().map_or(0, |a| a.len() as u64);
    }
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
    // Claude Code ends a session its API failed with `terminal_reason: "api_error"`: the run says
    // nothing about the arm.
    let mut api_error = None;
    let mut names: HashMap<String, (String, u64)> = HashMap::new();
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
                    if let Some((name, called)) = names.get(id).filter(|n| n.0.starts_with("mcp__"))
                    {
                        let waited = at.saturating_sub(*called);
                        on_mcp_result(&mut s, name, &result_text(&block["content"]), waited);
                    }
                }
            }
            Some("result") => {
                saw_result = true;
                on_result(&mut s, e);
                if e["terminal_reason"] == "api_error" {
                    let said: String = e["result"]
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .take(200)
                        .collect();
                    api_error = Some(format!("the agent's API failed: {said}"));
                }
            }
            _ => {}
        }
    }
    s.invalid = match (saw_init, saw_result) {
        (true, true) => api_error,
        (false, _) => Some("no init event: the agent did not start a session".into()),
        (true, false) => Some("no result event: the agent did not finish".into()),
    };
    s
}
