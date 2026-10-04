//! `ripwire-eval`: the A/B instrument (D-116). Separate from `ripwire-broker` on purpose: it
//! drives an agent from outside and adds nothing to the broker's own command line.

use ripwire_broker::eval::arm::{Arm, Tools};
use ripwire_broker::eval::corpus::Corpus;
use ripwire_broker::eval::report;
use ripwire_broker::eval::runner::{self, RunConfig};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "\
usage: ripwire-eval check --corpus FILE
       ripwire-eval validate --corpus FILE [--check-timeout-s N]
       ripwire-eval run --corpus FILE --out DIR [--arms none,ripwire,broker[,broker-online,broker-memory,broker-memory-deterministic]]
                    [--repeats N] [--agent-cmd CMD] [--broker BIN] [--ripwire BIN]
                    [--timeout-s N] [--check-timeout-s N]
       ripwire-eval report --out DIR [--json]

--agent-cmd is split on whitespace and never run through a shell; {mcp_config} and {workdir}
are replaced per run, and the task's prompt arrives on stdin. The default drives Claude Code
headless, isolated from your own settings, hooks and MCP servers.
Each run costs whatever the agent spends: a real run of the corpus is paid.";

struct Args {
    command: String,
    flags: std::collections::HashMap<String, String>,
    json: bool,
}

fn parse() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let command = it.next().ok_or_else(|| USAGE.to_string())?;
    let mut flags = std::collections::HashMap::new();
    let mut json = false;
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--json" => json = true,
            "--corpus" | "--out" | "--arms" | "--repeats" | "--agent-cmd" | "--broker"
            | "--ripwire" | "--timeout-s" | "--check-timeout-s" => {
                let value = it
                    .next()
                    .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
                flags.insert(flag, value);
            }
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown argument {other:?}\n{USAGE}")),
        }
    }
    Ok(Args {
        command,
        flags,
        json,
    })
}

fn required(a: &Args, flag: &str) -> Result<PathBuf, String> {
    a.flags
        .get(flag)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{flag} is required\n{USAGE}"))
}

fn number(a: &Args, flag: &str, default: u64) -> Result<u64, String> {
    a.flags.get(flag).map_or(Ok(default), |v| {
        v.parse().map_err(|_| format!("{flag}: not a number"))
    })
}

/// The `ripwire-broker` built next to this binary, else the one on `PATH`.
fn sibling_broker() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.join("ripwire-broker")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("ripwire-broker"))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let a = parse()?;
    match a.command.as_str() {
        "check" => {
            let corpus = Corpus::load(&required(&a, "--corpus")?)?;
            corpus.validate().map_err(|e| e.join("\n"))?;
            let (tasks, repos) = (corpus.tasks.len(), corpus.repos());
            println!("{tasks} tasks in {repos} repositories");
            if tasks < report::MIN_TASKS || repos < report::MIN_REPOS {
                println!(
                    "below the bar's minimum ({} tasks, {} repositories): every bar will read `insuficiente`",
                    report::MIN_TASKS,
                    report::MIN_REPOS
                );
            }
            Ok(())
        }
        "validate" => {
            let path = required(&a, "--corpus")?;
            let corpus = Corpus::load(&path)?;
            corpus.validate().map_err(|e| e.join("\n"))?;
            let timeout = Duration::from_secs(number(&a, "--check-timeout-s", 600)?);
            // Next to the corpus: the logs explain the corpus, not this checkout.
            let logs = path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("validate-logs");
            let mut broken = 0;
            for (id, verdict) in
                runner::validate(&corpus, timeout, &logs, &mut |l| eprintln!("{l}"))
            {
                match verdict {
                    Ok(()) => println!("{id}: ok"),
                    Err(why) => {
                        broken += usize::from(!why.starts_with("no check"));
                        println!("{id}: {why}");
                    }
                }
            }
            if broken > 0 {
                return Err(format!("{broken} task(s) broken"));
            }
            Ok(())
        }
        "run" => {
            let arms = a
                .flags
                .get("--arms")
                .map_or("none,ripwire,broker", String::as_str)
                .split(',')
                .map(|s| Arm::parse(s.trim()).ok_or_else(|| format!("unknown arm {s:?}")))
                .collect::<Result<Vec<_>, _>>()?;
            let agent = a
                .flags
                .get("--agent-cmd")
                .map_or(runner::DEFAULT_AGENT, String::as_str)
                .split_whitespace()
                .map(str::to_string)
                .collect();
            let cfg = RunConfig {
                corpus: Corpus::load(&required(&a, "--corpus")?)?,
                out: required(&a, "--out")?,
                arms,
                repeats: number(&a, "--repeats", 1)? as u32,
                agent,
                tools: Tools {
                    broker: a
                        .flags
                        .get("--broker")
                        .map_or_else(sibling_broker, PathBuf::from),
                    ripwire: a
                        .flags
                        .get("--ripwire")
                        .map_or_else(|| PathBuf::from("ripwire"), PathBuf::from),
                },
                timeout: Duration::from_secs(number(&a, "--timeout-s", 1800)?),
                check_timeout: Duration::from_secs(number(&a, "--check-timeout-s", 600)?),
            };
            let made = runner::run(&cfg, &mut |line| eprintln!("{line}"))?;
            eprintln!(
                "{made} runs made; results in {}",
                cfg.out.join("results.jsonl").display()
            );
            Ok(())
        }
        "report" => {
            let out_dir = required(&a, "--out")?;
            let records = report::load(&out_dir);
            let versions = std::fs::read_to_string(out_dir.join("versions.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
            if a.json {
                let arms: Vec<_> = ripwire_broker::eval::arm::ALL
                    .iter()
                    .map(|arm| report::arm_stats(&records, arm.name()))
                    .filter(|s| s.runs > 0)
                    .collect();
                let out = serde_json::json!({
                    "arms": arms,
                    "bars": report::bars(&records),
                    "memory_cost": report::memory_cost(&records),
                    "versions": versions,
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
            } else {
                print!("{}", report::render(&records));
                if let Some(v) = versions {
                    print!("{}", report::render_versions(&v, &records));
                }
            }
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}
