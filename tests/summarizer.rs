//! Seam 6: the command summarizer as a real subprocess, with `sh` scripts as the "model".
use ripwire_broker::summarizer::{CommandSummarizer, Summarizer};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn model(cmd: &str, timeout_ms: u64) -> CommandSummarizer {
    CommandSummarizer::from_command_line(cmd, Duration::from_millis(timeout_ms), None).unwrap()
}

#[tokio::test]
async fn the_command_summarizer_feeds_stdin_and_reads_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let upper = script(dir.path(), "upper", "tr a-z A-Z");

    let out = model(upper.to_str().unwrap(), 5_000)
        .summarize("a note about src")
        .await;

    assert_eq!(out.unwrap().trim(), "A NOTE ABOUT SRC");
}

#[tokio::test]
async fn arguments_reach_the_program_literally_without_a_shell() {
    let dir = tempfile::tempdir().unwrap();
    let echo = script(
        dir.path(),
        "args",
        "cat >/dev/null; for a in \"$@\"; do echo \"[$a]\"; done",
    );
    let marker = dir.path().join("pwned");
    let cmd = format!(
        "{} run;touch${{IFS}}{} $(id)",
        echo.display(),
        marker.display()
    );

    let out = model(&cmd, 5_000).summarize("x").await.unwrap();

    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        vec![
            format!("[run;touch${{IFS}}{}]", marker.display()),
            "[$(id)]".to_string()
        ]
    );
    assert!(!marker.exists(), "no shell interpreted the arguments");
}

#[tokio::test]
async fn a_hung_model_is_killed_at_the_hard_limit() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("pid");
    let hung = script(
        dir.path(),
        "hung",
        &format!("echo $$ > {}\nexec sleep 30", pid_file.display()),
    );
    let started = Instant::now();

    let err = model(hung.to_str().unwrap(), 300)
        .summarize("x")
        .await
        .unwrap_err();

    assert!(err.contains("timeout"), "{err}");
    assert!(started.elapsed() < Duration::from_secs(5));
    let pid = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .to_string();
    let mut alive = true;
    for _ in 0..50 {
        let probe = std::process::Command::new("kill")
            .args(["-0", &pid])
            .status()
            .unwrap();
        alive = probe.success();
        if !alive {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!alive, "the model process {pid} was left running");
}

#[tokio::test]
async fn a_nonzero_exit_or_a_missing_program_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let failing = script(
        dir.path(),
        "failing",
        "cat >/dev/null; echo oops >&2; exit 3",
    );

    let err = model(failing.to_str().unwrap(), 5_000)
        .summarize("x")
        .await
        .unwrap_err();
    assert!(err.contains('3'), "{err}");

    let err = model("/nonexistent/model-cli run", 5_000)
        .summarize("x")
        .await
        .unwrap_err();
    assert!(err.contains("model-cli"), "{err}");

    assert!(CommandSummarizer::from_command_line("   ", Duration::from_secs(1), None).is_err());
}

#[tokio::test]
async fn the_model_version_is_part_of_the_model_id() {
    let dir = tempfile::tempdir().unwrap();
    let cli = script(dir.path(), "llm", "cat");
    let v1 = script(dir.path(), "v1", "echo 'digest sha256:aaa'");
    let v2 = script(dir.path(), "v2", "echo 'digest sha256:bbb'");
    let with = |v: &Path| {
        CommandSummarizer::from_command_line(
            &format!("{} run phi4", cli.display()),
            Duration::from_secs(5),
            Some(v.to_str().unwrap()),
        )
        .unwrap()
    };

    let (a, b, plain) = (
        with(&v1),
        with(&v2),
        model(&format!("{} run phi4", cli.display()), 5_000),
    );

    assert_ne!(
        a.model_id(),
        b.model_id(),
        "new weights under the same tag invalidate notes"
    );
    assert_ne!(a.model_id(), plain.model_id());
    assert_eq!(a.model_id(), with(&v1).model_id());
    assert_eq!(
        a.program(),
        "llm",
        "the program's name only, for the status"
    );
    assert!(
        !a.model_id().contains("sha256:aaa"),
        "the version output is hashed"
    );
}

/// Opt-in: `RIPWIRE_BROKER_TEST_MODEL="ollama run phi4" cargo test -- --ignored`.
/// Off by default so the suite stays offline and fast (CA-10).
#[tokio::test]
#[ignore = "runs a real local model; set RIPWIRE_BROKER_TEST_MODEL"]
async fn a_real_local_model_writes_a_note() {
    let Ok(cmd) = std::env::var("RIPWIRE_BROKER_TEST_MODEL") else {
        eprintln!("skipping: RIPWIRE_BROKER_TEST_MODEL is not set");
        return;
    };
    let model = CommandSummarizer::from_command_line(&cmd, Duration::from_secs(300), None).unwrap();
    let prompt = ripwire_broker::notes::prompt(
        "src",
        "- src/auth.py#validate_token | def validate_token(token): | rank 1 | return token == \"ok\"\n\
         - src/auth.py#login | def login(user, token): | rank 2 | if not validate_token(token): raise ValueError\n",
    );

    let note = ripwire_broker::notes::sanitize(&model.summarize(&prompt).await.unwrap());

    eprintln!("note from {}: {note}", model.program());
    assert!(!note.is_empty());
    assert!(note.chars().count() <= 600);
}
