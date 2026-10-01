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

/// A path a host config file can carry. Refused rather than mangled: `install` writes the
/// path a host will later execute.
fn utf8(path: &Path, what: &str) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{what} ({}) is not valid UTF-8", path.display()))
}

/// Hosts run hook commands through a shell: always single-quote, with `'` as `'\\''`.
fn quote(p: &Path) -> String {
    format!("'{}'", p.display().to_string().replace('\'', "'\\''"))
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

fn statusline_command(binary: &Path, workspace: &Path) -> String {
    format!(
        "{} statusline --workspace {} --color never",
        quote(binary),
        quote(workspace)
    )
}

/// Just enough of POSIX shell words to read back a command `install` wrote: single quotes (with
/// `'\''`), double quotes, backslashes and whitespace. Used only to recognize ownership.
fn shell_words(cmd: &str) -> Vec<String> {
    let (mut words, mut cur, mut any) = (vec![], String::new(), false);
    let mut it = cmd.chars();
    while let Some(c) = it.next() {
        match c {
            '\'' => {
                any = true;
                for c in it.by_ref() {
                    if c == '\'' {
                        break;
                    }
                    cur.push(c);
                }
            }
            '"' => {
                any = true;
                while let Some(c) = it.next() {
                    match c {
                        '"' => break,
                        '\\' => {
                            if let Some(n) = it.next() {
                                cur.push(n)
                            }
                        }
                        _ => cur.push(c),
                    }
                }
            }
            '\\' => {
                any = true;
                if let Some(n) = it.next() {
                    cur.push(n)
                }
            }
            c if c.is_whitespace() => {
                if any {
                    words.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                any = true;
                cur.push(c)
            }
        }
    }
    if any {
        words.push(cur);
    }
    words
}

enum Bar {
    Absent,
    Ours,
    Foreign,
}

/// Ours: the program is a `ripwire-broker` executable and its first argument is `statusline`.
fn bar(settings: &Value) -> Bar {
    // `null` is no bar at all, as far as the host is concerned.
    let Some(line) = settings.get("statusLine").filter(|l| !l.is_null()) else {
        return Bar::Absent;
    };
    let words = line
        .get("command")
        .and_then(Value::as_str)
        .map(shell_words)
        .unwrap_or_default();
    let ours = words
        .first()
        .and_then(|p| Path::new(p).file_name())
        .is_some_and(|n| n == "ripwire-broker")
        && words.get(1).is_some_and(|w| w == "statusline");
    if ours { Bar::Ours } else { Bar::Foreign }
}

fn merge_statusline(mut settings: Value, command: &str) -> Value {
    if !settings.is_object() {
        settings = json!({});
    }
    match bar(&settings) {
        Bar::Foreign => {}
        Bar::Absent => {
            settings["statusLine"] = json!({"type": "command", "command": command});
        }
        Bar::Ours => {
            settings["statusLine"]["type"] = "command".into();
            settings["statusLine"]["command"] = command.into();
        }
    }
    settings
}

fn user_settings() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude")))
        .map(|d| d.join("settings.json"))
}

/// Missing is fine (`None`); any other failure means the file's content is undetermined.
fn read_settings(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("unreadable ({e})")),
    }
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

/// The variable the server reads its credential from (PRD §23.6).
const KEY_VAR: &str = "RIPWIRE_BROKER_JEV_API_KEY";

/// Shown with every `--online` install (PRD §23.6: mandatory in the documentation of the flag).
const CONSENT: &str =
    "--online: O modo online envia previews e trechos elegíveis do workspace ao provider Jev.
Selecione somente uma raiz cujo conteúdo você tem autorização para enviar.
The key is never written here: export RIPWIRE_BROKER_JEV_API_KEY in the environment the host
starts from. The binary must be built with `--features online`; check it with
`ripwire-broker doctor --workspace DIR --jev-probe`.";

pub fn plan(args: &InstallArgs, binary: &Path) -> Result<Plan, String> {
    // Both host configs are text (JSON, TOML). A path they cannot carry is refused for what
    // it is, before the disk is touched, rather than panicking in `json!` or being written
    // back mangled by a lossy conversion.
    let binary_text = utf8(binary, "the broker binary")?;
    utf8(&args.workspace, "the workspace")?;
    let workspace = args
        .workspace
        .canonicalize()
        .map_err(|e| format!("workspace {}: {e}", args.workspace.display()))?;
    // Resolving can still surface bytes the argument did not have, through a symlink into a
    // directory whose name is not UTF-8.
    let workspace_text = utf8(&workspace, "the workspace")?;
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
                    let mut server =
                        json!({"command": binary_text, "args": ["--workspace", workspace_text]});
                    if args.online {
                        server["args"]
                            .as_array_mut()
                            .unwrap()
                            .push("--online".into());
                        // Expanded by Claude Code from its own environment: a reference, never
                        // the value.
                        server["env"] = json!({ KEY_VAR: format!("${{{KEY_VAR}}}") });
                    }
                    servers
                        .as_object_mut()
                        .unwrap()
                        .insert("ripwire-broker".into(), server);
                    v
                })?);
            let settings_path = workspace.join(".claude/settings.json");
            let command = statusline_command(binary, &workspace);
            let manual = format!(
                "statusLine for {}:\n{}",
                settings_path.display(),
                pretty(&json!({"statusLine": {"type": "command", "command": command}}))
            );
            let mut bar_wanted = args.statusline;
            let mut shadowing = false;
            if args.statusline {
                // D5: an inherited user bar would be shadowed by ours; an unreadable one leaves the
                // effective bar unknown. Either way, nothing is written and the snippet is shown.
                match user_settings() {
                    None => {
                        bar_wanted = false;
                        plan.notes.push(format!(
                            "the user settings file could not be located (no CLAUDE_CONFIG_DIR or HOME), so an inherited statusLine is unknown; add the bar by hand if it is free:\n{manual}"
                        ));
                    }
                    Some(p) => match read_settings(&p).and_then(|t| parse(&t)) {
                        Err(e) => {
                            bar_wanted = false;
                            plan.notes.push(format!(
                                "{}: {e}; add the bar by hand if it is free:\n{manual}",
                                p.display()
                            ));
                        }
                        Ok(v) if matches!(bar(&v), Bar::Foreign) => {
                            bar_wanted = false;
                            plan.notes.push(format!(
                                "{} has its own statusLine; keeping it. To use the broker's:\n{manual}",
                                p.display()
                            ));
                            // A bar of ours left in the project from before would shadow it.
                            shadowing = read(&settings_path)
                                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                                .is_some_and(|v| matches!(bar(&v), Bar::Ours));
                            if shadowing {
                                plan.notes.push(format!(
                                    "removing the broker's statusLine from {} so it does not shadow the one in {}",
                                    settings_path.display(),
                                    p.display()
                                ));
                            }
                        }
                        Ok(_) => {}
                    },
                }
                // Ours is written either way (the local file wins), but the user hears about it.
                let local = workspace.join(".claude/settings.local.json");
                match read_settings(&local).and_then(|t| parse(&t)) {
                    Err(e) => plan.notes.push(format!(
                        "{} could not be read ({e}); a statusLine there would win over the project's.",
                        local.display()
                    )),
                    Ok(v) if matches!(bar(&v), Bar::Foreign) => plan.notes.push(format!(
                        "{} has its own statusLine, and it wins over the project's.",
                        local.display()
                    )),
                    Ok(_) => {}
                }
                if !args.hooks {
                    plan.notes.push("statusLine without --hooks: the bar shows `hooks sem dados` until hooks are installed.".into());
                }
            }
            if args.hooks || bar_wanted || shadowing {
                let mut foreign = false;
                plan.changes.push(change(settings_path.clone(), |mut v| {
                    if args.hooks {
                        v = merge_hooks(v, Host::ClaudeCode, binary, Some(&workspace));
                    }
                    if shadowing && let Some(o) = v.as_object_mut() {
                        o.remove("statusLine");
                    }
                    if bar_wanted {
                        foreign = matches!(bar(&v), Bar::Foreign);
                        v = merge_statusline(v, &command);
                    }
                    v
                })?);
                if foreign {
                    plan.notes.push(format!("{} has a statusLine that is not the broker's; keeping it. To use the broker's:\n{manual}",
            settings_path.display()));
                }
            }
        }
        Host::Codex => {
            let home = args
                .codex_home
                .clone()
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))
                .ok_or("no HOME: pass --codex-home")?;
            let online = match args.online {
                // Forwarded by name from Codex's environment, never the value.
                true => format!(", \"--online\"]\nenv_vars = [\"{KEY_VAR}\"]"),
                false => "]".into(),
            };
            let mut toml = format!(
                "# Add to {}:\n[mcp_servers.ripwire-broker]\ncommand = {:?}\nargs = [\"--workspace\", {:?}{online}\n",
                home.join("config.toml").display(),
                binary_text,
                workspace_text
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
    if args.online {
        plan.notes.push(CONSENT.into());
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
