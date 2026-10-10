//! `serve --online --log` (D-155): every exchange with Jev written to `jev.log` in the state dir,
//! for a person to read. Each call is one block: what was sent, what came back and how long it
//! took, with the JSON pretty printed. Off by default.
//!
//! The file holds what the broker sends, so source the classifier saw is in it: it is private
//! (0600) and stays local. The key is never in it: the `Authorization` header is written redacted
//! and any echo of the key in a response is replaced before the block is written.
//!
//! Cloudflare's URL carries the account id (D-166). It is not a credential: alone it grants
//! nothing, and Cloudflare shows it in dashboard URLs. It is treated as a secret here anyway,
//! because it names the tenant, it is stable, and this file exists to be read and pasted into an
//! issue. So the URL is written `accounts/[redacted]/`, and an echo of the id in a response is
//! replaced like an echo of the key. Hiding it costs nothing: the model is still in the URL, and
//! whoever reads the log knows which account they started the server with.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

const WIDTH: usize = 72;

pub struct JevLog {
    file: Mutex<File>,
    seq: AtomicU64,
}

/// One call as the client saw it.
pub struct Exchange<'a> {
    pub endpoint: &'a str,
    /// `discovery` or `memory`.
    pub purpose: &'a str,
    pub sent: &'a [u8],
    /// `None`: no HTTP answer (timeout, network).
    pub status: Option<u16>,
    pub received: &'a [u8],
    /// The error category when the call failed.
    pub error: Option<&'a str>,
    pub elapsed: Duration,
}

impl JevLog {
    /// Appends to `path`, creating it 0600 (and a missing parent 0700). A link is refused.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        Ok(Self {
            file: Mutex::new(file),
            seq: AtomicU64::new(0),
        })
    }

    /// Writes one block in a single write, so concurrent calls never interleave. Each of
    /// `secrets` is replaced wherever it appears: in the URL, in what was sent and in what came
    /// back. A failed write is ignored: the log never fails a call.
    pub fn record(&self, e: &Exchange<'_>, secrets: &[&str]) {
        let n = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let block = super::redact::hide(block(n, e, now_utc()), secrets);
        let mut f = self.file.lock().unwrap_or_else(|p| p.into_inner());
        let _ = f.write_all(block.as_bytes());
    }
}

fn rule(c: char) -> String {
    std::iter::repeat_n(c, WIDTH).collect()
}

fn section(title: &str) -> String {
    let head = format!("── {title} ");
    let fill = WIDTH.saturating_sub(head.chars().count());
    format!("{head}{}\n", "─".repeat(fill))
}

/// Pretty JSON when it parses, the text as it came otherwise.
fn pretty(bytes: &[u8]) -> String {
    match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn block(n: u64, e: &Exchange<'_>, at: String) -> String {
    let mut out = String::new();
    out.push_str(&rule('═'));
    out.push('\n');
    out.push_str(&format!("Jev call #{n} · {at} · {}\n", e.purpose));
    out.push_str(&rule('═'));
    out.push_str("\n\n");
    out.push_str(&section("enviado"));
    out.push_str(&format!("POST {}\n", e.endpoint));
    out.push_str("Authorization: Bearer [redacted]\nContent-Type: application/json\n\n");
    out.push_str(&pretty(e.sent));
    out.push_str("\n\n");
    out.push_str(&section("recebido"));
    match e.status {
        Some(s) => out.push_str(&format!("HTTP {s}\n")),
        None => out.push_str("sem resposta HTTP\n"),
    }
    if let Some(err) = e.error {
        out.push_str(&format!("erro: {err}\n"));
    }
    if !e.received.is_empty() {
        out.push('\n');
        out.push_str(&pretty(e.received));
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&section("duração"));
    out.push_str(&format!("{} ms\n\n", e.elapsed.as_millis()));
    out
}

fn now_utc() -> String {
    crate::memory::time::utc(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    )
}
