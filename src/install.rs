//! `install` (D-033): wires the broker into a host. A dry run unless `--write`; merges JSON
//! idempotently, keeps foreign keys and hooks, and backs up any file it changes. Codex's
//! `config.toml` is only printed: editing TOML without a parser could damage it.

use crate::cli::{Host, InstallArgs};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// One file the install would create or change.
pub struct Change {
    pub path: PathBuf,
    pub before: Option<String>,
    pub after: String,
}

pub struct Plan {
    pub changes: Vec<Change>,
    /// Text for the user to apply by hand (the Codex TOML).
    pub notes: Vec<String>,
}

fn events(host: Host) -> [(&'static str, &'static str, Option<&'static str>); 3] {
    let edits = match host {
        Host::ClaudeCode => "Edit|Write|MultiEdit|NotebookEdit",
        Host::Codex => "apply_patch|Edit|Write",
    };
    [
        ("UserPromptSubmit", "user-prompt-submit", None),
        ("PostToolUse", "post-tool-use", Some(edits)),
        ("Stop", "stop", None),
    ]
}

/// A hook command this broker installed, whatever binary path it had then.
fn is_ours(hook: &Value) -> bool {
    hook.get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains("ripwire-broker") && c.contains(" hook "))
}

fn quote(p: &Path) -> String {
    let s = p.display().to_string();
    if s.contains(char::is_whitespace) {
        format!("'{s}'")
    } else {
        s
    }
}

/// Replaces this broker's hooks in `settings` and keeps everything else.
fn merge_hooks(mut settings: Value, host: Host, binary: &Path, workspace: Option<&Path>) -> Value {
    let host_name = match host {
        Host::ClaudeCode => "claude-code",
        Host::Codex => "codex",
    };
    if !settings.is_object() {
        settings = json!({});
    }
    let hooks = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks = hooks.as_object_mut().unwrap();
    for (event, arg, matcher) in events(host) {
        let groups = hooks.entry(event).or_insert_with(|| json!([]));
        if !groups.is_array() {
            *groups = json!([]);
        }
        let groups = groups.as_array_mut().unwrap();
        for g in groups.iter_mut() {
            if let Some(list) = g.get_mut("hooks").and_then(Value::as_array_mut) {
                list.retain(|h| !is_ours(h));
            }
        }
        groups.retain(|g| {
            g.get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|l| !l.is_empty())
        });
        let mut command = format!("{} hook {host_name} {arg}", quote(binary));
        if let Some(ws) = workspace {
            command.push_str(&format!(" --workspace {}", quote(ws)));
        }
        let mut group = Map::new();
        if let Some(m) = matcher {
            group.insert("matcher".into(), m.into());
        }
        group.insert(
            "hooks".into(),
            json!([{"type": "command", "command": command, "timeout": 60}]),
        );
        groups.push(Value::Object(group));
    }
    settings
}

fn read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn parse(text: &Option<String>) -> Result<Value, String> {
    match text {
        None => Ok(json!({})),
        Some(t) => {
            serde_json::from_str(t).map_err(|e| format!("not valid JSON ({e}); left untouched"))
        }
    }
}

fn pretty(v: &Value) -> String {
    format!("{}\n", serde_json::to_string_pretty(v).unwrap_or_default())
}

fn change(path: PathBuf, edit: impl FnOnce(Value) -> Value) -> Result<Change, String> {
    let before = read(&path);
    let value = parse(&before).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Change {
        after: pretty(&edit(value)),
        path,
        before,
    })
}

pub fn plan(args: &InstallArgs, binary: &Path) -> Result<Plan, String> {
    let workspace = args
        .workspace
        .canonicalize()
        .map_err(|e| format!("workspace {}: {e}", args.workspace.display()))?;
    let mut plan = Plan {
        changes: vec![],
        notes: vec![],
    };
    match args.host {
        Host::ClaudeCode => {
            plan.changes
                .push(change(workspace.join(".mcp.json"), |mut v| {
                    if !v.is_object() {
                        v = json!({});
                    }
                    let servers = v
                        .as_object_mut()
                        .unwrap()
                        .entry("mcpServers")
                        .or_insert_with(|| json!({}));
                    if !servers.is_object() {
                        *servers = json!({});
                    }
                    servers.as_object_mut().unwrap().insert(
                        "ripwire-broker".into(),
                        json!({"command": binary, "args": ["--workspace", workspace]}),
                    );
                    v
                })?);
            if args.hooks {
                plan.changes
                    .push(change(workspace.join(".claude/settings.json"), |v| {
                        merge_hooks(v, Host::ClaudeCode, binary, Some(&workspace))
                    })?);
            }
        }
        Host::Codex => {
            let home = args
                .codex_home
                .clone()
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))
                .ok_or("no HOME: pass --codex-home")?;
            let mut toml = format!(
                "# Add to {}:\n[mcp_servers.ripwire-broker]\ncommand = {:?}\nargs = [\"--workspace\", {:?}]\n",
                home.join("config.toml").display(),
                binary.display().to_string(),
                workspace.display().to_string()
            );
            if args.hooks {
                toml.push_str("\n[features]\nhooks = true\n");
                // Global hooks: no --workspace, so each session uses its own cwd.
                plan.changes.push(change(home.join("hooks.json"), |v| {
                    merge_hooks(v, Host::Codex, binary, None)
                })?);
            }
            plan.notes.push(toml);
        }
    }
    Ok(plan)
}

/// Writes the changed files, each changed existing file backed up as `<name>.bak` once.
pub fn apply(plan: &Plan) -> std::io::Result<Vec<String>> {
    let mut log = vec![];
    for c in &plan.changes {
        if c.before.as_deref() == Some(c.after.as_str()) {
            log.push(format!("unchanged {}", c.path.display()));
            continue;
        }
        if let Some(parent) = c.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut line = format!("wrote {}", c.path.display());
        if let Some(before) = &c.before {
            let bak = PathBuf::from(format!("{}.bak", c.path.display()));
            if !bak.exists() {
                fs::write(&bak, before)?;
                line.push_str(&format!(" (backup {})", bak.display()));
            }
        }
        fs::write(&c.path, &c.after)?;
        log.push(line);
    }
    Ok(log)
}
