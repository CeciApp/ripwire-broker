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
    /// Whether [`Summarizer::model_id`] pins the weights (a `--summarizer-version-cmd` ran):
    /// only then may a generated note be reused from disk.
    fn trusted_version(&self) -> bool {
        false
    }
}

/// A local model CLI (`ollama run phi4`, `llama-cli ...`): prompt on stdin, note on stdout.
/// The command line is split on whitespace into an argument array and never passed to a
/// shell, like ripwire itself (PRD 14.3).
#[derive(Debug)]
pub struct CommandSummarizer {
    argv: Vec<String>,
    hard_timeout: Duration,
    model_id: String,
    trusted_version: bool,
}

/// How long `--summarizer-version-cmd` may take (`ollama show` against a stuck server hangs).
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);

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
            let mut command = std::process::Command::new(&vargv[0]);
            command.args(&vargv[1..]).env_remove(crate::online::KEY_VAR);
            // It runs before `serve` answers its host: a command that hangs must not hang it.
            let deadline = std::time::Instant::now() + VERSION_TIMEOUT;
            let (status, stdout) = match crate::bounded::output(command, deadline) {
                Ok(done) => done,
                Err(crate::bounded::Stop::Late) => {
                    return Err(format!(
                        "{} did not finish within {} s",
                        vargv[0],
                        VERSION_TIMEOUT.as_secs()
                    ));
                }
                Err(crate::bounded::Stop::Failed) => {
                    return Err(format!("{}: could not run it", vargv[0]));
                }
            };
            if !status.success() {
                return Err(format!("{} exited with {}", vargv[0], status));
            }
            let digest = format!("{:x}", Sha256::digest(&stdout));
            model_id = format!("{model_id} @{}", &digest[..16]);
        }
        Ok(Self {
            argv,
            hard_timeout,
            model_id,
            trusted_version: version_cmd.is_some(),
        })
    }
}

#[async_trait]
impl Summarizer for CommandSummarizer {
    async fn summarize(&self, prompt: &str) -> Result<String, String> {
        let program = &self.argv[0];
        // The model is someone else's program: it never sees the provider key (D-146).
        let mut child = tokio::process::Command::new(program)
            .args(&self.argv[1..])
            .env_remove(crate::online::KEY_VAR)
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

    fn trusted_version(&self) -> bool {
        self.trusted_version
    }

    fn program(&self) -> String {
        std::path::Path::new(&self.argv[0])
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}
