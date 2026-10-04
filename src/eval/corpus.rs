//! The corpus: tasks, each with a repository at a base commit, a prompt and the reference
//! patch it is scored against.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Corpus {
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    /// A local git repository; relative to the corpus file.
    pub repo: PathBuf,
    /// The commit the agent starts from.
    pub base: String,
    /// The reference commit, when the task comes from history. `validate` checks the task
    /// against it, and commands may name it as `{fix}` (e.g. to bring in its tests).
    #[serde(default)]
    pub fix: Option<String>,
    pub prompt: String,
    /// The task's words differ from the code's: the cases §23.15's recall bar is about.
    #[serde(default)]
    pub vocabulary_diverges: bool,
    pub reference: Reference,
    /// A shell command run in the agent's copy after it finishes; exit 0 is a correct task.
    #[serde(default)]
    pub check: Option<String>,
    /// A shell command run in the copy before the agent starts: dependencies, build caches, a
    /// database. Its failure makes the run invalid, and what it creates is not the agent's edit.
    #[serde(default)]
    pub setup: Option<String>,
    /// A shell command run after the check, whatever happened; its failure is ignored.
    #[serde(default)]
    pub teardown: Option<String>,
    /// Environment for setup, agent, check and teardown. `{run}` becomes the run's id, safe for
    /// a database name, so that runs never share state. The commands also take `{run}`, plus
    /// `{repo}` (the source repository) and `{fix}`.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Tasks with the same sequence are sessions of one history (PRD jev-mem §14): they run in
    /// corpus order, in one place and with one memory store per arm and repeat.
    #[serde(default)]
    pub sequence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    /// Files the reference patch modifies, relative to the repository root.
    pub files: Vec<String>,
    /// Tests that exercise the change.
    #[serde(default)]
    pub tests: Vec<String>,
}

impl Task {
    /// The repository's short name: what the report groups by, never a full path.
    pub fn repo_name(&self) -> String {
        self.repo
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.repo.display().to_string())
    }
}

fn git_ok(repo: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .is_ok_and(|o| o.status.success())
}

impl Corpus {
    /// Reads a corpus file and resolves relative repositories against its directory, as
    /// absolute paths: every git command runs from inside a copy, never from here.
    pub fn load(path: &Path) -> Result<Corpus, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut corpus: Corpus =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let dir = std::path::absolute(parent).map_err(|e| format!("{}: {e}", path.display()))?;
        for t in &mut corpus.tasks {
            if t.repo.is_relative() {
                t.repo = dir.join(&t.repo);
            }
        }
        Ok(corpus)
    }

    /// Every problem, one line each, prefixed with the task id; checked before any run.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = vec![];
        let mut ids = BTreeSet::new();
        // A sequence's sessions share one place, so one repository: another one's memories
        // would carry over.
        let mut sequences: BTreeMap<&str, &Path> = BTreeMap::new();
        for t in &self.tasks {
            match t.sequence.as_deref() {
                Some("") => errors.push(format!("{}: empty sequence name", t.id)),
                Some(s) => {
                    let repo = *sequences.entry(s).or_insert(&t.repo);
                    if repo != t.repo {
                        errors.push(format!(
                            "{}: sequence {s} runs in {}, not in this task's repository",
                            t.id,
                            repo.display()
                        ));
                    }
                }
                None => {}
            }
            if !ids.insert(&t.id) {
                errors.push(format!("{}: duplicate id", t.id));
            }
            if t.reference.files.is_empty() {
                errors.push(format!("{}: the reference names no files", t.id));
            }
            if !git_ok(&t.repo, &["rev-parse", "--git-dir"]) {
                errors.push(format!("{}: not a git repository", t.id));
            } else {
                if !git_ok(
                    &t.repo,
                    &["cat-file", "-e", &format!("{}^{{commit}}", t.base)],
                ) {
                    errors.push(format!("{}: base {} is not a commit", t.id, t.base));
                }
                // A mistyped fix would only show at run time, as a wrong answer in every arm.
                if let Some(fix) = &t.fix
                    && !git_ok(&t.repo, &["cat-file", "-e", &format!("{fix}^{{commit}}")])
                {
                    errors.push(format!("{}: fix {fix} is not a commit", t.id));
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Distinct repositories, by name.
    pub fn repos(&self) -> usize {
        self.tasks
            .iter()
            .map(Task::repo_name)
            .collect::<BTreeSet<_>>()
            .len()
    }
}
