//! Seam 6: the A/B instrument (PRD §16.2–16.4, §17, §23.15; D-116). Pure parts through the
//! library, the run through the `ripwire-eval` binary with a stand-in agent. No real agent, no
//! network: what a real run costs is the user's decision, not the suite's.
mod common;

use ripwire_broker::eval::arm::Arm;
use ripwire_broker::eval::corpus::Corpus;
use ripwire_broker::eval::report::{self, RunRecord, Verdict};
use ripwire_broker::eval::score;
use ripwire_broker::eval::transcript::{self, Summary};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// The synthetic `stream-json` fixture, one event every 100 ms of the runner's clock.
fn fixture() -> Summary {
    let path = format!(
        "{}/tests/fixtures/eval/claude_code_stream.jsonl",
        env!("CARGO_MANIFEST_DIR")
    );
    let events: Vec<(u64, Value)> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .enumerate()
        .map(|(n, l)| (n as u64 * 100, serde_json::from_str(l).unwrap()))
        .collect();
    transcript::summarize(&events)
}

// --- corpus ---

#[test]
fn a_corpus_is_validated_before_anything_runs() {
    let repo = common::sample_repo();
    let head = git(repo.path(), &["rev-parse", "HEAD"]);
    let dir = tempfile::tempdir().unwrap();
    let task = |id: &str, repo: &str, base: &str, files: Value| {
        json!({"id": id, "repo": repo, "base": base, "prompt": "p",
               "reference": {"files": files, "tests": []}})
    };
    let path = dir.path().join("corpus.json");
    let good = repo.path().to_str().unwrap();
    std::fs::write(
        &path,
        json!({"tasks": [
            task("ok", good, &head, json!(["src/auth.py"])),
            task("ok", good, &head, json!(["src/auth.py"])),
            task("no-files", good, &head, json!([])),
            task("no-repo", "/nonexistent/repo", &head, json!(["a"])),
            task("no-base", good, "0000000000000000000000000000000000000000", json!(["a"])),
        ]})
        .to_string(),
    )
    .unwrap();

    let errors = Corpus::load(&path).unwrap().validate().unwrap_err();
    let all = errors.join("\n");
    for id in ["ok", "no-files", "no-repo", "no-base"] {
        assert!(all.contains(&format!("{id}:")), "{id} not reported:\n{all}");
    }

    // A relative repo path is relative to the corpus file, not to the caller's directory.
    let rel = dir.path().join("rel.json");
    let link = dir.path().join("repo");
    std::os::unix::fs::symlink(repo.path(), &link).unwrap();
    std::fs::write(
        &rel,
        json!({"tasks": [task("t1", "repo", &head, json!(["src/auth.py"]))]}).to_string(),
    )
    .unwrap();
    let corpus = Corpus::load(&rel).unwrap();
    assert_eq!(corpus.validate(), Ok(()));
    assert_eq!(corpus.tasks.len(), 1);
    assert_eq!(corpus.repos(), 1);
}

// --- transcript ---

#[test]
fn a_transcript_is_reduced_to_counts() {
    let s = fixture();

    assert!(s.valid(), "{:?}", s.invalid);
    // Grep, `rg` through Bash, and the subagent's Glob are searches; Read and `sed -n` reads.
    assert_eq!(s.calls.search, 3, "{:?}", s.calls);
    assert_eq!(s.calls.read, 2, "{:?}", s.calls);
    assert_eq!(s.calls.edit, 1, "{:?}", s.calls);
    assert_eq!(s.calls.mcp, 1, "{:?}", s.calls);
    assert_eq!(s.calls.other, 2, "Task and pytest: {:?}", s.calls);
    assert_eq!(s.calls.exploratory(), 5);
    // The result event carries the session's totals.
    assert_eq!(s.tokens.total(), 50 + 2000 + 30000 + 800);
    assert!((s.cost_usd - 0.1234).abs() < 1e-9);
    assert_eq!(s.duration_ms, 12000);
    // The Edit is the 12th event.
    assert_eq!(s.first_edit_ms, Some(1100));
    assert_eq!(s.presented_files, vec!["src/routes.py", "src/auth.py"]);
    assert_eq!(s.presented_tests, vec!["tests/test_auth.py"]);
    assert_eq!(
        s.mcp_result_bytes, 287,
        "the envelope's text, byte for byte"
    );
    assert_eq!(s.mcp_servers, vec!["ripwire-broker"]);
}

#[test]
fn a_transcript_without_a_result_is_invalid_not_zero() {
    let init = json!({"type": "system", "subtype": "init", "tools": [], "mcp_servers": []});
    let no_result = transcript::summarize(&[(0, init)]);
    assert!(!no_result.valid());
    assert!(no_result.invalid.as_deref().unwrap().contains("result"));

    let result = json!({"type": "result", "subtype": "success", "is_error": false,
                        "usage": {"input_tokens": 1, "output_tokens": 1}});
    let no_init = transcript::summarize(&[(0, result.clone())]);
    assert!(no_init.invalid.as_deref().unwrap().contains("init"));

    let garbage = transcript::summarize(&[]);
    assert!(!garbage.valid());

    let failed = json!({"type": "result", "subtype": "error_max_turns", "is_error": true,
                        "usage": {"input_tokens": 1, "output_tokens": 1}});
    let init = json!({"type": "system", "subtype": "init", "tools": [], "mcp_servers": []});
    let s = transcript::summarize(&[(0, init), (1, failed)]);
    assert!(
        s.valid(),
        "a failed task is a measurement, not a broken run"
    );
    assert!(s.is_error);
}

#[test]
fn an_arm_contaminated_by_a_foreign_mcp_is_invalid() {
    let s = fixture();
    assert_eq!(Arm::Broker.contamination(&s), None);
    let none = Arm::None
        .contamination(&s)
        .expect("ripwire-broker leaked into `none`");
    assert!(none.contains("ripwire-broker"), "{none}");
    assert!(Arm::Ripwire.contamination(&s).is_some());

    // The arm's own server must have connected, or `broker` silently becomes `none`.
    let init = json!({"type": "system", "subtype": "init", "tools": ["Read"],
                      "mcp_servers": [{"name": "ripwire-broker", "status": "failed"}]});
    let result = json!({"type": "result", "is_error": false, "usage": {}});
    let down = transcript::summarize(&[(0, init), (1, result)]);
    let why = Arm::Broker.contamination(&down).expect("not connected");
    assert!(why.contains("not connected"), "{why}");
}

#[test]
fn arms_name_their_mcp_server_and_nothing_else() {
    let tools = ripwire_broker::eval::arm::Tools {
        broker: "/bin/rb".into(),
        ripwire: "/bin/rw".into(),
    };
    let ws = Path::new("/work");
    assert_eq!(Arm::None.mcp_config(&tools, ws), json!({"mcpServers": {}}));
    let rw = Arm::Ripwire.mcp_config(&tools, ws);
    assert_eq!(rw["mcpServers"]["ripwire"]["command"], "/bin/rw");
    assert_eq!(
        rw["mcpServers"]["ripwire"]["args"],
        json!(["/work", "--mcp"])
    );
    let b = Arm::Broker.mcp_config(&tools, ws);
    assert_eq!(
        b["mcpServers"]["ripwire-broker"]["args"],
        json!(["serve", "--workspace", "/work", "--ripwire", "/bin/rw"])
    );
    let on = Arm::BrokerOnline.mcp_config(&tools, ws);
    let args = on["mcpServers"]["ripwire-broker"]["args"]
        .as_array()
        .unwrap();
    assert!(args.contains(&json!("--online")));
    for a in [Arm::None, Arm::Ripwire, Arm::Broker, Arm::BrokerOnline] {
        assert_eq!(Arm::parse(a.name()), Some(a));
    }
    assert_eq!(Arm::parse("bogus"), None);
}

// --- scoring ---

#[test]
fn a_run_is_scored_against_the_reference_patch() {
    let task: ripwire_broker::eval::corpus::Task = serde_json::from_value(json!({
        "id": "t", "repo": "/r", "base": "b", "prompt": "p",
        "reference": {"files": ["src/auth.py", "src/session.py"], "tests": ["tests/test_auth.py"]}
    }))
    .unwrap();
    let s = fixture();
    let modified = vec!["src/auth.py".to_string(), "notes.txt".to_string()];

    let sc = score::score(&task, &s, &modified, Some(true));

    assert_eq!(sc.file_recall, 0.5);
    assert_eq!(sc.file_precision, Some(0.5));
    assert_eq!(sc.presented_recall, Some(0.5));
    assert_eq!(sc.first_correct_rank, Some(2), "src/auth.py came second");
    assert_eq!(sc.test_recall, 1.0);
    assert_eq!(sc.correct, Some(true));

    let nothing = score::score(&task, &Summary::default(), &[], None);
    assert_eq!(nothing.file_recall, 0.0);
    assert_eq!(
        nothing.file_precision, None,
        "nothing modified: precision undefined"
    );
    assert_eq!(nothing.presented_recall, None, "no broker in the arm");
    assert_eq!(nothing.correct, None);
}

#[test]
fn modified_files_come_from_git_including_new_ones() {
    let repo = common::sample_repo();
    std::fs::write(repo.path().join("src/auth.py"), "changed\n").unwrap();
    std::fs::create_dir_all(repo.path().join("src/new")).unwrap();
    std::fs::write(repo.path().join("src/new/token.py"), "x\n").unwrap();

    let mut files = score::modified_files(repo.path());
    files.sort();
    assert_eq!(files, vec!["src/auth.py", "src/new/token.py"]);
}

// --- bars ---

/// One valid run of `arm` on task `n`, spread over `repos` repositories.
fn rec(n: usize, repos: usize, arm: Arm, tokens: u64, exploratory: u64) -> RunRecord {
    RunRecord {
        task: format!("t{n}"),
        repo: format!("repo{}", n % repos),
        arm: arm.name().into(),
        valid: true,
        tokens,
        search: exploratory,
        correct: Some(true),
        file_recall: 0.8,
        presented_recall: Some(0.7),
        test_recall: 0.9,
        ..RunRecord::default()
    }
}

fn corpus(
    tasks: usize,
    repos: usize,
    broker_tokens: u64,
    broker_exploratory: u64,
) -> Vec<RunRecord> {
    (0..tasks)
        .flat_map(|n| {
            let mut b = rec(n, repos, Arm::Broker, broker_tokens, broker_exploratory);
            b.file_recall = 0.9;
            [rec(n, repos, Arm::None, 1000, 100), b]
        })
        .collect()
}

fn verdict(records: &[RunRecord], id: &str) -> Verdict {
    report::bars(records)
        .into_iter()
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("no bar {id}"))
        .verdict
}

#[test]
fn the_bars_follow_the_prd_thresholds() {
    // §17.1: at least 35% fewer tokens. Exactly 35% passes; one token short fails.
    assert_eq!(verdict(&corpus(30, 3, 650, 70), "17.1"), Verdict::Pass);
    assert_eq!(verdict(&corpus(30, 3, 651, 70), "17.1"), Verdict::Fail);
    // §17.2: at least 30% fewer exploratory calls.
    assert_eq!(verdict(&corpus(30, 3, 650, 70), "17.2"), Verdict::Pass);
    assert_eq!(verdict(&corpus(30, 3, 650, 71), "17.2"), Verdict::Fail);
    // §17.3, 17.4, 17.5 on the same records.
    let ok = corpus(30, 3, 650, 70);
    assert_eq!(verdict(&ok, "17.3"), Verdict::Pass);
    assert_eq!(verdict(&ok, "17.4"), Verdict::Pass);
    assert_eq!(verdict(&ok, "17.5"), Verdict::Pass);

    // Fewer than 30 tasks or 3 repositories is not a result, whatever the numbers say.
    assert_eq!(
        verdict(&corpus(29, 3, 100, 1), "17.1"),
        Verdict::Insufficient
    );
    assert_eq!(
        verdict(&corpus(30, 2, 100, 1), "17.1"),
        Verdict::Insufficient
    );

    // An invalid run never counts: invalidating every broker run leaves no arm to compare.
    let mut broken = corpus(30, 3, 100, 1);
    for r in broken.iter_mut().filter(|r| r.arm == "broker") {
        r.valid = false;
    }
    assert_eq!(verdict(&broken, "17.1"), Verdict::Insufficient);

    // Cheaper but less correct is not a pass (§17 "uma economia que diminua correção").
    let mut worse = corpus(30, 3, 100, 1);
    for r in worse.iter_mut().filter(|r| r.arm == "broker").take(1) {
        r.correct = Some(false);
    }
    assert_eq!(verdict(&worse, "17.3"), Verdict::Fail);
}

#[test]
fn the_online_bar_is_measured_on_divergent_vocabulary_only() {
    let mut records = vec![];
    for n in 0..30 {
        let divergent = n % 2 == 0;
        let mut off = rec(n, 3, Arm::Broker, 600, 50);
        let mut on = rec(n, 3, Arm::BrokerOnline, 600, 40);
        off.vocabulary_diverges = divergent;
        on.vocabulary_diverges = divergent;
        off.presented_recall = Some(0.5);
        // +10 pp exactly where the vocabulary diverges, nothing elsewhere.
        on.presented_recall = Some(if divergent { 0.6 } else { 0.5 });
        records.extend([off, on]);
    }
    assert_eq!(verdict(&records, "23.15.2"), Verdict::Pass);
    assert_eq!(
        verdict(&records, "23.15.3"),
        Verdict::Pass,
        "50 → 40 is −20%"
    );
    assert_eq!(verdict(&records, "23.15.1"), Verdict::Pass);

    for r in records.iter_mut().filter(|r| r.arm == "broker-online") {
        if r.vocabulary_diverges {
            r.presented_recall = Some(0.59);
        }
    }
    assert_eq!(verdict(&records, "23.15.2"), Verdict::Fail);

    // Without the online arm there is nothing to compare.
    let offline: Vec<_> = records
        .into_iter()
        .filter(|r| r.arm != "broker-online")
        .collect();
    assert_eq!(verdict(&offline, "23.15.2"), Verdict::Insufficient);
}

// --- the binary, end to end ---

/// A stand-in for `claude -p --output-format stream-json`: reads the prompt on stdin, finds
/// its MCP config after `--mcp-config`, announces the server it names, edits a file in its
/// working directory and reports a result. Counts its runs in `$COUNTER`.
fn fake_agent(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("fake-agent");
    common::write_executable(
        &path,
        r##"#!/bin/sh
cat > /dev/null
cfg=""
while [ $# -gt 0 ]; do
  if [ "$1" = "--mcp-config" ]; then cfg="$2"; shift; fi
  shift
done
echo run >> "$COUNTER"
servers='[]'; tools='["Read","Edit"]'
if grep -q '"ripwire-broker"' "$cfg"; then
  servers='[{"name":"ripwire-broker","status":"connected"}]'
  tools='["Read","Edit","mcp__ripwire-broker__context_for_task"]'
elif grep -q '"ripwire"' "$cfg"; then
  servers='[{"name":"ripwire","status":"connected"}]'
  tools='["Read","Edit","mcp__ripwire__explore"]'
fi
echo "{\"type\":\"system\",\"subtype\":\"init\",\"tools\":$tools,\"mcp_servers\":$servers}"
echo '{"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","id":"t1","name":"Read","input":{"file_path":"src/auth.py"}}]}}'
echo '{"type":"assistant","message":{"id":"m2","content":[{"type":"tool_use","id":"t2","name":"Edit","input":{"file_path":"src/auth.py"}}]}}'
echo "# expired tokens are rejected" >> src/auth.py
echo '{"type":"result","subtype":"success","is_error":false,"duration_ms":5,"total_cost_usd":0.01,"usage":{"input_tokens":10,"cache_read_input_tokens":90,"output_tokens":5}}'
"##,
    );
    path
}

fn eval(args: &[&str], counter: &Path) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ripwire-eval"))
        .args(args)
        .env("COUNTER", counter)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn ripwire_eval_runs_every_arm_and_never_touches_the_source_repo() {
    let repo = common::sample_repo();
    let head = git(repo.path(), &["rev-parse", "HEAD"]);
    let work = tempfile::tempdir().unwrap();
    let agent = fake_agent(work.path());
    let counter = work.path().join("runs");
    let corpus = work.path().join("corpus.json");
    std::fs::write(
        &corpus,
        json!({"tasks": [{
            "id": "expired-token", "repo": repo.path(), "base": head,
            "prompt": "make login reject expired tokens",
            "reference": {"files": ["src/auth.py"], "tests": ["tests/test_auth.py"]},
            "check": "grep -q 'expired tokens' src/auth.py"
        }]})
        .to_string(),
    )
    .unwrap();
    let out = work.path().join("out");
    let agent_cmd = format!("{} --mcp-config {{mcp_config}}", agent.display());
    let run = |extra: &[&str]| {
        let mut args = vec![
            "run",
            "--corpus",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--arms",
            "none,ripwire,broker",
            "--agent-cmd",
            &agent_cmd,
        ];
        args.extend(extra);
        eval(&args, &counter)
    };

    let (code, _, err) = run(&[]);
    assert_eq!(code, 0, "{err}");

    let results = std::fs::read_to_string(out.join("results.jsonl")).unwrap();
    let records: Vec<Value> = results
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let arms: Vec<&str> = records.iter().map(|r| r["arm"].as_str().unwrap()).collect();
    assert_eq!(arms, vec!["none", "ripwire", "broker"]);
    for r in &records {
        assert_eq!(r["valid"], true, "{r}");
        assert_eq!(r["correct"], true, "the check ran in the clone: {r}");
        assert_eq!(r["file_recall"], 1.0, "{r}");
        assert_eq!(r["tokens"], 105, "{r}");
        assert_eq!(r["read"], 1, "{r}");
        assert_eq!(r["edit"], 1, "{r}");
    }
    for arm in ["none", "ripwire", "broker"] {
        assert!(
            out.join(format!("transcripts/expired-token__{arm}__1.jsonl"))
                .exists(),
            "{arm}"
        );
    }

    // The source repository is exactly as it was: no edit, no worktree, same HEAD.
    assert_eq!(git(repo.path(), &["status", "--porcelain"]), "");
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD"]), head);
    assert_eq!(git(repo.path(), &["worktree", "list"]).lines().count(), 1);

    // Run again: every (task, arm, repeat) is already in results.jsonl, so nothing reruns.
    let (code, _, err) = run(&[]);
    assert_eq!(code, 0, "{err}");
    let runs = std::fs::read_to_string(&counter).unwrap().lines().count();
    assert_eq!(runs, 3, "resumed, not repeated");
    // A second repeat only runs what is new.
    let (code, _, err) = run(&["--repeats", "2"]);
    assert_eq!(code, 0, "{err}");
    let runs = std::fs::read_to_string(&counter).unwrap().lines().count();
    assert_eq!(runs, 6);

    let (code, md, err) = eval(&["report", "--out", out.to_str().unwrap()], &counter);
    assert_eq!(code, 0, "{err}");
    assert!(md.contains("17.1"), "{md}");
    assert!(
        md.contains("insuficiente"),
        "one task in one repo cannot pass a bar: {md}"
    );

    // The online arm needs the credential, and refuses before running anything.
    let (code, _, err) = eval(
        &[
            "run",
            "--corpus",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--arms",
            "broker-online",
            "--agent-cmd",
            &agent_cmd,
        ],
        &counter,
    );
    assert_ne!(code, 0);
    assert!(err.contains("RIPWIRE_BROKER_JEV_API_KEY"), "{err}");
    let runs = std::fs::read_to_string(&counter).unwrap().lines().count();
    assert_eq!(runs, 6, "nothing ran");
}

#[test]
fn a_contaminated_run_is_recorded_as_invalid() {
    let repo = common::sample_repo();
    let head = git(repo.path(), &["rev-parse", "HEAD"]);
    let work = tempfile::tempdir().unwrap();
    // An agent whose global config leaks a server into every arm.
    let agent = work.path().join("leaky-agent");
    common::write_executable(
        &agent,
        r#"#!/bin/sh
cat > /dev/null
echo '{"type":"system","subtype":"init","tools":["mcp__graft__ask"],"mcp_servers":[{"name":"graft","status":"connected"}]}'
echo '{"type":"result","is_error":false,"usage":{"input_tokens":1,"output_tokens":1}}'
"#,
    );
    let corpus = work.path().join("corpus.json");
    std::fs::write(
        &corpus,
        json!({"tasks": [{"id": "t", "repo": repo.path(), "base": head, "prompt": "p",
                          "reference": {"files": ["src/auth.py"]}}]})
        .to_string(),
    )
    .unwrap();
    let out = work.path().join("out");

    let (code, _, err) = eval(
        &[
            "run",
            "--corpus",
            corpus.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--arms",
            "none",
            "--agent-cmd",
            agent.to_str().unwrap(),
        ],
        &work.path().join("runs"),
    );

    assert_eq!(code, 0, "{err}");
    let line = std::fs::read_to_string(out.join("results.jsonl")).unwrap();
    let r: Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(r["valid"], false, "{r}");
    assert!(r["invalid"].as_str().unwrap().contains("graft"), "{r}");
}

#[test]
fn the_first_edit_is_the_earliest_one() {
    let edit = |id: &str| {
        json!({"type": "assistant", "message": {"id": id, "content": [
            {"type": "tool_use", "id": id, "name": "Write", "input": {"file_path": "a"}}]}})
    };
    let s = transcript::summarize(&[
        (
            0,
            json!({"type": "system", "subtype": "init", "tools": [], "mcp_servers": []}),
        ),
        (300, edit("e1")),
        (900, edit("e2")),
        (
            1000,
            json!({"type": "result", "is_error": false, "usage": {}}),
        ),
    ]);
    assert_eq!(s.first_edit_ms, Some(300));
    assert_eq!(s.calls.edit, 2);
}

#[test]
fn the_agent_cannot_see_history_after_the_base() {
    // A task taken from history: the fix is a later commit of the same repository. If the
    // agent's clone carries it, `git log --all` hands the agent the answer.
    let repo = common::sample_repo();
    let base = git(repo.path(), &["rev-parse", "HEAD"]);
    std::fs::write(repo.path().join("src/auth.py"), "the answer\n").unwrap();
    // An identity of its own, as `sample_repo` does: CI runners have none configured.
    git(
        repo.path(),
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-qam",
            "the reference fix",
        ],
    );
    let work = tempfile::tempdir().unwrap();
    let seen = work.path().join("seen");
    let agent = work.path().join("curious-agent");
    common::write_executable(
        &agent,
        r#"#!/bin/sh
cat > /dev/null
{ git log --all --format=%s; git for-each-ref; cat src/auth.py; } > "$COUNTER"
echo '{"type":"system","subtype":"init","tools":[],"mcp_servers":[]}'
echo '{"type":"result","is_error":false,"usage":{"input_tokens":1,"output_tokens":1}}'
"#,
    );
    let corpus = work.path().join("corpus.json");
    std::fs::write(
        &corpus,
        json!({"tasks": [{"id": "t", "repo": repo.path(), "base": base, "prompt": "p",
                          "reference": {"files": ["src/auth.py"]}}]})
        .to_string(),
    )
    .unwrap();

    let (code, _, err) = eval(
        &[
            "run",
            "--corpus",
            corpus.to_str().unwrap(),
            "--out",
            work.path().join("out").to_str().unwrap(),
            "--arms",
            "none",
            "--agent-cmd",
            agent.to_str().unwrap(),
        ],
        &seen,
    );

    assert_eq!(code, 0, "{err}");
    let seen = std::fs::read_to_string(&seen).unwrap();
    assert!(
        !seen.contains("the reference fix"),
        "the future leaked:\n{seen}"
    );
    assert!(
        !seen.contains("the answer"),
        "the fixed file leaked:\n{seen}"
    );
    assert!(
        seen.contains("def login"),
        "the agent works on the base:\n{seen}"
    );
}
