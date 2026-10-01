//! Seam 4: host hook events → broker → host output, with recorded payloads from real hosts
//! (tests/fixtures/hooks, captured from Claude Code 2.1.283 and Codex 0.157.1).
mod common;

use common::fake::FakeUpstream;
use ripwire_broker::broker::{Broker, BrokerConfig};
use ripwire_broker::cli::{Event, Host};
use ripwire_broker::hook::{self, Policy, SessionState};
use ripwire_broker::statusline_state::{self as projection, AnalysisStatus};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;

/// A recorded host event with its workspace placeholder pointed at `ws`.
fn event(name: &str, ws: &Path) -> Value {
    let path = format!(
        "{}/tests/fixtures/hooks/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap();
    serde_json::from_str(&text.replace("__WORKSPACE__", &ws.display().to_string())).unwrap()
}

async fn hook_broker(fake: FakeUpstream) -> (Broker, Arc<FakeUpstream>, tempfile::TempDir) {
    let ws = tempfile::tempdir().unwrap();
    let fake = Arc::new(fake);
    let mut config = BrokerConfig::new(ws.path());
    config.incremental = true;
    let b = Broker::connect(fake.clone(), config).await.unwrap();
    (b, fake, ws)
}

/// The envelope carried after the one-line header of an injected context.
fn injected(out: &Value) -> Value {
    let text = out["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    serde_json::from_str(text.split_once('\n').unwrap().1).unwrap()
}

#[tokio::test]
async fn the_first_prompt_gets_task_context_as_additional_context() {
    let (b, fake, ws) =
        hook_broker(FakeUpstream::new().answer("explore", "explore_export_auth")).await;
    let input = event("claude_code_user_prompt_submit", ws.path());
    let mut state = SessionState::default();

    let out = hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &input,
        &b,
        &mut state,
        &Policy::default(),
    )
    .await
    .expect("injected");

    assert_eq!(
        out["hookSpecificOutput"]["hookEventName"],
        "UserPromptSubmit"
    );
    let text = out["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    let header = text.lines().next().unwrap();
    assert!(header.starts_with("ripwire-broker context"), "{header}");
    assert!(header.contains("untrusted"), "{header}");
    let env = injected(&out);
    assert_eq!(env["tool"], "context_for_task");
    assert_eq!(env["items"][0]["symbol"], "export_route");
    assert_eq!(fake.called(), vec!["explore"]);
    assert_eq!(fake.calls()[0].1["task"], input["prompt"]);
    let note = out["systemMessage"].as_str().unwrap();
    assert!(
        note.contains("context_for_task") && note.contains("7 items"),
        "{note}"
    );
    assert_eq!(state.prompts_seen, 1);
    assert!(
        !state.memory.is_empty(),
        "the session remembers what was injected"
    );
}

// Both hosts inline ~10k characters of hook context (D-041).
const _: () = assert!(hook::MAX_CONTEXT_CHARS <= 9_000);

/// An `explore` answer far larger than any hook may inject: 60 ranked functions with bodies.
fn big_explore() -> String {
    let mut sigs = String::new();
    let mut bodies = String::new();
    for n in 0..60 {
        sigs.push_str(&format!(
            r#"<d l="{l}" n="handler_{n}" p="src/h{n}.py" r="{r}">def handler_{n}(req):</d>"#,
            l = n + 1,
            r = n + 1
        ));
        bodies.push_str(&format!(
            r#"<b t="fn" l="{l}" p="src/h{n}.py" n="handler_{n}"><![CDATA[def handler_{n}(req):
    {filler}
    return req]]></b>"#,
            l = n + 1,
            filler = "x = 1  # padding ".repeat(12)
        ));
    }
    format!(
        r#"<ctx schema="ripwire.pack-task/v1" task="t" route="subtoken+body"><sigs>{sigs}</sigs><bodies shown="60" total="60" capped="0">{bodies}</bodies></ctx>"#
    )
}

#[tokio::test]
async fn hook_output_stays_under_the_host_limit() {
    let (b, _fake, ws) =
        hook_broker(FakeUpstream::new().answer_text("explore", &big_explore())).await;
    let mut input = event("codex_user_prompt_submit", ws.path());
    input["prompt"] = "how are requests handled?".into();
    let policy = Policy {
        prompt_budget: 50_000,
        ..Policy::default()
    };

    let out = hook::handle(
        Host::Codex,
        Event::UserPromptSubmit,
        &input,
        &b,
        &mut SessionState::default(),
        &policy,
    )
    .await
    .unwrap();

    let text = out["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        text.chars().count() <= hook::MAX_CONTEXT_CHARS,
        "{} chars",
        text.chars().count()
    );
    let env = injected(&out);
    assert_eq!(
        env["budget"]["truncated"], true,
        "the cut is reported, not silent"
    );
}

#[tokio::test]
async fn later_prompts_are_not_injected_unless_every_prompt_is_set() {
    let (b, fake, ws) =
        hook_broker(FakeUpstream::new().answer("explore", "explore_export_auth")).await;
    let input = event("claude_code_user_prompt_submit", ws.path());
    let mut state = SessionState::default();
    let every = Policy {
        every_prompt: true,
        ..Policy::default()
    };
    let (ev, cc) = (Event::UserPromptSubmit, Host::ClaudeCode);

    let first = hook::handle(cc, ev, &input, &b, &mut state, &Policy::default()).await;
    let second = hook::handle(cc, ev, &input, &b, &mut state, &Policy::default()).await;
    assert!(first.is_some());
    assert!(second.is_none(), "second prompt: silent");
    assert_eq!(fake.called().len(), 1, "and no upstream call");
    let third = hook::handle(cc, ev, &input, &b, &mut state, &every).await;
    assert!(third.is_some());
    assert_eq!(state.prompts_seen, 3);
}

#[tokio::test]
async fn an_opt_out_marker_silences_the_session_until_opt_in() {
    let (b, fake, ws) =
        hook_broker(FakeUpstream::new().answer("explore", "explore_export_auth")).await;
    let mut input = event("claude_code_user_prompt_submit", ws.path());
    let every = Policy {
        every_prompt: true,
        ..Policy::default()
    };
    let mut state = SessionState::default();

    input["prompt"] = "#ripwire-off let me explore on my own".into();
    let off = hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &input,
        &b,
        &mut state,
        &every,
    )
    .await;
    input["prompt"] = "how are the routes authenticated?".into();
    let still_off = hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &input,
        &b,
        &mut state,
        &every,
    )
    .await;

    assert!(state.opted_out);
    assert!(fake.called().is_empty(), "no upstream call while opted out");
    let note = off.expect("the opt-out is acknowledged")["systemMessage"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        note.contains("off") && note.contains("#ripwire-on"),
        "{note}"
    );
    assert!(still_off.is_none());
    std::fs::write(ws.path().join("a.txt"), "x").unwrap();
    let gate = Policy {
        gate: true,
        ..Policy::default()
    };
    for (ev, name) in [
        (Event::PostToolUse, "claude_code_post_tool_use"),
        (Event::Stop, "claude_code_stop"),
    ] {
        let quiet = hook::handle(
            Host::ClaudeCode,
            ev,
            &event(name, ws.path()),
            &b,
            &mut state,
            &gate,
        )
        .await;
        assert!(quiet.is_none(), "{name} while opted out: {quiet:?}");
    }
    assert!(fake.called().is_empty(), "no upstream call while opted out");

    input["prompt"] = "#ripwire-on how are the routes authenticated?".into();
    let on = hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &input,
        &b,
        &mut state,
        &every,
    )
    .await;
    assert!(!state.opted_out);
    assert_eq!(injected(&on.unwrap())["tool"], "context_for_task");
    assert_eq!(
        fake.calls()[0].1["task"],
        "how are the routes authenticated?",
        "marker stripped"
    );
}

#[tokio::test]
async fn a_marker_only_counts_as_the_first_or_last_word_of_the_prompt() {
    let (b, _fake, ws) =
        hook_broker(FakeUpstream::new().answer("explore", "explore_export_auth")).await;
    let mut input = event("claude_code_user_prompt_submit", ws.path());
    let every = Policy {
        every_prompt: true,
        ..Policy::default()
    };
    let mut state = SessionState::default();
    let mut say = async |state: &mut SessionState, prompt: &str| {
        input["prompt"] = prompt.into();
        hook::handle(
            Host::ClaudeCode,
            Event::UserPromptSubmit,
            &input,
            &b,
            state,
            &every,
        )
        .await
    };

    // A marker quoted or mentioned inside the text is not a command.
    for quoted in [
        "neither `#ripwire-on` nor `#ripwire-off` is processed",
        "the defect: neither #ripwire-on nor #ripwire-off is processed here",
        "type #ripwire-off/#ripwire-on to toggle",
        "a tag glued to a word, like x#ripwire-off",
    ] {
        say(&mut state, quoted).await;
        assert!(!state.opted_out, "{quoted}");
    }

    say(&mut state, "pause now #ripwire-off").await;
    assert!(state.opted_out, "last word");
    say(&mut state, "a report quoting #ripwire-on mid-sentence").await;
    assert!(state.opted_out, "a quoted opt-in does not resume");
    say(&mut state, "#ripwire-on and carry on").await;
    assert!(!state.opted_out, "first word");
    say(&mut state, "#ripwire-off").await;
    assert!(state.opted_out, "the marker alone");
}

fn edit_fake() -> FakeUpstream {
    FakeUpstream::new()
        .answer("situational_awareness", "situational_awareness_files")
        .answer("edit_check", "edit_check_login")
        .answer("impact", "impact_login")
}

async fn post_tool_use(
    host: Host,
    input: &Value,
    b: &Broker,
    state: &mut SessionState,
) -> Option<Value> {
    hook::handle(
        host,
        Event::PostToolUse,
        input,
        b,
        state,
        &Policy::default(),
    )
    .await
}

#[tokio::test]
async fn an_edit_injects_what_it_may_have_affected() {
    let (b, fake, ws) = hook_broker(edit_fake()).await;
    std::fs::write(ws.path().join("a.txt"), "hello.").unwrap();
    let input = event("claude_code_post_tool_use", ws.path());

    let out = post_tool_use(Host::ClaudeCode, &input, &b, &mut SessionState::default())
        .await
        .expect("injected");

    assert_eq!(out["hookSpecificOutput"]["hookEventName"], "PostToolUse");
    let env = injected(&out);
    assert_eq!(env["tool"], "context_after_edit");
    assert_eq!(env["budget"]["requested_tokens"], 800);
    assert_eq!(fake.calls()[0].0, "situational_awareness");
    assert_eq!(
        fake.calls()[0].1["files"],
        "a.txt",
        "absolute host path → workspace-relative"
    );
}

#[tokio::test]
async fn a_codex_patch_names_the_changed_files() {
    let (b, fake, ws) = hook_broker(edit_fake()).await;
    let mut input = event("codex_post_tool_use", ws.path());
    input["tool_input"]["command"] = "*** Begin Patch\n\
        *** Add File: src/new.py\n+x = 1\n\
        *** Update File: src/old_name.py\n*** Move to: src/new_name.py\n@@\n-a\n+b\n\
        *** Delete File: legacy.py\n\
        *** Update File: src/new.py\n@@\n-x\n+y\n\
        *** End Patch"
        .into();

    post_tool_use(Host::Codex, &input, &b, &mut SessionState::default())
        .await
        .expect("injected");

    assert_eq!(
        fake.calls()[0].1["files"],
        "src/new.py,src/old_name.py,src/new_name.py,legacy.py",
        "every path, in order, once"
    );
}

#[tokio::test]
async fn an_edit_outside_the_workspace_is_ignored_without_upstream_calls() {
    let (b, fake, ws) = hook_broker(edit_fake()).await;
    let mut claude = event("claude_code_post_tool_use", ws.path());
    claude["tool_input"]["file_path"] = "/etc/hosts".into();
    let mut codex = event("codex_post_tool_use", ws.path());
    codex["tool_input"]["command"] =
        "*** Begin Patch\n*** Update File: ../escape.txt\n@@\n-a\n+b\n*** End Patch".into();

    let a = post_tool_use(Host::ClaudeCode, &claude, &b, &mut SessionState::default()).await;
    let c = post_tool_use(Host::Codex, &codex, &b, &mut SessionState::default()).await;

    assert!(
        a.is_none() && c.is_none(),
        "policy, not a failure: no message either"
    );
    assert!(
        fake.called().is_empty(),
        "refused before any upstream call (CA-08)"
    );

    // A patch touching both sides still reports the inside file.
    codex["tool_input"]["command"] =
        "*** Begin Patch\n*** Update File: ../escape.txt\n*** Update File: src/in.py\n*** End Patch".into();
    let mixed = post_tool_use(Host::Codex, &codex, &b, &mut SessionState::default()).await;
    assert!(mixed.is_some());
    assert_eq!(fake.calls()[0].1["files"], "src/in.py");
}

#[tokio::test]
async fn an_edit_with_nothing_new_injects_nothing() {
    let (b, _fake, ws) = hook_broker(edit_fake()).await;
    let input = event("claude_code_post_tool_use", ws.path());
    let mut state = SessionState::default();

    let first = post_tool_use(Host::ClaudeCode, &input, &b, &mut state).await;
    let again = post_tool_use(Host::ClaudeCode, &input, &b, &mut state).await;

    assert!(first.is_some());
    assert!(
        again.is_none(),
        "same items, tests and risks as before: {again:?}"
    );
}

fn finish_fake() -> FakeUpstream {
    edit_fake()
        .answer("affected", "affected_auth")
        .answer("quality_delta", "quality_delta_clean")
}

async fn stop(host: Host, input: &Value, b: &Broker, gate: bool) -> Option<Value> {
    let policy = Policy {
        gate,
        ..Policy::default()
    };
    hook::handle(
        host,
        Event::Stop,
        input,
        b,
        &mut SessionState::default(),
        &policy,
    )
    .await
}

#[tokio::test]
async fn the_stop_gate_blocks_once_when_attention_is_required() {
    let (b, _fake, ws) = hook_broker(finish_fake()).await;
    let mut input = event("claude_code_stop", ws.path());

    let blocked = stop(Host::ClaudeCode, &input, &b, true).await.unwrap();
    assert_eq!(blocked["decision"], "block");
    let reason = blocked["reason"].as_str().unwrap();
    assert!(reason.chars().count() <= hook::MAX_CONTEXT_CHARS);
    let env: Value = serde_json::from_str(reason.split_once('\n').unwrap().1).unwrap();
    assert_eq!(env["status"], "attention_required");
    assert_eq!(
        env["risks"][0]["kind"], "cochange_missing",
        "the evidence travels with the block"
    );

    // The host is already continuing because of a block: never loop.
    input["stop_hook_active"] = true.into();
    let again = stop(Host::ClaudeCode, &input, &b, true).await.unwrap();
    assert!(again.get("decision").is_none(), "{again}");
    assert!(
        again["systemMessage"]
            .as_str()
            .unwrap()
            .contains("attention_required")
    );
}

#[tokio::test]
async fn without_the_gate_stop_only_warns() {
    let (b, _fake, ws) = hook_broker(finish_fake()).await;
    let input = event("codex_stop", ws.path());

    let out = stop(Host::Codex, &input, &b, false).await.unwrap();

    assert!(out.get("decision").is_none(), "{out}");
    let note = out["systemMessage"].as_str().unwrap();
    assert!(
        note.contains("attention_required") && note.contains("cochange_missing"),
        "{note}"
    );
}

#[tokio::test]
async fn an_unknown_gate_never_blocks_and_a_ready_gate_stays_silent() {
    let unknown_fake = FakeUpstream::new()
        .fail(
            "situational_awareness",
            ripwire_broker::upstream::UpstreamError::Refused("no git diff".into()),
        )
        .answer("quality_delta", "quality_delta_clean");
    let (b, _fake, ws) = hook_broker(unknown_fake).await;
    let input = event("claude_code_stop", ws.path());
    let out = stop(Host::ClaudeCode, &input, &b, true).await.unwrap();
    assert!(out.get("decision").is_none(), "{out}");
    assert!(out["systemMessage"].as_str().unwrap().contains("unknown"));

    let ready_fake = FakeUpstream::new()
        .answer("situational_awareness", "situational_awareness_clean")
        .answer("quality_delta", "quality_delta_clean");
    let (b, _fake, ws) = hook_broker(ready_fake).await;
    let input = event("claude_code_stop", ws.path());
    assert_eq!(stop(Host::ClaudeCode, &input, &b, true).await, None);
}

#[tokio::test]
async fn a_broker_failure_never_breaks_the_host() {
    let (b, _fake, ws) = hook_broker(FakeUpstream::new().down()).await;
    std::fs::write(ws.path().join("a.txt"), "x").unwrap();
    for (ev, name) in [
        (Event::UserPromptSubmit, "claude_code_user_prompt_submit"),
        (Event::PostToolUse, "claude_code_post_tool_use"),
        (Event::Stop, "claude_code_stop"),
    ] {
        let input = event(name, ws.path());
        let policy = Policy {
            gate: true,
            ..Policy::default()
        };
        let out = hook::handle(
            Host::ClaudeCode,
            ev,
            &input,
            &b,
            &mut SessionState::default(),
            &policy,
        )
        .await
        .unwrap_or_else(|| panic!("{name}: the failure is reported"));
        assert!(
            out.get("hookSpecificOutput").is_none(),
            "{name}: no context is fabricated"
        );
        assert!(
            out.get("decision").is_none(),
            "{name}: never blocks on its own failure"
        );
        let note = out["systemMessage"].as_str().unwrap();
        assert!(note.contains("upstream_unavailable"), "{name}: {note}");
    }
}

#[tokio::test]
async fn malformed_or_unrelated_events_are_ignored() {
    let (b, fake, ws) = hook_broker(edit_fake().answer("explore", "explore_export_auth")).await;
    let mut read_tool = event("claude_code_post_tool_use", ws.path());
    read_tool["tool_name"] = "Read".into();
    for (ev, input) in [
        (
            Event::UserPromptSubmit,
            serde_json::json!({"session_id": "s"}),
        ),
        (Event::UserPromptSubmit, serde_json::json!("not an object")),
        (Event::PostToolUse, serde_json::json!({})),
        (Event::PostToolUse, read_tool),
    ] {
        let out = hook::handle(
            Host::ClaudeCode,
            ev,
            &input,
            &b,
            &mut SessionState::default(),
            &Policy::default(),
        )
        .await;
        assert_eq!(out, None, "{input}");
    }
    assert!(fake.called().is_empty());
}

#[tokio::test]
async fn session_state_is_private_and_holds_no_prompt_or_code() {
    use ripwire_broker::state::StateStore;
    use std::os::unix::fs::PermissionsExt;
    let (b, _fake, ws) =
        hook_broker(FakeUpstream::new().answer("explore", "explore_export_auth")).await;
    let input = event("claude_code_user_prompt_submit", ws.path());
    let session_id = input["session_id"].as_str().unwrap();
    let mut state = SessionState::default();
    hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &input,
        &b,
        &mut state,
        &Policy::default(),
    )
    .await
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path().join("ripwire-broker"));

    store.save(session_id, &state).unwrap();

    let dir = root.path().join("ripwire-broker");
    let files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1, "no temporary file left behind: {files:?}");
    let file = &files[0];
    let name = file.file_name().unwrap().to_string_lossy().to_string();
    assert!(!name.contains(session_id), "{name}");
    assert_eq!(
        name.len(),
        64 + ".json".len(),
        "sha256 of the session id: {name}"
    );
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let text = std::fs::read_to_string(file).unwrap();
    for secret in [
        input["prompt"].as_str().unwrap(),
        "export_route",
        "src/routes.py",
        "def ",
    ] {
        assert!(!text.contains(secret), "state leaks {secret:?}: {text}");
    }
    assert_eq!(store.load(session_id), state);
    assert_eq!(store.load("another-session"), SessionState::default());
    std::fs::write(file, "{not json").unwrap();
    assert_eq!(
        store.load(session_id),
        SessionState::default(),
        "corrupt → fresh session"
    );
}

#[tokio::test]
async fn request_ids_keep_counting_across_hook_processes() {
    let fixture = || edit_fake().answer("explore", "explore_export_auth");
    let (first, _f, ws) = hook_broker(fixture()).await;
    std::fs::write(ws.path().join("a.txt"), "x").unwrap();
    let mut state = SessionState::default();
    let prompt = hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &event("claude_code_user_prompt_submit", ws.path()),
        &first,
        &mut state,
        &Policy::default(),
    )
    .await
    .unwrap();

    // Each hook event is a new process, so a new Broker, with the saved state.
    let mut config = BrokerConfig::new(ws.path());
    config.incremental = true;
    let second = Broker::connect(Arc::new(fixture()), config).await.unwrap();
    let edit = hook::handle(
        Host::ClaudeCode,
        Event::PostToolUse,
        &event("claude_code_post_tool_use", ws.path()),
        &second,
        &mut state,
        &Policy::default(),
    )
    .await
    .unwrap();

    assert_eq!(injected(&prompt)["provenance"]["request_id"], 1);
    assert_eq!(injected(&edit)["provenance"]["request_id"], 2);
    let ids: Vec<u64> = state.log.iter().map(|e| e.request_id).collect();
    assert_eq!(ids, vec![1, 2], "hook-log tells the injections apart");
}

// --- D-052 #6: a Stop that only notifies the user must not hide tests from the model ---

#[tokio::test]
async fn a_stop_notice_does_not_mark_tests_as_delivered() {
    let (b, _fake, ws) = hook_broker(finish_fake()).await;
    std::fs::write(ws.path().join("a.txt"), "x").unwrap();
    let mut state = SessionState::default();

    let notice = hook::handle(
        Host::ClaudeCode,
        Event::Stop,
        &event("claude_code_stop", ws.path()),
        &b,
        &mut state,
        &Policy::default(),
    )
    .await
    .unwrap();
    assert!(notice.get("decision").is_none(), "only a notice: {notice}");

    let edit = hook::handle(
        Host::ClaudeCode,
        Event::PostToolUse,
        &event("claude_code_post_tool_use", ws.path()),
        &b,
        &mut state,
        &Policy::default(),
    )
    .await
    .expect("the edit still has news for the model");
    let env = injected(&edit);
    assert_eq!(env["tests"][0]["path"], "tests/test_auth.py", "{env}");
}

// --- the gate notice names each risk kind once, whatever order the risks arrive in ---

#[tokio::test]
async fn the_gate_notice_never_repeats_a_risk_kind() {
    // Two regressions around one minor finding: the same kind arrives non-adjacently.
    let interleaved = r#"{"regressions":2,"r":[
        {"kind":"complexity","sym":"login","sev":"major","was":3,"now":11,"p":"src/auth.py:5"},
        {"kind":"docs","sym":"helper","sev":"minor","was":1,"now":0,"p":"src/util.py:2"},
        {"kind":"complexity","sym":"logout","sev":"major","was":2,"now":9,"p":"src/auth.py:20"}]}"#;
    let fake = FakeUpstream::new()
        .answer("situational_awareness", "situational_awareness_clean")
        .answer_text("quality_delta", interleaved);
    let (b, _fake, ws) = hook_broker(fake).await;
    let input = event("claude_code_stop", ws.path());

    let out = stop(Host::ClaudeCode, &input, &b, false).await.unwrap();

    let note = out["systemMessage"].as_str().unwrap();
    assert!(note.contains("attention_required"), "{note}");
    assert_eq!(
        note.matches("quality_regression").count(),
        1,
        "each kind once: {note}"
    );
    assert_eq!(note.matches("quality_minor").count(), 1, "{note}");
}

// --- D-106: a burst of edits is one ask, not one per edit ---

/// `Policy::default()` with a clock, so the coalescing window can be driven exactly. Tests must
/// never depend on real time (D-102).
fn at(now_ms: u64) -> Policy {
    Policy {
        now_ms: Some(now_ms),
        ..Policy::default()
    }
}

async fn post_tool_use_at(
    input: &Value,
    b: &Broker,
    state: &mut SessionState,
    policy: &Policy,
) -> Option<Value> {
    hook::handle(
        Host::ClaudeCode,
        Event::PostToolUse,
        input,
        b,
        state,
        policy,
    )
    .await
}

#[tokio::test]
async fn edits_inside_the_window_share_one_upstream_ask() {
    let (b, fake, ws) = hook_broker(edit_fake()).await;
    std::fs::write(ws.path().join("a.txt"), "hello.").unwrap();
    let input = event("claude_code_post_tool_use", ws.path());
    let mut state = SessionState::default();

    let first = post_tool_use_at(&input, &b, &mut state, &at(10_000)).await;
    assert!(first.is_some(), "the first edit of a burst is answered");
    let after_first = fake.called().len();
    assert!(after_first > 0, "and it asks upstream");

    // Two more edits, 100 ms apart, well inside the default window.
    let second = post_tool_use_at(&input, &b, &mut state, &at(10_100)).await;
    let third = post_tool_use_at(&input, &b, &mut state, &at(10_200)).await;

    assert!(
        second.is_none() && third.is_none(),
        "edits inside the window are not answered one by one"
    );
    assert_eq!(
        fake.called().len(),
        after_first,
        "and cost no upstream call at all: {:?}",
        fake.called()
    );
}

#[tokio::test]
async fn an_edit_past_the_window_is_answered_again_and_carries_what_was_held() {
    let (b, fake, ws) = hook_broker(edit_fake()).await;
    std::fs::write(ws.path().join("a.txt"), "hello.").unwrap();
    let mut state = SessionState::default();
    let one = event("claude_code_post_tool_use", ws.path());
    let mut two = one.clone();
    two["tool_input"]["file_path"] = json!(ws.path().join("b.txt").display().to_string());
    std::fs::write(ws.path().join("b.txt"), "second file").unwrap();

    post_tool_use_at(&one, &b, &mut state, &at(10_000)).await;
    // Held back: inside the window, and naming a file the first ask did not cover.
    assert!(
        post_tool_use_at(&two, &b, &mut state, &at(10_100))
            .await
            .is_none()
    );
    let held = fake.called().len();

    // Past the window, the next edit is answered and the held file goes with it.
    let later = post_tool_use_at(&one, &b, &mut state, &at(20_000)).await;

    assert!(
        later.is_some() || fake.called().len() > held,
        "the ask happens"
    );
    let files = fake
        .calls()
        .iter()
        .rfind(|(verb, _)| verb == "situational_awareness")
        .map(|(_, args)| args["files"].as_str().unwrap_or("").to_string())
        .expect("situational_awareness was asked");
    assert!(
        files.contains("b.txt"),
        "the edit held back during the window is not forgotten: {files}"
    );
}

// --- §21.3: session_hits must survive the hook process to be measurable in real use ---

/// The session counters as persisted, read through the state's serialized form so that this
/// test describes the file a later `hook-stats` reads, not an in-memory field.
fn stats(state: &SessionState) -> Value {
    serde_json::to_value(state).unwrap()["stats"].clone()
}

/// Everything an injected envelope delivered whole: items that are not mere references,
/// tests, risks and notes.
fn whole(env: &Value) -> u64 {
    let full_items = env["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| {
            !i["why_included"]
                .as_str()
                .unwrap()
                .starts_with("already delivered")
        })
        .count();
    let len = |k: &str| env[k].as_array().map_or(0, Vec::len);
    (full_items + len("tests") + len("risks") + len("notes")) as u64
}

#[tokio::test]
async fn session_hits_survive_across_hook_processes() {
    let (first, _f, ws) = hook_broker(edit_fake()).await;
    std::fs::write(ws.path().join("a.txt"), "x").unwrap();
    let input = event("claude_code_post_tool_use", ws.path());
    let mut state = SessionState::default();

    let out = post_tool_use(Host::ClaudeCode, &input, &first, &mut state)
        .await
        .expect("the first edit has news");
    let delivered = whole(&injected(&out));
    assert!(delivered > 0);

    // The next hook event is a new process: a new Broker, the saved state.
    let mut config = BrokerConfig::new(ws.path());
    config.incremental = true;
    let second = Broker::connect(Arc::new(edit_fake()), config)
        .await
        .unwrap();
    let again = post_tool_use(Host::ClaudeCode, &input, &second, &mut state).await;
    assert!(again.is_none(), "nothing new: {again:?}");

    let hits_in_second =
        serde_json::to_value(second.status().await).unwrap()["metrics"]["session_hits"]
            .as_u64()
            .unwrap();
    assert!(hits_in_second > 0, "the second process saw repeats");
    let s = stats(&state);
    assert_eq!(s["session_hits"], hits_in_second, "{s}");
    assert_eq!(s["delivered"], delivered, "{s}");
    assert_eq!(s["events"], 2, "{s}");
    assert_eq!(s["injections"], 1, "{s}");
    assert!(s["started_at"].as_u64().unwrap() > 0, "{s}");
}

#[tokio::test]
async fn only_what_reached_the_model_counts_as_delivered() {
    // A Stop without the gate is a notice for the user: the model never sees its tests.
    let (b, _fake, ws) = hook_broker(finish_fake()).await;
    std::fs::write(ws.path().join("a.txt"), "x").unwrap();
    let mut state = SessionState::default();

    let notice = hook::handle(
        Host::ClaudeCode,
        Event::Stop,
        &event("claude_code_stop", ws.path()),
        &b,
        &mut state,
        &Policy::default(),
    )
    .await
    .unwrap();
    assert!(notice.get("hookSpecificOutput").is_none(), "{notice}");

    let s = stats(&state);
    assert_eq!(s["events"], 1, "{s}");
    assert_eq!(s["injections"], 0, "{s}");
    assert_eq!(s["delivered"], 0, "{s}");
}

#[test]
fn an_old_state_file_loads_with_zero_stats() {
    // The shape written before the counters existed (D-032 .. D-106).
    let old = r#"{"memory":{"seen":["fp-1"]},"prompts_seen":1,"opted_out":false,"log":[],"next_request":2}"#;
    let state: SessionState = serde_json::from_str(old).unwrap();

    let s = stats(&state);
    for field in [
        "events",
        "injections",
        "delivered",
        "session_hits",
        "started_at",
    ] {
        assert_eq!(s[field], 0, "{field}: {s}");
    }
    assert_eq!(state.next_request, 2, "the rest of the state is untouched");
}

#[tokio::test]
async fn a_reference_to_something_already_delivered_is_a_hit_not_a_delivery() {
    let every = Policy {
        every_prompt: true,
        ..Policy::default()
    };
    let (b, _fake, ws) = hook_broker(
        FakeUpstream::new()
            .answer("explore", "explore_export_auth")
            .answer("explore", "explore_export_auth"),
    )
    .await;
    let input = event("claude_code_user_prompt_submit", ws.path());
    let mut state = SessionState::default();

    let first = hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &input,
        &b,
        &mut state,
        &every,
    )
    .await
    .expect("injected");
    let second = hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &input,
        &b,
        &mut state,
        &every,
    )
    .await
    .expect("every prompt is injected");

    let (first, second) = (injected(&first), injected(&second));
    let references = second["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| {
            i["why_included"]
                .as_str()
                .unwrap()
                .starts_with("already delivered")
        })
        .count();
    assert!(references > 0, "the second answer points back: {second}");
    let s = stats(&state);
    assert_eq!(s["delivered"], whole(&first) + whole(&second), "{s}");
    assert!(
        s["session_hits"].as_u64().unwrap() >= references as u64,
        "{s}"
    );
}

fn bound() -> SessionState {
    let mut s = SessionState::default();
    projection::bind(&mut s, "k");
    s
}

fn last(state: &SessionState) -> Option<AnalysisStatus> {
    state
        .statusline
        .as_ref()?
        .last_analysis
        .as_ref()
        .map(|a| a.status)
}

#[tokio::test]
async fn an_injection_records_the_analysis_and_the_delivery() {
    let (b, _, ws) =
        hook_broker(FakeUpstream::new().answer("explore", "explore_export_auth")).await;
    let mut state = bound();
    hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &event("claude_code_user_prompt_submit", ws.path()),
        &b,
        &mut state,
        &Policy::default(),
    )
    .await;
    let summary = state.statusline.as_ref().unwrap();
    assert_eq!(
        summary.last_analysis.as_ref().unwrap().event,
        "UserPromptSubmit"
    );
    assert!(summary.last_delivery.as_ref().unwrap().estimated_tokens > 0);
    let snap = projection::project(&state, 5).unwrap();
    assert_eq!(snap.stats.injections, state.stats.injections);
}

#[tokio::test]
async fn a_silent_ready_stop_replaces_an_earlier_attention() {
    // First Stop: attention (the gate blocks). Second: ready, which the hook answers with silence.
    let (b, _, ws) = hook_broker(finish_fake()).await;
    let mut state = bound();
    let input = event("claude_code_stop", ws.path());
    hook::handle(
        Host::ClaudeCode,
        Event::Stop,
        &input,
        &b,
        &mut state,
        &Policy {
            gate: true,
            ..Policy::default()
        },
    )
    .await;
    assert_eq!(last(&state), Some(AnalysisStatus::AttentionRequired));
    let delivered_at = state.statusline.as_ref().unwrap().last_delivery.clone();

    let (ready, _, _) = hook_broker(
        FakeUpstream::new()
            .answer("situational_awareness", "situational_awareness_clean")
            .answer("quality_delta", "quality_delta_clean"),
    )
    .await;
    let out = hook::handle(
        Host::ClaudeCode,
        Event::Stop,
        &input,
        &ready,
        &mut state,
        &Policy::default(),
    )
    .await;
    assert!(out.is_none(), "ready stays silent");
    assert_eq!(last(&state), Some(AnalysisStatus::Ready));
    assert_eq!(
        state.statusline.as_ref().unwrap().last_delivery,
        delivered_at,
        "silence delivers nothing"
    );
}

#[tokio::test]
async fn an_event_without_analysis_keeps_the_summary() {
    let (b, fake, ws) = hook_broker(finish_fake()).await;
    let mut state = bound();
    hook::handle(
        Host::ClaudeCode,
        Event::Stop,
        &event("claude_code_stop", ws.path()),
        &b,
        &mut state,
        &Policy {
            gate: true,
            ..Policy::default()
        },
    )
    .await;
    let before = state.statusline.clone();
    // A second prompt without --every-prompt asks nothing upstream.
    let calls = fake.called().len();
    state.prompts_seen = 1;
    hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &event("claude_code_user_prompt_submit", ws.path()),
        &b,
        &mut state,
        &Policy::default(),
    )
    .await;
    assert_eq!(fake.called().len(), calls);
    assert_eq!(state.statusline, before);
}

#[tokio::test]
async fn opt_out_and_opt_in_show_in_the_projection() {
    let (b, _, ws) =
        hook_broker(FakeUpstream::new().answer("explore", "explore_export_auth")).await;
    let mut state = bound();
    let mut input = event("claude_code_user_prompt_submit", ws.path());
    input["prompt"] = "stop it #ripwire-off".into();
    hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &input,
        &b,
        &mut state,
        &Policy::default(),
    )
    .await;
    assert!(projection::project(&state, 0).unwrap().opted_out);
    assert!(last(&state).is_none(), "a pause analyses nothing");
    input["prompt"] = "back #ripwire-on".into();
    hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &input,
        &b,
        &mut state,
        &Policy {
            every_prompt: true,
            ..Policy::default()
        },
    )
    .await;
    assert!(!projection::project(&state, 0).unwrap().opted_out);
}

#[tokio::test]
async fn a_broker_failure_becomes_an_error_analysis_with_its_kind_only() {
    let (b, _, ws) = hook_broker(FakeUpstream::new().fail(
        "explore",
        ripwire_broker::upstream::UpstreamError::Refused("boom /secret/path".into()),
    ))
    .await;
    let mut state = bound();
    hook::handle(
        Host::ClaudeCode,
        Event::UserPromptSubmit,
        &event("claude_code_user_prompt_submit", ws.path()),
        &b,
        &mut state,
        &Policy::default(),
    )
    .await;
    let a = state
        .statusline
        .as_ref()
        .unwrap()
        .last_analysis
        .clone()
        .unwrap();
    assert_eq!(a.status, AnalysisStatus::Error);
    let kind = a.error_kind.unwrap();
    assert!(
        !kind.contains("secret") && !kind.contains(' '),
        "a kind, not a message: {kind}"
    );
}

#[test]
fn rebinding_starts_a_new_visual_baseline_and_legacy_state_loads() {
    let legacy = r#"{"memory":{"seen":[]},"prompts_seen":3,"opted_out":false,
        "stats":{"started_at":1,"events":40,"injections":9,"delivered":30,"session_hits":12}}"#;
    let mut state: SessionState = serde_json::from_str(legacy).expect("legacy state still loads");
    assert!(state.statusline.is_none());
    projection::bind(&mut state, "k1");
    assert_eq!(
        projection::project(&state, 0).unwrap().stats.injections,
        0,
        "old totals are not this workspace's"
    );
    state.stats.injections += 2;
    assert_eq!(projection::project(&state, 0).unwrap().stats.injections, 2);
    projection::bind(&mut state, "k1");
    assert_eq!(
        projection::project(&state, 0).unwrap().stats.injections,
        2,
        "same key: no reset"
    );
    projection::bind(&mut state, "k2");
    let p = projection::project(&state, 0).unwrap();
    assert_eq!(
        (p.stats.injections, p.last_analysis.is_none()),
        (0, true),
        "another root: fresh"
    );
}

#[tokio::test]
async fn an_edit_without_news_still_updates_the_last_analysis() {
    let (first, _f, ws) = hook_broker(edit_fake()).await;
    std::fs::write(ws.path().join("a.txt"), "x").unwrap();
    let input = event("claude_code_post_tool_use", ws.path());
    let mut state = bound();
    post_tool_use(Host::ClaudeCode, &input, &first, &mut state)
        .await
        .expect("the first edit has news");
    let delivery = state.statusline.as_ref().unwrap().last_delivery.clone();
    assert!(delivery.is_some());
    // Mark the summary stale, so only a fresh analysis can clear the mark.
    state.statusline.as_mut().unwrap().last_analysis = None;

    let mut config = BrokerConfig::new(ws.path());
    config.incremental = true;
    let second = Broker::connect(Arc::new(edit_fake()), config)
        .await
        .unwrap();
    let again = post_tool_use(Host::ClaudeCode, &input, &second, &mut state).await;
    assert!(again.is_none(), "nothing new: {again:?}");
    assert!(
        last(&state).is_some(),
        "an analysis without news is still an analysis"
    );
    assert_eq!(
        state
            .statusline
            .as_ref()
            .unwrap()
            .last_analysis
            .as_ref()
            .unwrap()
            .event,
        "PostToolUse"
    );
    assert_eq!(
        state.statusline.as_ref().unwrap().last_delivery,
        delivery,
        "nothing delivered"
    );
}

/// A summary whose last analysis is a sentinel no hook would write: any run that analyses
/// something replaces it.
fn with_sentinel() -> (SessionState, projection::Analysis) {
    let mut state = bound();
    let sentinel = projection::Analysis {
        at: 1,
        event: "Sentinel".into(),
        status: AnalysisStatus::Unknown,
        error_kind: None,
    };
    state.statusline.as_mut().unwrap().last_analysis = Some(sentinel.clone());
    (state, sentinel)
}

fn sentinel_left(state: &SessionState, sentinel: &projection::Analysis, path: &str) {
    assert_eq!(
        state.statusline.as_ref().unwrap().last_analysis.as_ref(),
        Some(sentinel),
        "{path} analysed nothing, so it leaves the last analysis alone"
    );
}

#[tokio::test]
async fn paths_that_run_no_analysis_leave_the_last_analysis_untouched() {
    let (b, fake, ws) = hook_broker(edit_fake()).await;
    std::fs::write(ws.path().join("a.txt"), "x").unwrap();

    // Opted out: edits and the stop are silent.
    let (mut state, sentinel) = with_sentinel();
    state.opted_out = true;
    let edit = event("claude_code_post_tool_use", ws.path());
    assert!(
        post_tool_use(Host::ClaudeCode, &edit, &b, &mut state)
            .await
            .is_none()
    );
    let stop_event = event("claude_code_stop", ws.path());
    let policy = Policy::default();
    let stopped = hook::handle(
        Host::ClaudeCode,
        Event::Stop,
        &stop_event,
        &b,
        &mut state,
        &policy,
    );
    assert!(stopped.await.is_none());
    sentinel_left(&state, &sentinel, "an opted-out edit");

    // An edit outside the workspace never reaches the broker.
    let (mut state, sentinel) = with_sentinel();
    let mut outside = event("claude_code_post_tool_use", ws.path());
    outside["tool_input"]["file_path"] = "/etc/hosts".into();
    assert!(
        post_tool_use(Host::ClaudeCode, &outside, &b, &mut state)
            .await
            .is_none()
    );
    sentinel_left(&state, &sentinel, "an edit outside the workspace");
    assert!(fake.called().is_empty());

    // The second edit of a burst is held back, not analysed.
    let (mut state, sentinel) = with_sentinel();
    assert!(
        post_tool_use_at(&edit, &b, &mut state, &at(10_000))
            .await
            .is_some()
    );
    assert_eq!(
        state
            .statusline
            .as_ref()
            .unwrap()
            .last_analysis
            .as_ref()
            .unwrap()
            .event,
        "PostToolUse",
        "the first edit was analysed"
    );
    state.statusline.as_mut().unwrap().last_analysis = Some(sentinel.clone());
    assert!(
        post_tool_use_at(&edit, &b, &mut state, &at(10_100))
            .await
            .is_none()
    );
    sentinel_left(&state, &sentinel, "a coalesced edit");
}

#[test]
fn a_state_that_was_never_bound_serializes_without_the_status_line_field() {
    let plain = serde_json::to_value(SessionState::default()).unwrap();
    assert!(plain.get("statusline").is_none(), "{plain}");
    let bound = serde_json::to_value(bound()).unwrap();
    assert!(bound.get("statusline").is_some(), "{bound}");
}
