//! The Claude Code plugin under `integrations/claude-code/` (spec/plan/mod-plan.md, D-157): the
//! shape of its JSON files, the argv its resolver builds, and the versions it pins.
//! `claude plugin validate --strict` is the authoritative check of the manifest; these tests
//! hold what it cannot know (our names, our options, our flags).
mod common;

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn plugin_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("integrations/claude-code")
}

fn read_json(rel: &str) -> Value {
    let path = plugin_root().join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn manifest() -> Value {
    read_json(".claude-plugin/plugin.json")
}

#[test]
fn the_manifest_names_the_plugin_and_passes_the_anthropic_name_rules() {
    let m = manifest();
    assert_eq!(m["name"], "ripwire-broker");
    for key in ["version", "description", "license"] {
        assert!(m[key].as_str().is_some_and(|s| !s.is_empty()), "{key}: {m}");
    }
    assert!(m["author"]["name"].as_str().is_some_and(|s| !s.is_empty()));
    for key in ["homepage", "repository"] {
        let url = m[key].as_str().unwrap_or_default();
        assert!(
            url.starts_with("https://") && !url.contains(char::is_whitespace) && url.len() > 8,
            "{key} is not a URL: {url:?}"
        );
    }
    assert!(
        m["keywords"]
            .as_array()
            .is_some_and(|k| !k.is_empty() && k.iter().all(Value::is_string))
    );
    // The default layout (skills/, hooks/hooks.json, .mcp.json) needs no component keys.
    for key in ["hooks", "mcpServers", "skills", "commands", "agents"] {
        assert!(m.get(key).is_none(), "component key {key} in the manifest");
    }
}

/// A sandbox for `scripts/broker`: a plugin root that pins `pinned` in its `checksums.txt`, an
/// empty `${CLAUDE_PLUGIN_DATA}`, a `PATH` with only the system tools, and a workspace to run in.
/// Every fake broker it installs records what it received in `out/`.
struct Resolver {
    dir: tempfile::TempDir,
    path: Vec<PathBuf>,
}

impl Resolver {
    fn new(pinned: &str) -> Resolver {
        let dir = tempfile::tempdir().unwrap();
        for sub in ["root/scripts", "data", "ws", "out", "pathbin"] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        std::fs::write(
            dir.path().join("root/scripts/checksums.txt"),
            format!("{pinned}\n"),
        )
        .unwrap();
        Resolver {
            dir,
            path: vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")],
        }
    }

    fn at(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    /// A fake `ripwire-broker` at `rel` that answers `--version` with `version`, and otherwise
    /// records its own path, its argv and the Jev key it inherited.
    fn fake(&self, rel: &str, version: &str) -> PathBuf {
        let path = self.at(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let out = self.at("out");
        common::write_executable(
            &path,
            format!(
                r#"#!/bin/sh
if [ "$1" = "--version" ]; then echo "ripwire-broker {version}"; exit 0; fi
printf '%s\n' "$0" "$@" > "{out}/argv"
if [ "${{RIPWIRE_BROKER_JEV_API_KEY+set}}" = set ]; then
  printf '%s' "$RIPWIRE_BROKER_JEV_API_KEY" > "{out}/key"
fi
env | grep -E '^(RIPWIRE_BROKER_PLUGIN|CLAUDE_PLUGIN_OPTION)_JEV_API_KEY=' > "{out}/leak" || true
"#,
                out = out.display()
            ),
        );
        path
    }

    /// A fake in a directory of its own, put first on `PATH`.
    fn fake_on_path(&mut self, version: &str) -> PathBuf {
        let path = self.fake("pathbin/ripwire-broker", version);
        self.path.insert(0, self.at("pathbin"));
        path
    }

    fn run(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let path = std::env::join_paths(&self.path).unwrap();
        Command::new(plugin_root().join("scripts/broker"))
            .args(args)
            .env_clear()
            .env("PATH", path)
            .env("CLAUDE_PLUGIN_ROOT", self.at("root"))
            .env("CLAUDE_PLUGIN_DATA", self.at("data"))
            .envs(env.iter().copied())
            .current_dir(self.at("ws"))
            .output()
            .unwrap()
    }

    /// What the fake that ran received: its own path first, then its argv.
    fn argv(&self) -> Option<Vec<String>> {
        let text = std::fs::read_to_string(self.at("out/argv")).ok()?;
        Some(text.lines().map(str::to_string).collect())
    }

    fn key(&self) -> Option<String> {
        std::fs::read_to_string(self.at("out/key")).ok()
    }

    /// The option's own variables as the child saw them; the key must travel only as
    /// `RIPWIRE_BROKER_JEV_API_KEY`.
    fn leak(&self) -> String {
        std::fs::read_to_string(self.at("out/leak")).unwrap_or_default()
    }

    fn reset(&self) {
        for f in ["argv", "key", "leak"] {
            let _ = std::fs::remove_file(self.at("out").join(f));
        }
    }
}

fn ran(r: &Resolver) -> PathBuf {
    PathBuf::from(&r.argv().expect("no broker ran")[0])
}

fn args_of(r: &Resolver) -> Vec<String> {
    r.argv().expect("no broker ran")[1..].to_vec()
}

const HOOK: [&str; 3] = ["hook", "claude-code", "user-prompt-submit"];

#[test]
fn the_resolver_picks_the_binary_in_order_and_maps_options_to_flags() {
    let mut r = Resolver::new("v0.2.0");
    let on_path = r.fake_on_path("0.2.0");
    // A binary left by the previous release is never chosen: only the pinned version counts.
    r.fake("data/bin/0.1.0/ripwire-broker", "0.1.0");
    assert!(r.run(&HOOK, &[]).status.success());
    assert_eq!(ran(&r), on_path);

    r.reset();
    let pinned = r.fake("data/bin/0.2.0/ripwire-broker", "0.2.0");
    assert!(r.run(&HOOK, &[]).status.success());
    assert_eq!(ran(&r), pinned);

    r.reset();
    let chosen = r.fake("chosen/ripwire-broker", "0.2.0");
    let chosen_text = chosen.display().to_string();
    assert!(
        r.run(&HOOK, &[("CLAUDE_PLUGIN_OPTION_BINARY", &chosen_text)])
            .status
            .success()
    );
    assert_eq!(ran(&r), chosen);
    // No options, no flags; and a hook follows the event's cwd, never a fixed --workspace.
    assert_eq!(args_of(&r), HOOK);
}

#[test]
fn the_resolver_turns_hook_options_into_flags_in_any_spelling() {
    let mut r = Resolver::new("v0.1.0");
    r.fake_on_path("0.1.0");
    for on in ["true", "1", "yes", "TRUE", "Yes"] {
        r.reset();
        let out = r.run(
            &HOOK,
            &[
                ("CLAUDE_PLUGIN_OPTION_MEMORY", on),
                ("CLAUDE_PLUGIN_OPTION_EVERY_PROMPT", on),
                ("CLAUDE_PLUGIN_OPTION_GATE", on),
            ],
        );
        assert!(out.status.success());
        assert_eq!(
            args_of(&r),
            [&HOOK[..], &["--memory", "--every-prompt", "--gate"]].concat(),
            "{on}"
        );
    }
    for off in ["false", "0", "", "no"] {
        r.reset();
        r.run(&HOOK, &[("CLAUDE_PLUGIN_OPTION_MEMORY", off)]);
        assert_eq!(args_of(&r), HOOK, "{off:?}");
    }
}

#[test]
fn the_resolver_turns_server_options_into_flags_and_finds_the_workspace() {
    let mut r = Resolver::new("v0.1.0");
    r.fake_on_path("0.1.0");
    // Without CLAUDE_PROJECT_DIR the server serves the directory it was started in.
    assert!(r.run(&["serve"], &[]).status.success());
    let ws = r.at("ws").canonicalize().unwrap().display().to_string();
    let got = args_of(&r);
    assert_eq!(got[0], "serve");
    assert_eq!(got[1], "--workspace");
    assert_eq!(
        PathBuf::from(&got[2])
            .canonicalize()
            .unwrap()
            .display()
            .to_string(),
        ws
    );
    assert_eq!(got.len(), 3);

    r.reset();
    r.run(
        &["serve"],
        &[
            ("CLAUDE_PROJECT_DIR", "/project"),
            ("RIPWIRE_BROKER_PLUGIN_ONLINE", "true"),
            ("RIPWIRE_BROKER_PLUGIN_INCREMENTAL", "1"),
        ],
    );
    assert_eq!(
        args_of(&r),
        [
            "serve",
            "--workspace",
            "/project",
            "--online",
            "--incremental"
        ]
    );

    // --memory implies online on the server: no redundant --online.
    r.reset();
    r.run(
        &["serve"],
        &[
            ("CLAUDE_PROJECT_DIR", "/project"),
            ("RIPWIRE_BROKER_PLUGIN_ONLINE", "yes"),
            ("RIPWIRE_BROKER_PLUGIN_MEMORY", "yes"),
        ],
    );
    assert_eq!(
        args_of(&r),
        ["serve", "--workspace", "/project", "--memory"]
    );
}

#[test]
fn the_resolver_only_replaces_the_jev_key_with_a_non_empty_option() {
    let mut r = Resolver::new("v0.1.0");
    r.fake_on_path("0.1.0");
    let serve = ["serve"];
    // The option wins over the shell's key.
    r.run(
        &serve,
        &[
            ("RIPWIRE_BROKER_PLUGIN_JEV_API_KEY", "from-option"),
            ("RIPWIRE_BROKER_JEV_API_KEY", "from-shell"),
        ],
    );
    assert_eq!(r.key().as_deref(), Some("from-option"));
    // An empty option (what `${user_config.jev_api_key}` gives when unset) keeps the shell's.
    r.reset();
    r.run(
        &serve,
        &[
            ("RIPWIRE_BROKER_PLUGIN_JEV_API_KEY", ""),
            ("RIPWIRE_BROKER_JEV_API_KEY", "from-shell"),
        ],
    );
    assert_eq!(r.key().as_deref(), Some("from-shell"));
    // Both empty: the child gets no key at all, never "" (D-155 would call it malformed).
    r.reset();
    r.run(
        &serve,
        &[
            ("RIPWIRE_BROKER_PLUGIN_JEV_API_KEY", ""),
            ("RIPWIRE_BROKER_JEV_API_KEY", ""),
        ],
    );
    assert_eq!(r.key(), None);
    // The option's own variables never reach the child, in a hook either.
    r.reset();
    r.run(
        &HOOK,
        &[
            ("CLAUDE_PLUGIN_OPTION_JEV_API_KEY", "from-option"),
            ("RIPWIRE_BROKER_PLUGIN_JEV_API_KEY", "from-option"),
        ],
    );
    assert!(r.argv().is_some());
    assert_eq!(r.leak(), "");
}

#[test]
fn the_resolver_warns_when_a_chosen_binary_is_not_the_pinned_version_and_runs_it() {
    let mut r = Resolver::new("v0.2.0");
    let on_path = r.fake_on_path("0.1.0");
    let out = r.run(&HOOK, &[]);
    assert!(out.status.success());
    assert_eq!(ran(&r), on_path);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("0.1.0") && err.contains("0.2.0"), "{err}");

    r.reset();
    let chosen = r.fake("chosen/ripwire-broker", "0.3.0");
    let out = r.run(
        &HOOK,
        &[("CLAUDE_PLUGIN_OPTION_BINARY", &chosen.display().to_string())],
    );
    assert_eq!(ran(&r), chosen);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("0.3.0") && err.contains("0.2.0"), "{err}");

    // The pinned version is quiet.
    r.reset();
    r.fake("data/bin/0.2.0/ripwire-broker", "0.2.0");
    let out = r.run(&HOOK, &[]);
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_resolver_runs_no_hook_while_the_mod_is_active() {
    let mut r = Resolver::new("v0.1.0");
    r.fake_on_path("0.1.0");
    let out = r.run(&HOOK, &[("RIPWIRE_BROKER_MOD_ACTIVE", "1")]);
    assert!(out.status.success());
    assert_eq!(r.argv(), None);
    // The server still runs: the mod talks to it.
    r.run(&["serve"], &[("RIPWIRE_BROKER_MOD_ACTIVE", "1")]);
    assert!(r.argv().is_some());
}

#[test]
fn a_missing_binary_lets_a_hook_pass_and_stops_the_server() {
    let r = Resolver::new("v0.2.0");
    r.fake("data/bin/0.1.0/ripwire-broker", "0.1.0");
    let out = r.run(&HOOK, &[]);
    assert!(
        out.status.success(),
        "a failing hook would block the session"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("install-binary.sh") && err.contains("0.2.0"),
        "{err}"
    );
    let out = r.run(&["serve"], &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("install-binary.sh"));
    assert_eq!(r.argv(), None, "the old release never runs");
}

#[test]
fn user_config_declares_consent_options_with_the_install_texts() {
    use ripwire_broker::install::{MEMORY_CONSENT, ONLINE_CONSENT};
    let config = manifest()["userConfig"].clone();
    let options = config.as_object().expect("userConfig");
    let mut keys: Vec<&str> = options.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "binary",
            "every_prompt",
            "gate",
            "incremental",
            "jev_api_key",
            "memory",
            "online"
        ]
    );
    for (key, o) in options {
        assert!(
            key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !key.starts_with(|c: char| c.is_ascii_digit()),
            "{key}"
        );
        // Both are required by the manifest reference.
        for field in ["title", "description"] {
            assert!(
                o[field].as_str().is_some_and(|s| !s.is_empty()),
                "{key}.{field}"
            );
        }
    }
    let boolean = |key: &str, default: bool| {
        assert_eq!(config[key]["type"], "boolean", "{key}");
        assert_eq!(config[key]["default"], default, "{key}");
    };
    // Consent stays off by default, and says what the install says is sent to Jev.
    boolean("online", false);
    assert_eq!(config["online"]["description"], ONLINE_CONSENT);
    boolean("memory", false);
    assert_eq!(config["memory"]["description"], MEMORY_CONSENT);
    // The server dedups per session what the agent and the mod ask it (mod-plan §2.3).
    boolean("incremental", true);
    boolean("every_prompt", false);
    boolean("gate", false);
    let key = &config["jev_api_key"];
    assert_eq!(key["type"], "string");
    assert_eq!(key["sensitive"], true, "the key goes to secure storage");
    assert_ne!(key["required"], true);
    assert!(key.get("default").is_none());
    assert_eq!(config["binary"]["type"], "file");
    assert_ne!(config["binary"]["required"], true);
}

#[test]
fn hooks_json_mirrors_the_install_events_in_exec_form() {
    use ripwire_broker::cli::Host;
    use ripwire_broker::install;
    let file = read_json("hooks/hooks.json");
    let events = file["hooks"].as_object().expect("the \"hooks\" envelope");
    let expected = install::events(Host::ClaudeCode);
    let classic: Vec<&str> = expected.iter().map(|(event, ..)| *event).collect();
    for name in events.keys() {
        assert!(
            classic.contains(&name.as_str()) || name == "SessionStart",
            "unexpected event {name}"
        );
    }
    for (event, arg, matcher) in expected {
        let groups = events[event]
            .as_array()
            .unwrap_or_else(|| panic!("{event}"));
        assert_eq!(groups.len(), 1, "{event}");
        assert_eq!(groups[0].get("matcher").and_then(Value::as_str), matcher);
        // Exec form: no shell, no quoting, and the options arrive as CLAUDE_PLUGIN_OPTION_*.
        // No --workspace (the hook follows the event's cwd) and no --memory (an option).
        assert_eq!(
            groups[0]["hooks"],
            serde_json::json!([{
                "type": "command",
                "command": "${CLAUDE_PLUGIN_ROOT}/scripts/broker",
                "args": ["hook", "claude-code", arg],
                "timeout": 60
            }]),
            "{event}"
        );
    }
    assert!(
        !plugin_root().join("settings.json").exists(),
        "the old hand-written example is gone"
    );
}

#[test]
fn mcp_json_runs_the_resolver_and_passes_options_through_env() {
    let file = read_json(".mcp.json");
    let servers = file["mcpServers"].as_object().expect("mcpServers");
    // `broker` gives plugin:ripwire-broker:broker and mcp__plugin_ripwire-broker_broker__* (DM-3).
    assert_eq!(servers.keys().collect::<Vec<_>>(), ["broker"]);
    let server = &servers["broker"];
    assert_eq!(server["command"], "${CLAUDE_PLUGIN_ROOT}/scripts/broker");
    assert_eq!(server["args"], serde_json::json!(["serve"]));
    // The key travels in a variable of its own: written straight into
    // RIPWIRE_BROKER_JEV_API_KEY, an empty option would overwrite the shell's key with ""
    // (mod-plan §2.3). The resolver decides.
    assert_eq!(
        server["env"],
        serde_json::json!({
            "RIPWIRE_BROKER_PLUGIN_JEV_API_KEY": "${user_config.jev_api_key}",
            "RIPWIRE_BROKER_PLUGIN_ONLINE": "${user_config.online}",
            "RIPWIRE_BROKER_PLUGIN_MEMORY": "${user_config.memory}",
            "RIPWIRE_BROKER_PLUGIN_INCREMENTAL": "${user_config.incremental}",
            "RIPWIRE_BROKER_PLUGIN_BINARY": "${user_config.binary}"
        })
    );
    assert!(
        !plugin_root().join("mcp.json").exists(),
        "the old hand-written example is gone"
    );
}

/// A release asset as T2.1 publishes it: a tarball with the broker inside, named after its tag
/// and target. Returns the asset's file name and its SHA-256.
fn release_asset(dir: &Path, tag: &str, target: &str, says: &str) -> (String, String) {
    use sha2::{Digest, Sha256};
    let stage = dir.join(format!("stage-{tag}-{says}"));
    std::fs::create_dir_all(&stage).unwrap();
    common::write_executable(
        &stage.join("ripwire-broker"),
        format!("#!/bin/sh\necho \"ripwire-broker {says}\"\n"),
    );
    let name = format!("ripwire-broker-{tag}-{target}.tar.gz");
    std::fs::create_dir_all(dir.join("from")).unwrap();
    let status = Command::new("tar")
        .arg("-czf")
        .arg(dir.join("from").join(&name))
        .arg("-C")
        .arg(&stage)
        .arg("ripwire-broker")
        .status()
        .unwrap();
    assert!(status.success());
    let bytes = std::fs::read(dir.join("from").join(&name)).unwrap();
    let sha = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    (name, sha)
}

const TARGET: &str = "x86_64-unknown-linux-gnu";

impl Resolver {
    fn install_binary(&self, args: &[&str]) -> Output {
        Command::new("sh")
            .arg(plugin_root().join("scripts/install-binary.sh"))
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("CLAUDE_PLUGIN_ROOT", self.at("root"))
            .env("CLAUDE_PLUGIN_DATA", self.at("data"))
            .output()
            .unwrap()
    }

    fn pin(&self, tag: &str, assets: &[(String, String)]) {
        let mut text = format!("{tag}\n");
        for (name, sha) in assets {
            text.push_str(&format!("{sha}  {name}\n"));
        }
        std::fs::write(self.at("root/scripts/checksums.txt"), text).unwrap();
    }
}

#[test]
fn install_binary_refuses_a_checksum_mismatch_and_writes_to_plugin_data() {
    let r = Resolver::new("v0.2.0");
    std::fs::create_dir_all(r.at("data/bin/0.1.0")).unwrap();
    std::fs::write(r.at("data/bin/0.1.0/ripwire-broker"), "old").unwrap();
    let asset = release_asset(r.dir.path(), "v0.2.0", TARGET, "0.2.0");
    r.pin("v0.2.0", std::slice::from_ref(&asset));
    let from = r.at("from").display().to_string();

    let out = r.install_binary(&["--from", &from, "--target", TARGET]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let installed = r.at("data/bin/0.2.0/ripwire-broker");
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(&installed).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755);
    let says = Command::new(&installed).output().unwrap().stdout;
    assert_eq!(
        String::from_utf8_lossy(&says).trim(),
        "ripwire-broker 0.2.0"
    );
    // Earlier versions stay until --prune.
    assert!(r.at("data/bin/0.1.0/ripwire-broker").exists());

    // An asset whose hash is not the pinned one is refused, and leaves nothing behind.
    let r = Resolver::new("v0.3.0");
    let (name, _) = release_asset(r.dir.path(), "v0.3.0", TARGET, "tampered");
    r.pin("v0.3.0", &[(name, "0".repeat(64))]);
    let from = r.at("from").display().to_string();
    let out = r.install_binary(&["--from", &from, "--target", TARGET]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("SHA-256"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut left = vec![];
    for entry in walk(&r.at("data")) {
        left.push(entry.display().to_string());
    }
    assert!(left.is_empty(), "files left behind: {left:?}");

    // A target the release does not list is refused before any download.
    let out = r.install_binary(&["--from", &from, "--target", "riscv64-unknown-linux-gnu"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("riscv64"));
}

/// Every file under `dir`, recursively.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut files = vec![];
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => files.extend(walk(&path)),
            false => files.push(path),
        }
    }
    files
}

#[test]
fn install_binary_prune_removes_only_the_other_versions() {
    let r = Resolver::new("v0.2.0");
    for dir in ["data/bin/0.1.0", "data/bin/0.2.0", "data/other"] {
        std::fs::create_dir_all(r.at(dir)).unwrap();
        std::fs::write(r.at(dir).join("ripwire-broker"), "x").unwrap();
    }
    let out = r.install_binary(&["--prune"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!r.at("data/bin/0.1.0").exists());
    assert!(r.at("data/bin/0.2.0/ripwire-broker").exists());
    assert!(
        r.at("data/other/ripwire-broker").exists(),
        "never outside bin/"
    );
}

#[test]
fn checksums_pin_a_tag_and_list_one_sha_per_asset() {
    let text = std::fs::read_to_string(plugin_root().join("scripts/checksums.txt")).unwrap();
    let mut lines = text.lines();
    let tag = lines.next().unwrap();
    assert!(
        tag.starts_with('v') && tag[1..].split('.').count() == 3,
        "first line is the release tag: {tag:?}"
    );
    for line in lines {
        let (sha, name) = line.split_once("  ").expect("sha256  asset");
        assert!(sha.len() == 64 && sha.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(name.starts_with(&format!("ripwire-broker-{tag}-")) && name.ends_with(".tar.gz"));
    }
}

#[test]
fn plugin_version_equals_the_pinned_release() {
    let text = std::fs::read_to_string(plugin_root().join("scripts/checksums.txt")).unwrap();
    let tag = text.lines().next().unwrap();
    assert_eq!(
        manifest()["version"].as_str(),
        tag.strip_prefix('v'),
        "plugin.json.version is the release in checksums.txt (DM-6)"
    );
}

#[test]
fn session_start_check_says_what_is_missing_in_one_line_and_exits_zero() {
    let hooks = read_json("hooks/hooks.json");
    assert_eq!(
        hooks["hooks"]["SessionStart"],
        serde_json::json!([{"hooks": [{
            "type": "command",
            "command": "${CLAUDE_PLUGIN_ROOT}/scripts/broker",
            "args": ["check"],
            "timeout": 10
        }]}])
    );

    let mut r = Resolver::new("v0.2.0");
    // A binary of the previous release does not count (mod-plan §2.3).
    r.fake("data/bin/0.1.0/ripwire-broker", "0.1.0");
    // A `curl` that tells on itself: the check never goes to the network.
    common::write_executable(
        &r.at("pathbin/curl"),
        format!("#!/bin/sh\ntouch {}/network\n", r.at("out").display()),
    );
    r.path.insert(0, r.at("pathbin"));
    let check = |r: &Resolver| {
        let out = r.run(&["check"], &[]);
        assert!(out.status.success());
        assert!(
            out.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };

    let said = check(&r);
    assert_eq!(said.lines().count(), 1, "{said}");
    assert!(
        said.contains("install-binary.sh") && said.contains("0.2.0"),
        "{said}"
    );
    assert!(said.contains("ripwire"), "ripwire is missing too: {said}");
    // Outside a hook, Claude Code exports no CLAUDE_PLUGIN_DATA: the command carries it, so the
    // binary lands where this plugin looks for it.
    assert!(
        said.contains(&format!("CLAUDE_PLUGIN_DATA='{}'", r.at("data").display())),
        "{said}"
    );

    r.fake("data/bin/0.2.0/ripwire-broker", "0.2.0");
    let said = check(&r);
    assert_eq!(said.lines().count(), 1, "{said}");
    assert!(!said.contains("install-binary.sh"), "{said}");
    assert!(said.contains("ripwire"), "{said}");

    common::write_executable(&r.at("pathbin/ripwire"), "#!/bin/sh\n");
    assert_eq!(check(&r), "");
    assert_eq!(r.argv(), None, "the check runs no broker");
    assert!(!r.at("out/network").exists());
}

#[test]
fn the_repository_is_the_marketplace_aquental_listing_the_plugin() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".claude-plugin/marketplace.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let market: Value = serde_json::from_str(&text).unwrap();
    // `claude plugin install ripwire-broker@aquental` (DM-4).
    assert_eq!(market["name"], "aquental");
    assert!(
        market["owner"]["name"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
    );
    let plugins = market["plugins"].as_array().unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0]["name"], manifest()["name"]);
    assert_eq!(plugins[0]["source"], "./integrations/claude-code");
    // The version lives in plugin.json only; in both, validate warns and plugin.json wins.
    assert!(plugins[0].get("version").is_none());
}

fn semver(text: &str) -> (u64, u64, u64) {
    let mut parts = text.split('.').map(|p| p.parse::<u64>().unwrap());
    let mut next = || parts.next().unwrap_or_else(|| panic!("not x.y.z: {text}"));
    (next(), next(), next())
}

#[test]
fn cargo_version_is_not_behind_the_plugin_version() {
    // Release order (mod-plan §6): Cargo.toml goes up first, then the tag, then the plugin.
    let plugin = manifest()["version"].as_str().unwrap().to_string();
    assert!(
        semver(env!("CARGO_PKG_VERSION")) >= semver(&plugin),
        "the plugin pins {plugin}, which the code ({}) has not reached",
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn an_update_with_the_old_binary_present_runs_nothing_old() {
    // Before the update: v0.1.0 pinned and installed.
    let r = Resolver::new("v0.1.0");
    r.fake("data/bin/0.1.0/ripwire-broker", "0.1.0");
    r.run(&["serve"], &[]);
    assert_eq!(ran(&r), r.at("data/bin/0.1.0/ripwire-broker"));
    // The update brings a plugin pinned to v0.2.0; ${CLAUDE_PLUGIN_DATA} keeps 0.1.0.
    r.reset();
    r.pin("v0.2.0", &[]);
    let check = r.run(&["check"], &[]);
    assert!(String::from_utf8_lossy(&check.stdout).contains("install-binary.sh"));
    assert_eq!(r.run(&["serve"], &[]).status.code(), Some(1));
    assert!(r.run(&HOOK, &[]).status.success());
    assert_eq!(r.argv(), None, "0.1.0 never runs after the update");
}

/// The targets `install-binary.sh` maps this machine to: the `) target=TRIPLE ;;` arms.
fn installer_targets() -> Vec<String> {
    let script = std::fs::read_to_string(plugin_root().join("scripts/install-binary.sh")).unwrap();
    let targets: Vec<String> = script
        .lines()
        .filter_map(|l| {
            l.split_once(") target=")?
                .1
                .split_once(" ;;")
                .map(|(t, _)| t.to_string())
        })
        .filter(|t| {
            t.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
        .collect();
    assert_eq!(targets.len(), 4, "{targets:?}");
    targets
}

#[test]
fn the_release_workflow_publishes_what_install_binary_downloads() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/release.yml");
    let yml = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(yml.contains("tags: [ \"v*\" ]"), "runs on a v* tag");
    for target in installer_targets() {
        assert!(
            yml.contains(&format!("target: {target}")),
            "no build for {target}"
        );
    }
    // The asset name install-binary.sh asks for, and the broker at the top of the tarball.
    assert!(
        yml.contains("ripwire-broker-$tag-$target.tar.gz"),
        "asset name"
    );
    assert!(
        yml.contains("ripwire-broker ripwire-eval"),
        "both binaries in the tarball"
    );
    assert!(
        yml.contains("--features online"),
        "the release carries the classifier"
    );
    assert!(yml.contains("SHA256SUMS"));
    // The gates run before anything is published; the release stays a draft until the end.
    assert!(yml.contains("uses: ./.github/workflows/rust.yml"));
    assert!(yml.contains("--draft") && yml.contains("--draft=false"));
}
