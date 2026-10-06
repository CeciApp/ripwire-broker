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
