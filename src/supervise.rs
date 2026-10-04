//! `__supervise` (D-050, D-052): runs ripwire as a child with the same stdio under a memory
//! limit (PRD 15.3). The SDK starts this process and, on a restart, SIGKILLs it alone, which
//! cannot be caught. So the watching happens in a second process, `__watch`: it kills
//! ripwire when its resident memory passes the limit, and also as soon as the supervisor is
//! gone, so no ripwire is ever left running unwatched. `ps` and `kill` behave the same on
//! macOS and Linux, where `RLIMIT_AS` would not (macOS does not enforce it).

use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

const POLL: Duration = Duration::from_millis(200);

/// Resident set size of `pid` in MiB and when it started (`ps -o lstart=`, its spaces folded), or
/// `None` once it is gone. The start time tells ripwire from a later process that reuses its pid.
fn probe(pid: u32) -> Option<(u64, String)> {
    let out = Command::new("ps")
        .args(["-o", "rss=,lstart=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut words = text.split_whitespace();
    let kib: u64 = words.next()?.parse().ok()?;
    Some((kib / 1024, words.collect::<Vec<_>>().join(" ")))
}

/// `ps -o lstart=` with its spaces folded, so two readings compare equal.
fn fold(started: &str) -> String {
    started.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn kill(pid: u32) {
    let _ = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .stderr(Stdio::null())
        .status();
}

pub fn run(max_rss_mb: u64, argv: &[String]) -> ExitCode {
    let Some((program, args)) = argv.split_first() else {
        eprintln!("ripwire-broker: __supervise needs a command after --");
        return ExitCode::from(2);
    };
    let mut child = match Command::new(program).args(args).spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ripwire-broker: {program}: {e}");
            return ExitCode::from(127);
        }
    };
    let started = probe(child.id()).map(|(_, started)| started);
    let watcher = std::env::current_exe().and_then(|me| {
        let mut watch = Command::new(me);
        watch.args([
            "__watch",
            "--parent",
            &std::process::id().to_string(),
            "--child",
            &child.id().to_string(),
            "--max-rss-mb",
            &max_rss_mb.to_string(),
            "--program",
            program,
        ]);
        if let Some(started) = &started {
            watch.args(["--child-started", started]);
        }
        watch.stdin(Stdio::null()).stdout(Stdio::null()).spawn()
    });
    if let Err(e) = watcher {
        // Without a watcher there is neither a limit nor orphan protection: refuse to run.
        let _ = child.kill();
        eprintln!("ripwire-broker: cannot start the memory watcher: {e}");
        return ExitCode::FAILURE;
    }
    match child.wait() {
        Ok(status) => ExitCode::from(
            status
                .code()
                .or(status.signal().map(|s| 128 + s))
                .unwrap_or(1) as u8,
        ),
        Err(e) => {
            eprintln!("ripwire-broker: waiting for {program}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `__watch`: kills `child` above the limit or once `parent` (the supervisor) is gone. Only the
/// process that started at `started` (as the supervisor read it; now, without it) is ever killed: a
/// process that reuses the pid after ripwire ended is not ripwire (D-147).
pub fn watch(
    parent: u32,
    child: u32,
    max_rss_mb: u64,
    program: &str,
    started: Option<&str>,
) -> ExitCode {
    let Some(born) = started.map(fold).or_else(|| probe(child).map(|(_, s)| s)) else {
        return ExitCode::SUCCESS; // ripwire ended before the watch began
    };
    loop {
        let Some((rss, at)) = probe(child) else {
            return ExitCode::SUCCESS; // ripwire ended on its own
        };
        if at != born {
            return ExitCode::SUCCESS; // ripwire ended, and its pid went to another process
        }
        if std::os::unix::process::parent_id() != parent {
            kill(child); // the supervisor was killed: never leave ripwire unwatched
            return ExitCode::SUCCESS;
        }
        if rss > max_rss_mb {
            kill(child);
            eprintln!(
                "ripwire-broker: {program} passed the memory limit ({rss} MiB > {max_rss_mb} MiB) and was killed"
            );
            return ExitCode::SUCCESS;
        }
        std::thread::sleep(POLL);
    }
}
