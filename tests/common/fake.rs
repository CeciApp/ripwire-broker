//! A scripted `Upstream` that answers with payloads recorded from a real ripwire.
use async_trait::async_trait;
use ripwire_broker::upstream::{Upstream, UpstreamError};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

/// Every verb ripwire 0.6.4 lists, including the ones the broker must never expose.
pub const RIPWIRE_TOOLS: &[&str] = &[
    "analyze",
    "rank_by",
    "find_symbol",
    "find_referencing_symbols",
    "grep",
    "cochange",
    "memory_recall",
    "situational_awareness",
    "mentions",
    "for",
    "lego",
    "owners",
    "replace_symbol_body",
    "insert_before_symbol",
    "insert_after_symbol",
    "fetch_body",
    "exemplar",
    "quality_delta",
    "quality_baseline",
    "impact",
    "uses",
    "affected",
    "path_between",
    "connect",
    "explore",
    "from_trace",
    "edit_check",
    "whereis",
    "stray_content",
    "flags",
    "doc_drift",
    "slice",
    "batch",
];

pub fn fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/ripwire/{name}.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[derive(Default)]
pub struct FakeUpstream {
    tools: Vec<String>,
    down: bool,
    answers: HashMap<String, Result<String, UpstreamError>>,
    /// Answers consumed in order before falling back to `answers`.
    sequences: Mutex<HashMap<String, Vec<String>>>,
    calls: Mutex<Vec<(String, Value)>>,
}

impl FakeUpstream {
    pub fn new() -> Self {
        Self {
            tools: RIPWIRE_TOOLS.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    /// Answer `tool` with the recorded fixture `name`.
    pub fn answer(mut self, tool: &str, name: &str) -> Self {
        self.answers.insert(tool.into(), Ok(fixture(name)));
        self
    }

    /// Answer `tool` with a literal payload.
    pub fn answer_text(mut self, tool: &str, text: &str) -> Self {
        self.answers.insert(tool.into(), Ok(text.into()));
        self
    }

    /// Answer `tool` with these literal payloads, one per call, then with `answers`.
    pub fn answer_seq(self, tool: &str, texts: &[&str]) -> Self {
        self.sequences.lock().unwrap().insert(
            tool.into(),
            texts.iter().rev().map(|t| t.to_string()).collect(),
        );
        self
    }

    pub fn fail(mut self, tool: &str, err: UpstreamError) -> Self {
        self.answers.insert(tool.into(), Err(err));
        self
    }

    /// Every call fails as if the process were gone.
    pub fn down(mut self) -> Self {
        self.down = true;
        self
    }

    pub fn without_tool(mut self, tool: &str) -> Self {
        self.tools.retain(|t| t != tool);
        self
    }

    pub fn calls(&self) -> Vec<(String, Value)> {
        self.calls.lock().unwrap().clone()
    }

    pub fn called(&self) -> Vec<String> {
        self.calls().into_iter().map(|(t, _)| t).collect()
    }
}

#[async_trait]
impl Upstream for FakeUpstream {
    async fn list_tools(&self) -> Result<Vec<String>, UpstreamError> {
        Ok(self.tools.clone())
    }

    async fn call(&self, tool: &str, args: Value) -> Result<String, UpstreamError> {
        self.calls.lock().unwrap().push((tool.into(), args));
        if self.down {
            return Err(UpstreamError::Unavailable("process exited".into()));
        }
        if let Some(next) = self
            .sequences
            .lock()
            .unwrap()
            .get_mut(tool)
            .and_then(Vec::pop)
        {
            return Ok(next);
        }
        self.answers
            .get(tool)
            .cloned()
            .unwrap_or_else(|| Err(UpstreamError::Refused(format!("no fixture for {tool}"))))
    }
}
