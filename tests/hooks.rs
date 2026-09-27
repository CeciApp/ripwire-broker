//! Seam 4: host hook events → broker → host output, with recorded payloads from real hosts
//! (tests/fixtures/hooks, captured from Claude Code 2.1.283 and Codex 0.157.1).
mod common;

use common::fake::FakeUpstream;
use ripwire_broker::broker::{Broker, BrokerConfig};
use ripwire_broker::cli::{Event, Host};
use ripwire_broker::hook::{self, Policy, SessionState};
use serde_json::Value;
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
