//! A child process with a deadline (D-146): what `git` for the worktree fingerprint and
//! `ripwire --version` both need, since either can hang and both run where a hang holds up a host.
use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Why there is no output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stop {
    /// It could not be started, or its output could not be read.
    Failed,
    /// The deadline passed first; a child still running was killed.
    Late,
}

/// Runs `command` with stdin and stderr closed and returns its exit status and its stdout.
///
/// Stdout is read on a thread: a long listing would fill the pipe and stall the child past the
/// deadline. The answer comes back over a channel with the same deadline: a process that the child
/// left behind can hold the pipe open long after the child exits, and a plain `join` would wait for
/// it. Such a reader is abandoned; it ends when the pipe closes.
pub(crate) fn output(
    mut command: Command,
    deadline: Instant,
) -> Result<(ExitStatus, Vec<u8>), Stop> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Stop::Failed)?;
    let mut stdout = child.stdout.take().ok_or(Stop::Failed)?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = tx.send(stdout.read_to_end(&mut buf).map(|_| buf));
    });
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let left = deadline.saturating_duration_since(Instant::now());
                return match rx.recv_timeout(left) {
                    Ok(read) => read.map(|out| (status, out)).map_err(|_| Stop::Failed),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(Stop::Late),
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(Stop::Failed),
                };
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(2)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Stop::Late);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Stop::Failed);
            }
        }
    }
}
