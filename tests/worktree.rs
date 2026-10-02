//! The working-tree fingerprint (D-129): which files a shell command changed, from `git status`.
use ripwire_broker::worktree::{
    Fingerprint, MAX_FINGERPRINT_ENTRIES, Unusable, changed, fingerprint, fingerprint_within,
};
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "user.email=t@t", "-c", "user.name=t"])
        .args(args)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

/// A repository with one committed file, `a.txt`, and `ignored/` in `.gitignore`.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("a.txt"), "a\n").unwrap();
    std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
    git(root, &["init", "-q"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "init"]);
    dir
}

fn root_of(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().canonicalize().unwrap()
}

fn abs(dir: &tempfile::TempDir, rel: &str) -> String {
    root_of(dir).join(rel).to_string_lossy().into_owned()
}

/// mtime has a coarse clock on some filesystems: make a second write distinguishable.
fn rewrite(path: &Path, text: &str) {
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(path, text).unwrap();
}

#[test]
fn a_clean_tree_has_an_empty_fingerprint() {
    let dir = repo();
    assert_eq!(fingerprint(dir.path()), Ok(Fingerprint::default()));
}

#[test]
fn new_modified_and_deleted_files_are_changes() {
    let dir = repo();
    std::fs::write(dir.path().join("b.txt"), "b\n").unwrap();
    let before = fingerprint(dir.path()).unwrap();
    std::fs::write(dir.path().join("new.txt"), "n\n").unwrap();
    rewrite(&dir.path().join("b.txt"), "bb\n");
    std::fs::remove_file(dir.path().join("a.txt")).unwrap();
    let after = fingerprint(dir.path()).unwrap();
    let mut got = changed(&before, &after);
    got.sort();
    let mut want = vec![abs(&dir, "a.txt"), abs(&dir, "b.txt"), abs(&dir, "new.txt")];
    want.sort();
    assert_eq!(got, want);
    let deleted = after
        .entries
        .iter()
        .find(|e| e.path == abs(&dir, "a.txt"))
        .unwrap();
    assert_eq!(deleted.stamp, None, "a deleted file has no stamp");
}

#[test]
fn an_untouched_dirty_file_is_not_a_change() {
    let dir = repo();
    std::fs::write(dir.path().join("b.txt"), "b\n").unwrap();
    let before = fingerprint(dir.path()).unwrap();
    let after = fingerprint(dir.path()).unwrap();
    assert!(changed(&before, &after).is_empty());
}

#[test]
fn a_file_that_went_back_to_clean_is_a_change() {
    let dir = repo();
    rewrite(&dir.path().join("a.txt"), "changed\n");
    let before = fingerprint(dir.path()).unwrap();
    git(dir.path(), &["checkout", "-q", "--", "a.txt"]);
    let after = fingerprint(dir.path()).unwrap();
    assert_eq!(changed(&before, &after), vec![abs(&dir, "a.txt")]);
}

#[test]
fn ignored_files_are_not_seen() {
    let dir = repo();
    let before = fingerprint(dir.path()).unwrap();
    std::fs::create_dir(dir.path().join("ignored")).unwrap();
    std::fs::write(dir.path().join("ignored/x.o"), "x").unwrap();
    assert!(changed(&before, &fingerprint(dir.path()).unwrap()).is_empty());
}

#[test]
fn a_directory_outside_git_has_no_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(fingerprint(dir.path()), Err(Unusable::NotGit));
    assert!(!Unusable::NotGit.switches_off(), "failing fast is cheap");
}

#[test]
fn too_many_dirty_files_are_too_dirty() {
    let dir = repo();
    let many = dir.path().join("many");
    std::fs::create_dir(&many).unwrap();
    for i in 0..=MAX_FINGERPRINT_ENTRIES {
        std::fs::write(many.join(format!("{i}.txt")), "x").unwrap();
    }
    assert_eq!(fingerprint(dir.path()), Err(Unusable::TooDirty));
    assert!(Unusable::TooDirty.switches_off());
}

#[test]
fn a_spent_budget_is_too_slow() {
    let dir = repo();
    assert_eq!(
        fingerprint_within(dir.path(), std::time::Duration::ZERO),
        Err(Unusable::TooSlow)
    );
    assert!(Unusable::TooSlow.switches_off());
}

// Review focus 1
#[test]
fn a_subdirectory_workspace_gets_paths_from_the_repository_root() {
    let dir = repo();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/in.txt"), "i\n").unwrap();
    std::fs::write(dir.path().join("out.txt"), "o\n").unwrap();
    let print = fingerprint(&dir.path().join("sub")).unwrap();
    let mut paths: Vec<&str> = print.entries.iter().map(|e| e.path.as_str()).collect();
    paths.sort();
    let (inside, outside) = (abs(&dir, "sub/in.txt"), abs(&dir, "out.txt"));
    let mut want = vec![inside.as_str(), outside.as_str()];
    want.sort();
    assert_eq!(
        paths, want,
        "absolute, from the repository root, both sides"
    );
}

// Review focus 2
#[test]
fn odd_file_names_come_through_whole() {
    let dir = repo();
    for name in ["a b.txt", "ção.txt", "q\"uote.txt"] {
        std::fs::write(dir.path().join(name), "x").unwrap();
    }
    let print = fingerprint(dir.path()).unwrap();
    for name in ["a b.txt", "ção.txt", "q\"uote.txt"] {
        assert!(
            print.entries.iter().any(|e| e.path == abs(&dir, name)),
            "{name}: {print:?}"
        );
    }
}

// Review focus 3
#[test]
fn a_rename_reports_the_new_name_and_skips_the_old_field() {
    let dir = repo();
    git(dir.path(), &["mv", "a.txt", "renamed.txt"]);
    let print = fingerprint(dir.path()).unwrap();
    let paths: Vec<&str> = print.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, vec![abs(&dir, "renamed.txt").as_str()], "{print:?}");
}

// Review focus 4
#[test]
fn a_held_index_lock_does_not_stop_the_fingerprint() {
    let dir = repo();
    std::fs::write(dir.path().join("b.txt"), "b\n").unwrap();
    let lock = dir.path().join(".git/index.lock");
    std::fs::write(&lock, "").unwrap();
    let index = std::fs::read(dir.path().join(".git/index")).unwrap();
    assert!(fingerprint(dir.path()).is_ok());
    assert_eq!(
        std::fs::read(dir.path().join(".git/index")).unwrap(),
        index,
        "index untouched"
    );
    assert!(lock.exists(), "the user's lock is left alone");
}
