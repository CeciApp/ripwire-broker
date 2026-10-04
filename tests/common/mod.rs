#![allow(dead_code)]
pub mod classifier;
pub mod fake;
pub mod memory;
pub mod summarizer;
use std::path::Path;
use std::process::Command;

/// True when a `ripwire` binary is on PATH; real-upstream tests skip otherwise.
pub fn ripwire_available() -> bool {
    Command::new("ripwire").arg("--version").output().is_ok()
}

/// A tiny git repo with a caller/callee pair, a test, and one committed revision.
pub fn sample_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "src/auth.py",
        "def validate_token(token):\n    return token == \"ok\"\n\n\ndef login(user, token):\n    if not validate_token(token):\n        raise ValueError(\"bad token\")\n    return user\n",
    );
    write(
        root,
        "tests/test_auth.py",
        "from src.auth import login\n\n\ndef test_login():\n    assert login(\"a\", \"ok\") == \"a\"\n",
    );
    git(root, &["init", "-q"]);
    git(root, &["add", "."]);
    git(
        root,
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-qm",
            "init",
        ],
    );
    dir
}

pub fn write(root: &Path, rel: &str, body: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

fn git(root: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?} failed");
}

/// An executable stand-in for ripwire: the first launch runs `first_launch`
/// (a shell snippet), every later launch execs the real ripwire.
pub fn flaky_ripwire(dir: &Path, first_launch: &str) -> std::path::PathBuf {
    let marker = dir.join("launched-once");
    let script = dir.join("flaky-ripwire");
    let body = format!(
        "#!/bin/sh\nif [ ! -f '{m}' ]; then touch '{m}'; {first_launch}; fi\nexec ripwire \"$@\"\n",
        m = marker.display()
    );
    write_executable(&script, body);
    script
}

/// A stand-in `ripwire --mcp` (Python, line-delimited JSON-RPC) whose `explore` takes 30 s
/// when the task contains "slow", after creating `<dir>/busy`; everything else answers at
/// once. For cancellation tests.
pub fn slow_ripwire(dir: &Path) -> std::path::PathBuf {
    let verbs = crate::common::fake::RIPWIRE_TOOLS
        .iter()
        .map(|v| format!("\"{v}\""))
        .collect::<Vec<_>>()
        .join(",");
    let script = dir.join("slow-ripwire");
    write_executable(
        &script,
        format!(
            r#"#!/usr/bin/env python3
import json, sys, time
if "--version" in sys.argv:
    print("ripwire 0.6.4"); sys.exit(0)
for line in sys.stdin:
    msg = json.loads(line)
    if "id" not in msg:
        continue
    if msg.get("method") == "tools/list":
        result = {{"tools": [{{"name": v, "inputSchema": {{"type": "object"}}}} for v in [{verbs}]]}}
    else:
        args = msg.get("params", {{}}).get("arguments", {{}})
        if "slow" in str(args.get("task", "")):
            open("{busy}", "w").close()
            time.sleep(30)
        result = {{"content": [{{"type": "text", "text": "<ctx></ctx>"}}]}}
    sys.stdout.write(json.dumps({{"jsonrpc": "2.0", "id": msg["id"], "result": result}}) + "\n")
    sys.stdout.flush()
"#,
            busy = dir.join("busy").display()
        ),
    );
    script
}

/// Writes an executable script without this process ever holding it open for writing. On
/// Linux, when another test thread forks while a write descriptor is open, the child inherits
/// it until its own `exec`, and executing the script meanwhile fails with ETXTBSY ("Text file
/// busy"). A short-lived `sh` writes the file instead, so no writer can leak into a fork.
pub fn write_executable(path: &Path, body: impl AsRef<str>) {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new("sh")
        .args(["-c", "cat > \"$1\" && chmod 755 \"$1\"", "sh"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(body.as_ref().as_bytes())
        .unwrap();
    assert!(
        child.wait().unwrap().success(),
        "could not write {}",
        path.display()
    );
}

/// A stand-in `ripwire` that records every `--version` invocation in `counter`, and otherwise
/// speaks just enough MCP over stdio to let a broker connect.
pub fn counting_ripwire(dir: &Path, counter: &Path) -> std::path::PathBuf {
    let verbs = crate::common::fake::RIPWIRE_TOOLS
        .iter()
        .map(|v| format!("\"{v}\""))
        .collect::<Vec<_>>()
        .join(",");
    let script = dir.join("counting-ripwire");
    write_executable(
        &script,
        format!(
            r#"#!/usr/bin/env python3
import json, sys
if "--version" in sys.argv:
    open("{counter}", "a").write("asked\n")
    print("ripwire 0.6.5"); sys.exit(0)
open("{counter}.launches", "a").write("launched\n")
for line in sys.stdin:
    msg = json.loads(line)
    if "id" not in msg:
        continue
    if msg.get("method") == "tools/list":
        result = {{"tools": [{{"name": v, "inputSchema": {{"type": "object"}}}} for v in [{verbs}]]}}
    else:
        result = {{"content": [{{"type": "text", "text": "<ctx></ctx>"}}]}}
    sys.stdout.write(json.dumps({{"jsonrpc": "2.0", "id": msg["id"], "result": result}}) + "\n")
    sys.stdout.flush()
"#,
            counter = counter.display()
        ),
    );
    script
}
