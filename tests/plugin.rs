//! The Claude Code plugin under `integrations/claude-code/` (spec/plan/mod-plan.md, D-157): the
//! shape of its JSON files, the argv its resolver builds, and the versions it pins.
//! `claude plugin validate --strict` is the authoritative check of the manifest; these tests
//! hold what it cannot know (our names, our options, our flags).

use serde_json::Value;
use std::path::{Path, PathBuf};

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
