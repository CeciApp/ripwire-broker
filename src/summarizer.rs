//! A local model that writes notes (PRD 10.3, D-034). Off unless configured.

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

#[async_trait]
pub trait Summarizer: Send + Sync + std::fmt::Debug {
    /// The model's answer to `prompt`, or why there is none.
    async fn summarize(&self, prompt: &str) -> Result<String, String>;
    /// Identifies the model for the note cache: a change invalidates cached notes.
    fn model_id(&self) -> String;
    /// The program's name for the status resource (no arguments, no paths).
    fn program(&self) -> String;
}

/// A local model CLI (`ollama run phi4`, `llama-cli ...`): prompt on stdin, note on stdout.
/// The command line is split on whitespace into an argument array and never passed to a
/// shell, like ripwire itself (PRD 14.3).
#[derive(Debug)]
pub struct CommandSummarizer {
    argv: Vec<String>,
    hard_timeout: Duration,
    model_id: String,
}

fn split_command(command: &str) -> Result<Vec<String>, String> {
    let argv: Vec<String> = command.split_whitespace().map(str::to_string).collect();
    if argv.is_empty() {
        return Err("empty summarizer command".into());
    }
    Ok(argv)
}

impl CommandSummarizer {
    /// `version_cmd` (e.g. `ollama show phi4 --modelfile`) runs once; the hash of its output
    /// joins the model id, so new weights under the same tag invalidate cached notes
    /// (PRD 10.3). Without it, the id is the command alone.
    pub fn from_command_line(
        command: &str,
        hard_timeout: Duration,
        version_cmd: Option<&str>,
    ) -> Result<Self, String> {
        let argv = split_command(command)?;
        let mut model_id = argv.join(" ");
        if let Some(v) = version_cmd {
            let vargv = split_command(v)?;
            let out = std::process::Command::new(&vargv[0])
                .args(&vargv[1..])
                .stdin(Stdio::null())
                .output()
                .map_err(|e| format!("{}: {e}", vargv[0]))?;
            if !out.status.success() {
                return Err(format!("{} exited with {}", vargv[0], out.status));
            }
            let digest = format!("{:x}", Sha256::digest(&out.stdout));
            model_id = format!("{model_id} @{}", &digest[..16]);
        }
        Ok(Self {
            argv,
            hard_timeout,
            model_id,
        })
    }
}

#[async_trait]
impl Summarizer for CommandSummarizer {
    async fn summarize(&self, prompt: &str) -> Result<String, String> {
        let program = &self.argv[0];
        let mut child = tokio::process::Command::new(program)
            .args(&self.argv[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("{program}: {e}"))?;
        let mut stdin = child.stdin.take().ok_or("no stdin")?;
        let run = async {
            // A model that exits without reading its input is not an error by itself.
            let _ = stdin.write_all(prompt.as_bytes()).await;
            drop(stdin);
            child.wait_with_output().await
        };
        let out = tokio::time::timeout(self.hard_timeout, run)
            .await
            .map_err(|_| format!("timeout after {} ms", self.hard_timeout.as_millis()))?
            .map_err(|e| format!("{program}: {e}"))?;
        if !out.status.success() {
            return Err(format!("{program} exited with {}", out.status));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn model_id(&self) -> String {
        self.model_id.clone()
    }

    fn program(&self) -> String {
        std::path::Path::new(&self.argv[0])
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}
