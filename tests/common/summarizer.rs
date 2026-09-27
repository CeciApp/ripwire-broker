//! A scripted `Summarizer`: records the prompts it receives, can fail, and can be held
//! until the test releases it (no real time involved).
use async_trait::async_trait;
use ripwire_broker::summarizer::Summarizer;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Debug)]
pub struct FakeSummarizer {
    model: String,
    reply: Result<String, String>,
    gate: Option<Arc<Notify>>,
    prompts: Mutex<Vec<String>>,
}

impl FakeSummarizer {
    pub fn replying(text: &str) -> Self {
        Self {
            model: "fake-model".into(),
            reply: Ok(text.into()),
            gate: None,
            prompts: Mutex::new(vec![]),
        }
    }

    pub fn failing(why: &str) -> Self {
        Self {
            reply: Err(why.into()),
            ..Self::replying("")
        }
    }

    pub fn model(mut self, id: &str) -> Self {
        self.model = id.into();
        self
    }

    /// Every answer waits for `gate.notify_one()`.
    pub fn gated(mut self) -> (Self, Arc<Notify>) {
        let gate = Arc::new(Notify::new());
        self.gate = Some(gate.clone());
        (self, gate)
    }

    pub fn prompts(&self) -> Vec<String> {
        self.prompts.lock().unwrap().clone()
    }
}

#[async_trait]
impl Summarizer for FakeSummarizer {
    async fn summarize(&self, prompt: &str) -> Result<String, String> {
        self.prompts.lock().unwrap().push(prompt.into());
        if let Some(gate) = &self.gate {
            gate.notified().await;
        }
        self.reply.clone()
    }

    fn model_id(&self) -> String {
        self.model.clone()
    }

    fn program(&self) -> String {
        "fake".into()
    }
}
