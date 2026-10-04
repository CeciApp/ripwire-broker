//! `doctor` (D-033): is this machine ready to run the broker for this workspace?

use crate::broker::{REQUIRED_VERBS, TaskRequest, check_version};
use crate::cli::DoctorArgs;
use crate::online::SemanticStage;
use crate::online::classifier::Classifier;
use crate::online::request::{StateItem, build};
use crate::state::StateStore;
use crate::upstream::ripwire_version;
use serde::Serialize;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Ok,
    /// Works, with a known consequence.
    Warn,
    Fail,
    /// Not run because an earlier check failed.
    Skip,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub status: Outcome,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct Report {
    /// No check failed; warnings allowed.
    pub ok: bool,
    pub checks: Vec<Check>,
}

impl Report {
    fn add(&mut self, name: &'static str, status: Outcome, detail: impl Into<String>) {
        self.checks.push(Check {
            name,
            status,
            detail: detail.into(),
        });
    }

    pub fn text(&self) -> String {
        self.checks
            .iter()
            .map(|c| {
                let tag = match c.status {
                    Outcome::Ok => "ok  ",
                    Outcome::Warn => "warn",
                    Outcome::Fail => "FAIL",
                    Outcome::Skip => "skip",
                };
                format!("{tag} {:<16} {}\n", c.name, c.detail)
            })
            .collect()
    }
}

pub async fn run(args: &DoctorArgs) -> Report {
    let mut r = Report {
        ok: true,
        checks: vec![],
    };
    let version = ripwire_version(&args.upstream.ripwire);
    let mut upstream_ok = true;
    if version == "unavailable" {
        upstream_ok = false;
        r.add(
            "ripwire_binary",
            Outcome::Fail,
            format!("cannot run {} --version", args.upstream.ripwire.display()),
        );
        r.add("ripwire_version", Outcome::Skip, "");
    } else {
        r.add(
            "ripwire_binary",
            Outcome::Ok,
            args.upstream.ripwire.display().to_string(),
        );
        match check_version(&version) {
            Ok(()) => r.add("ripwire_version", Outcome::Ok, format!("ripwire {version}")),
            Err(e) => {
                upstream_ok = false;
                r.add("ripwire_version", Outcome::Fail, e.message);
            }
        }
    }
    let workspace = args.workspace.canonicalize().ok().filter(|p| p.is_dir());
    match &workspace {
        Some(ws) => r.add("workspace", Outcome::Ok, ws.display().to_string()),
        None => r.add(
            "workspace",
            Outcome::Fail,
            format!("{} is not a readable directory", args.workspace.display()),
        ),
    }
    let broker = match (&workspace, upstream_ok) {
        (Some(ws), true) => match crate::local::launch(ws, &args.upstream, false, None, None).await
        {
            Ok(b) => {
                r.add(
                    "required_verbs",
                    Outcome::Ok,
                    format!("{} read-only verbs available", REQUIRED_VERBS.len()),
                );
                Some(b)
            }
            Err(e) => {
                r.add(
                    "required_verbs",
                    Outcome::Fail,
                    format!("{}: {}", e.error, e.message),
                );
                None
            }
        },
        _ => {
            r.add("required_verbs", Outcome::Skip, "");
            None
        }
    };
    match &workspace {
        Some(ws) => git_history(&mut r, ws),
        None => r.add("git_history", Outcome::Skip, ""),
    }
    state_dir(&mut r, args);
    memory(&mut r, args);
    summarizer(&mut r, args);
    match broker {
        Some(b) => {
            let started = Instant::now();
            let mut req = TaskRequest::new("overview of the repository structure");
            req.mode = crate::broker::Mode::Orient;
            match b.context_for_task(req).await {
                Ok(env) => r.add(
                    "smoke_call",
                    Outcome::Ok,
                    format!(
                        "context_for_task: {} items in {} ms",
                        env.items.len(),
                        started.elapsed().as_millis()
                    ),
                ),
                Err(e) => r.add(
                    "smoke_call",
                    Outcome::Fail,
                    format!("{}: {}", e.error, e.message),
                ),
            }
        }
        None => r.add("smoke_call", Outcome::Skip, ""),
    }
    if args.jev_probe {
        r.checks.push(probe_from_env(args).await);
    }
    r.ok = r.checks.iter().all(|c| c.status != Outcome::Fail);
    r
}

/// The probe's only content: invented and embedded in the binary. No workspace byte is ever
/// sent by `--jev-probe` (PRD §23.6, D-064).
pub const PROBE_PATH: &str = "probe/example.py";
pub const PROBE_SOURCE: &str = "def add(a, b):\n    return a + b\n";
pub const PROBE_QUERY: &str = "where are two numbers added?";

/// One synthetic `file_admission` question: does the provider answer, with this credential
/// and this model? It takes no workspace, so it cannot read one.
pub async fn jev_probe(classifier: &dyn Classifier) -> Check {
    let model = classifier.model().to_string();
    let item = StateItem {
        id: "p0".into(),
        path: PROBE_PATH.into(),
        text: PROBE_SOURCE.into(),
    };
    let req = build(
        &model,
        PROBE_QUERY,
        SemanticStage::FileAdmission,
        vec![item],
    );
    let started = Instant::now();
    let (status, detail) = match classifier.classify(&req).await {
        Ok(answers) => match answers.first().copied().flatten() {
            Some(p) => (
                Outcome::Ok,
                format!(
                    "{model} answered 1 synthetic question in {} ms (p={p:.2})",
                    started.elapsed().as_millis()
                ),
            ),
            None => (
                Outcome::Fail,
                format!("{model} answered without a valid probability"),
            ),
        },
        Err(e) => (Outcome::Fail, format!("{model}: {e}")),
    };
    Check {
        name: "jev_probe",
        status,
        detail,
    }
}

#[cfg(feature = "online")]
async fn probe_from_env(args: &DoctorArgs) -> Check {
    use crate::online::credential::Credential;
    use crate::online::jev::JevClient;
    let fail = |detail: String| Check {
        name: "jev_probe",
        status: Outcome::Fail,
        detail,
    };
    let key = match Credential::from_env() {
        Ok(k) => k,
        Err(e) => return fail(e.to_string()),
    };
    let model = args.jev_model.as_deref().unwrap_or("jev-1.13.0");
    match JevClient::new(key, model, std::time::Duration::from_secs(15)) {
        Ok(client) => jev_probe(&client).await,
        Err(e) => fail(e),
    }
}

#[cfg(not(feature = "online"))]
async fn probe_from_env(_: &DoctorArgs) -> Check {
    Check {
        name: "jev_probe",
        status: Outcome::Fail,
        detail: "this binary was built without the online feature; rebuild it with \
                 `cargo build --release --features online`"
            .into(),
    }
}

/// Without a commit, `situational_awareness` refuses and the finish gate says `unknown` (D-019).
fn git_history(r: &mut Report, ws: &std::path::Path) {
    let head = std::process::Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .env_remove(crate::online::KEY_VAR)
        .current_dir(ws)
        .output();
    match head {
        Ok(o) if o.status.success() => {
            r.add("git_history", Outcome::Ok, "git repository with commits")
        }
        Ok(_) => r.add(
            "git_history",
            Outcome::Warn,
            "no commit yet: context_after_edit fails and the finish gate answers unknown",
        ),
        Err(_) => r.add(
            "git_history",
            Outcome::Warn,
            "git not found: the finish gate answers unknown",
        ),
    }
}

fn state_dir(r: &mut Report, args: &DoctorArgs) {
    let Some(dir) = args.state_dir.clone().or_else(StateStore::default_dir) else {
        r.add(
            "state_dir",
            Outcome::Warn,
            "no HOME: hooks need --state-dir",
        );
        return;
    };
    let probe = StateStore::new(dir.clone());
    match probe.save("ripwire-broker-doctor", &Default::default()) {
        Ok(()) => {
            probe.remove("ripwire-broker-doctor");
            r.add("state_dir", Outcome::Ok, dir.display().to_string());
        }
        Err(e) => r.add(
            "state_dir",
            Outcome::Warn,
            format!(
                "{}: {e}; hooks would repeat context every time",
                dir.display()
            ),
        ),
    }
}

/// `name` as given if it has a slash, else the first match on `PATH`.
fn resolve(name: &str) -> Option<std::path::PathBuf> {
    if name.contains('/') {
        let p = std::path::PathBuf::from(name);
        return p.is_file().then_some(p);
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

/// The local model is optional: problems are warnings, and the model itself is never run
/// here (a cold start takes ~19 s, D-034).
fn summarizer(r: &mut Report, args: &DoctorArgs) {
    let Some(m) = &args.summarizer else {
        r.add("summarizer", Outcome::Ok, "not configured: notes are off");
        return;
    };
    let program = m.command.split_whitespace().next().unwrap_or("");
    if resolve(program).is_none() {
        r.add(
            "summarizer",
            Outcome::Warn,
            format!("{program} not found: notes would be unavailable"),
        );
        return;
    }
    let args: Vec<&str> = m.command.split_whitespace().collect();
    let is_ollama_run = std::path::Path::new(program)
        .file_name()
        .is_some_and(|n| n == "ollama")
        && args.get(1) == Some(&"run");
    if is_ollama_run && !args.contains(&"--nowordwrap") {
        r.add(
            "summarizer",
            Outcome::Warn,
            "ollama run word-wraps with terminal redraws even through a pipe, which garbles notes: \
             add --nowordwrap",
        );
        return;
    }
    let Some(version) = &m.version_cmd else {
        r.add(
            "summarizer",
            Outcome::Warn,
            format!(
                "{program} found; without --summarizer-version-cmd, new weights under the same \
                 model tag keep serving old cached notes"
            ),
        );
        return;
    };
    match crate::summarizer::CommandSummarizer::from_command_line(
        &m.command,
        m.timeout,
        Some(version),
    ) {
        Ok(_) => r.add(
            "summarizer",
            Outcome::Ok,
            format!("{program} found; model version tracked"),
        ),
        Err(e) => r.add(
            "summarizer",
            Outcome::Warn,
            format!("version command failed: {e}"),
        ),
    }
}

/// The workspace's memory store, read locally (PRD jev-mem §4). Only when there is one: a
/// workspace that never used `--memory` gets no extra line.
fn memory(r: &mut Report, args: &DoctorArgs) {
    use crate::memory::{identity, store::Store};
    let Some(dir) = args.state_dir.clone().or_else(StateStore::default_dir) else {
        return;
    };
    let Ok(id) = identity::workspace_id(&args.workspace) else {
        return;
    };
    let store = Store::new(&dir, &id);
    if std::fs::symlink_metadata(store.dir()).is_err() {
        return;
    }
    match store.load() {
        Err(why) => r.add(
            "memory",
            Outcome::Warn,
            format!(
                "store unavailable ({}): memory stays off for this workspace",
                why.as_str()
            ),
        ),
        Ok(state) => {
            let pending = store.pending().unwrap_or(0);
            let detail = format!(
                "{} memories, {pending} pending, generation {}",
                state.nodes.len(),
                state.generation
            );
            match store.is_revoked() {
                true => r.add(
                    "memory",
                    Outcome::Warn,
                    format!("{detail}; collection revoked: run `memory resume`"),
                ),
                false => r.add("memory", Outcome::Ok, detail),
            }
        }
    }
}
