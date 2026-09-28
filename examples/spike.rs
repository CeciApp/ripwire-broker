//! Phase 0 measurements: `cargo run --release --example spike -- <workspace> ["task"]`.
//! Prints sizes and latencies only; never code or paths.

use ripwire_broker::broker::{Broker, BrokerConfig, FinishRequest, TaskRequest};
use ripwire_broker::mcp::tools;
use ripwire_broker::upstream::{RipwireUpstream, Upstream, UpstreamConfig};
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let ws = std::path::PathBuf::from(args.next().expect("workspace"));
    let task = args
        .next()
        .unwrap_or_else(|| "how is authentication handled?".into());

    let t = Instant::now();
    let up = Arc::new(
        RipwireUpstream::spawn(UpstreamConfig::new(&ws))
            .await
            .unwrap(),
    );
    println!("spawn                         {:>8.1} ms", ms(t));
    let t = Instant::now();
    let names = up.list_tools().await.unwrap();
    println!(
        "tools/list (upstream)         {:>8.1} ms  {} tools",
        ms(t),
        names.len()
    );
    let ours = serde_json::to_string(&tools(false)).unwrap().len();
    println!(
        "broker tool schemas           {:>8} bytes (~{} tokens) for 3 tools",
        ours,
        ours / 4
    );

    for (verb, a) in [
        ("explore", json!({"task": task, "budget_tokens": 2500})),
        ("situational_awareness", json!({})),
        ("quality_delta", json!({})),
    ] {
        for round in ["cold", "warm"] {
            let t = Instant::now();
            let Ok(out) = up.call(verb, a.clone()).await else {
                println!("{verb:<22} refused (no git diff)");
                break;
            };
            println!(
                "{verb:<22} {round:<5} {:>8.1} ms  {:>7} bytes (~{} tokens)",
                ms(t),
                out.len(),
                out.len() / 4
            );
        }
    }

    let broker = Broker::connect(up.clone(), BrokerConfig::new(&ws))
        .await
        .unwrap();
    let mut samples = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        let env = broker
            .context_for_task(TaskRequest::new(&task))
            .await
            .unwrap();
        samples.push((ms(t), env.budget.estimated_tokens));
    }
    let t = Instant::now();
    let gate = broker
        .context_before_finish(FinishRequest::default())
        .await
        .unwrap();
    println!(
        "context_before_finish         {:>8.1} ms  status={:?} ~{} tokens",
        ms(t),
        gate.status,
        gate.budget.estimated_tokens
    );
    let st = serde_json::to_value(broker.status().await).unwrap();
    let m = &st["metrics"];
    let total: f64 = m["tools"]
        .as_object()
        .unwrap()
        .values()
        .map(|t| t["total_us"].as_f64().unwrap())
        .sum();
    let upstream_us = m["upstream_us"].as_f64().unwrap();
    samples.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!(
        "context_for_task x20          p50 {:.1} ms  p95 {:.1} ms  ~{} tokens",
        samples[10].0, samples[18].0, samples[10].1
    );
    let calls = m["tools"]
        .as_object()
        .unwrap()
        .values()
        .map(|t| t["calls"].as_u64().unwrap())
        .sum::<u64>();
    println!(
        "broker own time               {:>8.3} ms per tool call (mean of {calls})",
        (total - upstream_us) / 1000.0 / calls as f64
    );
}
