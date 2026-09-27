//! `__supervise` (D-050): runs ripwire as a child with the same stdio and kills it when its
//! resident memory passes a limit (PRD 15.3). The broker's SDK sees the process end, and the
//! upstream's controlled restart takes over. `ps` works the same on macOS and Linux, where
//! `RLIMIT_AS` would not (macOS does not enforce it).

use std::process::{Command, ExitCode};
use std::time::Duration;

const POLL: Duration = Duration::from_millis(200);

/// Resident set size of `pid` in MiB, or `None` once it is gone.
fn rss_mb(pid: u32) -> Option<u64> {
    let out = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let kib: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    Some(kib / 1024)
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
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return ExitCode::from(status.code().unwrap_or(1) as u8),
            Ok(None) => {}
            Err(e) => {
                eprintln!("ripwire-broker: waiting for {program}: {e}");
                return ExitCode::FAILURE;
            }
        }
        if let Some(rss) = rss_mb(child.id()).filter(|rss| *rss > max_rss_mb) {
            let _ = child.kill();
            let _ = child.wait();
            eprintln!(
                "ripwire-broker: {program} passed the memory limit ({rss} MiB > {max_rss_mb} MiB) and was killed"
            );
            return ExitCode::from(137);
        }
        std::thread::sleep(POLL);
    }
}
