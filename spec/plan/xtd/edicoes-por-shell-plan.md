# Edições pelo shell no hook de edição — Plano de implementação

> **Para agentes:** sub-skill obrigatória: `superpowers:subagent-driven-development` (recomendada) ou
> `superpowers:executing-plans`, tarefa por tarefa. Os passos usam checkbox (`- [ ]`).

**Objetivo:** uma edição feita pela ferramenta Bash do Claude Code recebe o mesmo
`context_after_edit` que uma feita pelo Edit, logo depois do comando, e conta em `stats.events`;
um Bash que não mudou nada não sobe o ripwire.

**Arquitetura:** um módulo novo, `src/worktree.rs`, tira uma impressão digital barata da árvore do
git (`git status` + `mtime`/`size` de cada arquivo sujo) e compara duas impressões. `hook::run`
tira a impressão **antes** de `local::launch` em todo evento do Claude Code menos o `Stop`, guarda
em `SessionState.worktree`, e, num `PostToolUse` do Bash, só segue para o ripwire se algo mudou;
os arquivos mudados vão por `SessionState.shell_edits` (não salvo) para o mesmo caminho do Edit em
`respond`. O `install` acrescenta `Bash` ao matcher do Claude Code.

**Stack:** Rust 1.98.1 (edition 2024), `serde`/`serde_json`, `std::process` para o `git`. Nenhuma
dependência nova.

**Spec:** [`proposta-edicoes-por-shell.md`](proposta-edicoes-por-shell.md) (aprovada). Leia os dois;
"spec §N" abaixo é a seção N da proposta.

**Decisão:** o número da entrada de changelog é **D-129**, supondo o PR #38 (D-128) mesclado antes.
Se não estiver, use o próximo número livre e ajuste as referências desta lista.

## Restrições globais

- `MAX_FINGERPRINT_ENTRIES = 5000`; acima disso `fingerprint` devolve `None` (spec §5.4).
- `MAX_BASH_EDIT_FILES = 50`, aplicado **depois** do filtro `in_workspace` (spec §5.4).
- Timeout do `git`: **500 ms** no total das duas chamadas (spec §4.2, §5.1).
- `git status --porcelain=v1 -z --untracked-files=all`, com `GIT_OPTIONAL_LOCKS=0` (o `status` não
  pode escrever o índice nem disputar o lock do usuário).
- Só o host **Claude Code** tira impressão; o Codex não muda (spec §2, fora do escopo).
- O `Stop` não tira impressão (spec §4.4).
- Bash sem mudança: sem ripwire, sem `stats.events` (spec §4.5).
- Sem git: silêncio, sem mensagem de falha (spec §5.1).
- Custo: p95 ≤ **50 ms** acrescentados a um Bash só de leitura num repositório grande (spec §2.3).
- TDD com uma mutação por teste novo, como no D-127. Comentários de código em inglês, como o resto
  do crate; documentos em português.
- Antes de cada commit: `cargo fmt --all --check`, e no fim de cada tarefa
  `cargo clippy --all-targets --locked -- -D warnings`.

## Foco de revisão

Entradas que a spec implica mas não lista, mais prováveis de morder alguém; cada uma ganhou teste
na tarefa dona do código:

1. **Workspace num subdiretório do repositório git:** o `git status` lista caminhos relativos à
   raiz do repositório, não ao workspace. Esperado: caminhos absolutos certos, e o `in_workspace`
   descarta os de fora. Teste na Tarefa 1 (`a_subdirectory_workspace_gets_paths_from_the_repository_root`).
2. **Nomes com espaço, acento ou aspas:** `-z` evita o escape do git. Esperado: o caminho exato.
   Teste na Tarefa 1 (`odd_file_names_come_through_whole`).
3. **`git mv` (renomeação):** a entrada `R` traz um segundo campo com o nome antigo. Esperado: o
   nome novo aparece, e o segundo campo não vira uma entrada falsa. Teste na Tarefa 1
   (`a_rename_reports_the_new_name_and_skips_the_old_field`).
4. **O usuário rodando git ao mesmo tempo (`index.lock` presente):** esperado: a impressão sai
   assim mesmo e nada é escrito no `.git`. Teste na Tarefa 1
   (`a_held_index_lock_does_not_stop_the_fingerprint`).
5. **Bash que apaga um arquivo (`rm`):** esperado: o caminho apagado chega ao `context_after_edit`
   (o `in_workspace` resolve só o prefixo existente). Teste na Tarefa 2
   (`a_shell_deletion_still_reaches_the_edit_context`).

---

### Tarefa 1: `src/worktree.rs` — impressão digital e comparação

**Arquivos:**
- Criar: `src/worktree.rs`
- Modificar: `src/lib.rs` (registrar `pub mod worktree;` logo depois de `pub mod workspace;`, mantendo
  a ordem alfabética)
- Teste: `tests/worktree.rs`

**Interfaces:**
- Consome: nada.
- Produz:
  - `pub const MAX_FINGERPRINT_ENTRIES: usize = 5000;`
  - `pub struct Stamp { pub secs: u64, pub nanos: u32, pub size: u64 }` (`Copy`, `Eq`, serde)
  - `pub struct Entry { pub path: String, pub stamp: Option<Stamp> }` (`path` absoluto; `None` = apagado)
  - `pub struct Fingerprint { pub entries: Vec<Entry> }` (`Default`, `Eq`, serde; ordem do `git status`)
  - `pub fn fingerprint(root: &std::path::Path) -> Option<Fingerprint>`
  - `pub fn changed(before: &Fingerprint, after: &Fingerprint) -> Vec<String>`

- [ ] **Passo 1: escrever os testes que falham**

`tests/worktree.rs`:

```rust
//! The working-tree fingerprint (D-129): which files a shell command changed, from `git status`.
use ripwire_broker::worktree::{Fingerprint, MAX_FINGERPRINT_ENTRIES, changed, fingerprint};
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
    assert_eq!(fingerprint(dir.path()), Some(Fingerprint::default()));
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
    let deleted = after.entries.iter().find(|e| e.path == abs(&dir, "a.txt")).unwrap();
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
    assert_eq!(fingerprint(dir.path()), None);
}

#[test]
fn too_many_dirty_files_have_no_fingerprint() {
    let dir = repo();
    let many = dir.path().join("many");
    std::fs::create_dir(&many).unwrap();
    for i in 0..=MAX_FINGERPRINT_ENTRIES {
        std::fs::write(many.join(format!("{i}.txt")), "x").unwrap();
    }
    assert_eq!(fingerprint(dir.path()), None);
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
    assert_eq!(paths, want, "absolute, from the repository root, both sides");
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
    assert!(fingerprint(dir.path()).is_some());
    assert_eq!(std::fs::read(dir.path().join(".git/index")).unwrap(), index, "index untouched");
    assert!(lock.exists(), "the user's lock is left alone");
}
```

- [ ] **Passo 2: rodar e ver falhar**

Run: `cargo test --locked --test worktree`
Expected: FAIL de compilação, `unresolved import ripwire_broker::worktree`.

- [ ] **Passo 3: implementar o mínimo**

`src/worktree.rs`:

```rust
//! The dirty files of a git working tree and when each last changed (D-129): how a shell command
//! that edited files is told apart from one that only read, before ripwire starts.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Past this many dirty files the tree is "too dirty to follow" and has no fingerprint, so the
/// session state never grows with it.
pub const MAX_FINGERPRINT_ENTRIES: usize = 5000;

/// For both git calls together: a hook must never hold the host up for long.
const GIT_TIMEOUT: Duration = Duration::from_millis(500);

/// When a file last changed, as far as a cheap `stat` can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    pub secs: u64,
    pub nanos: u32,
    pub size: u64,
}

/// One dirty path, absolute; no stamp when it no longer exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    pub stamp: Option<Stamp>,
}

/// The dirty files of a tree, in `git status` order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub entries: Vec<Entry>,
}

/// The fingerprint of the repository holding `root`; `None` outside git, when git fails or is
/// slow, or past `MAX_FINGERPRINT_ENTRIES`.
pub fn fingerprint(root: &Path) -> Option<Fingerprint> {
    let deadline = Instant::now() + GIT_TIMEOUT;
    let top = git(root, &["rev-parse", "--show-toplevel"], deadline)?;
    let top = String::from_utf8(top).ok()?;
    let top = Path::new(top.trim_end_matches('\n'));
    let out = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        deadline,
    )?;
    let mut entries = Vec::new();
    let mut fields = out.split(|b| *b == 0).filter(|f| !f.is_empty());
    while let Some(field) = fields.next() {
        // "XY path": two status letters, a space, the path relative to the repository root.
        if field.len() < 4 {
            continue;
        }
        // A rename or copy is followed by one more field, the original path.
        if field[..2].iter().any(|c| matches!(c, b'R' | b'C')) {
            fields.next();
        }
        let Ok(rel) = std::str::from_utf8(&field[3..]) else {
            continue;
        };
        let path = top.join(rel);
        entries.push(Entry {
            stamp: stamp(&path),
            path: path.to_string_lossy().into_owned(),
        });
        if entries.len() > MAX_FINGERPRINT_ENTRIES {
            return None;
        }
    }
    Some(Fingerprint { entries })
}

/// The paths that are new, changed or deleted in `after`, then those dirty in `before` and clean
/// in `after` (a revert or a commit is an edit too). Once each.
pub fn changed(before: &Fingerprint, after: &Fingerprint) -> Vec<String> {
    let was: HashMap<&str, Option<Stamp>> = before
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e.stamp))
        .collect();
    let now: HashSet<&str> = after.entries.iter().map(|e| e.path.as_str()).collect();
    let mut out: Vec<String> = after
        .entries
        .iter()
        .filter(|e| was.get(e.path.as_str()) != Some(&e.stamp))
        .map(|e| e.path.clone())
        .collect();
    out.extend(
        before
            .entries
            .iter()
            .filter(|e| !now.contains(e.path.as_str()))
            .map(|e| e.path.clone()),
    );
    let mut seen = HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    let t = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(Stamp {
        secs: t.as_secs(),
        nanos: t.subsec_nanos(),
        size: meta.len(),
    })
}

/// Runs git read-only (`GIT_OPTIONAL_LOCKS=0`: `status` must not refresh the index or contend for
/// the user's lock) and returns stdout, or `None` on failure or past `deadline`.
fn git(root: &Path, args: &[&str], deadline: Instant) -> Option<Vec<u8>> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Read on a thread: a long listing would fill the pipe and stall the child past the deadline.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        stdout.read_to_end(&mut buf).map(|_| buf)
    });
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = reader.join().ok()?.ok()?;
                return status.success().then_some(out);
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(2)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}
```

Em `src/lib.rs`, depois de `pub mod workspace;`:

```rust
pub mod worktree;
```

- [ ] **Passo 4: rodar e ver passar**

Run: `cargo test --locked --test worktree`
Expected: PASS, 11 testes.

- [ ] **Passo 5: mutações (uma por teste; reverter cada uma e `touch src/worktree.rs` depois)**

| Mutação | Teste que deve falhar |
| --- | --- |
| tirar o `fields.next()` da renomeação | `a_rename_reports_the_new_name_and_skips_the_old_field` |
| `top.join(rel)` → `root.join(rel)` | `a_subdirectory_workspace_gets_paths_from_the_repository_root` |
| tirar o `out.extend(...)` dos que ficaram limpos | `a_file_that_went_back_to_clean_is_a_change` |
| `!=` → `==` no filtro de `changed` | `new_modified_and_deleted_files_are_changes`, `an_untouched_dirty_file_is_not_a_change` |
| `> MAX_FINGERPRINT_ENTRIES` → `> usize::MAX` | `too_many_dirty_files_have_no_fingerprint` |
| tirar `.env("GIT_OPTIONAL_LOCKS", "0")` | `a_held_index_lock_does_not_stop_the_fingerprint` (se não falhar no seu git, registre no D-129 que o teste não pega a mutação e por quê) |
| `--untracked-files=all` → `--untracked-files=no` | `new_modified_and_deleted_files_are_changes` |
| sem `-z` (e split por `\n`) | `odd_file_names_come_through_whole` |
| `success().then_some` → `Some(out)` sempre | `a_directory_outside_git_has_no_fingerprint` |
| `stamp` devolve sempre `None` | `new_modified_and_deleted_files_are_changes` (o `b.txt` mudado some) |

`ignored_files_are_not_seen`: mutação `--ignored` acrescentado ao `status`.

- [ ] **Passo 6: commit**

```bash
cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings
git add src/worktree.rs src/lib.rs tests/worktree.rs
git commit -m "worktree: fingerprint the dirty files of a git tree and compare two (D-129)"
```

---

### Tarefa 2: `respond` — o PostToolUse do Bash usa os arquivos que o `run` achou

**Arquivos:**
- Modificar: `src/hook.rs` — `SessionState` (dois campos novos, depois de `statusline`), uma função
  `is_shell`, e o ramo `Event::PostToolUse` de `respond` (hoje em `src/hook.rs:460-506`).
- Teste: `tests/hooks.rs` (testes novos perto de `an_edit_injects_what_it_may_have_affected`).

**Interfaces:**
- Consome: `ripwire_broker::worktree::Fingerprint` (Tarefa 1).
- Produz:
  - `SessionState.worktree: Option<crate::worktree::Fingerprint>` (salvo; `#[serde(default, skip_serializing_if = "Option::is_none")]`)
  - `SessionState.shell_edits: Vec<String>` (`#[serde(skip)]`, nunca salvo; caminhos absolutos)
  - `pub const MAX_BASH_EDIT_FILES: usize = 50;` em `src/hook.rs`
  - `pub fn is_shell(event: Event, input: &Value) -> bool` em `src/hook.rs`

- [ ] **Passo 1: escrever os testes que falham**

Em `tests/hooks.rs`, depois de `an_edit_injects_what_it_may_have_affected`:

```rust
/// A Claude Code `PostToolUse` of the Bash tool, as the host sends it.
fn shell_event(ws: &Path, command: &str) -> Value {
    json!({
        "session_id": "s",
        "cwd": ws,
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "tool_response": {"stdout": "", "stderr": "", "interrupted": false},
    })
}

#[tokio::test]
async fn a_shell_edit_gets_the_edit_context_for_the_files_run_found() {
    let (b, fake, ws) = hook_broker(edit_fake()).await;
    let root = ws.path().canonicalize().unwrap();
    std::fs::write(root.join("a.txt"), "hello.").unwrap();
    let mut state = SessionState {
        shell_edits: vec![root.join("a.txt").to_string_lossy().into_owned()],
        ..SessionState::default()
    };
    let input = shell_event(ws.path(), "echo hello. > a.txt");

    let out = post_tool_use(Host::ClaudeCode, &input, &b, &mut state)
        .await
        .expect("injected");

    assert_eq!(injected(&out)["tool"], "context_after_edit");
    assert_eq!(fake.calls()[0].1["files"], "a.txt");
    assert!(state.shell_edits.is_empty(), "consumed by this event");
    assert_eq!(state.stats.events, 1);
}

#[tokio::test]
async fn a_shell_command_with_no_files_asks_nothing() {
    let (b, fake, ws) = hook_broker(edit_fake()).await;
    let input = shell_event(ws.path(), "ls");

    let out = post_tool_use(Host::ClaudeCode, &input, &b, &mut SessionState::default()).await;

    assert!(out.is_none());
    assert!(fake.calls().is_empty(), "{:?}", fake.calls());
}

#[tokio::test]
async fn a_large_shell_change_sends_at_most_the_cap() {
    let (b, fake, ws) = hook_broker(edit_fake()).await;
    let root = ws.path().canonicalize().unwrap();
    let files: Vec<String> = (0..60)
        .map(|i| {
            let p = root.join(format!("f{i}.txt"));
            std::fs::write(&p, "x").unwrap();
            p.to_string_lossy().into_owned()
        })
        .collect();
    let mut state = SessionState {
        shell_edits: files,
        ..SessionState::default()
    };

    post_tool_use(Host::ClaudeCode, &shell_event(ws.path(), "gen"), &b, &mut state).await;

    let sent = fake.calls()[0].1["files"].as_str().unwrap().to_string();
    assert_eq!(sent.split(',').count(), hook::MAX_BASH_EDIT_FILES, "{sent}");
    assert!(sent.starts_with("f0.txt,f1.txt"), "in the order run found them: {sent}");
}

// Review focus 5
#[tokio::test]
async fn a_shell_deletion_still_reaches_the_edit_context() {
    let (b, fake, ws) = hook_broker(edit_fake()).await;
    let root = ws.path().canonicalize().unwrap();
    let mut state = SessionState {
        shell_edits: vec![root.join("gone.txt").to_string_lossy().into_owned()],
        ..SessionState::default()
    };

    post_tool_use(Host::ClaudeCode, &shell_event(ws.path(), "rm gone.txt"), &b, &mut state).await;

    assert_eq!(fake.calls()[0].1["files"], "gone.txt");
}

#[test]
fn shell_edits_are_never_saved_and_the_fingerprint_is() {
    let state = SessionState {
        shell_edits: vec!["/x".into()],
        worktree: Some(ripwire_broker::worktree::Fingerprint::default()),
        ..SessionState::default()
    };
    let text = serde_json::to_string(&state).unwrap();
    assert!(!text.contains("shell_edits"), "{text}");
    let back: SessionState = serde_json::from_str(&text).unwrap();
    assert!(back.shell_edits.is_empty());
    assert_eq!(back.worktree, Some(Default::default()));
    let old: SessionState = serde_json::from_str("{\"memory\":{},\"prompts_seen\":0,\"opted_out\":false}").unwrap();
    assert_eq!(old.worktree, None, "a state saved before D-129 loads");
}
```

Se `serde_json::from_str` do estado antigo falhar por causa de outro campo obrigatório de
`SessionMemory`, troque o JSON por `serde_json::to_string(&SessionState::default())` com a chave
`worktree` removida — o que importa é carregar sem `worktree`.

- [ ] **Passo 2: rodar e ver falhar**

Run: `cargo test --locked --test hooks shell`
Expected: FAIL de compilação, `struct SessionState has no field named shell_edits`.

- [ ] **Passo 3: implementar o mínimo**

Em `SessionState`, depois do campo `statusline`:

```rust
    /// The working tree as the last hook saw it, so a shell command can be told which files it
    /// changed (D-129); absent outside git and in states saved before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<crate::worktree::Fingerprint>,
    /// The files this event's shell command changed, found by `run` before ripwire starts.
    /// Belongs to one event: never saved.
    #[serde(skip)]
    pub shell_edits: Vec<String>,
```

Perto de `edited_files`:

```rust
/// A shell command changing many files (a formatter, a generator) is asked about the first ones;
/// the `Stop` gate sees them all (D-129).
pub const MAX_BASH_EDIT_FILES: usize = 50;

/// Whether the event is Claude Code's Bash tool finishing.
pub fn is_shell(event: Event, input: &Value) -> bool {
    event == Event::PostToolUse && input.get("tool_name").and_then(Value::as_str) == Some("Bash")
}
```

No ramo `Event::PostToolUse` de `respond`, troque o começo (de `let cwd` até o `if files.is_empty()`):

```rust
        Event::PostToolUse => {
            let cwd = input.get("cwd").and_then(Value::as_str).unwrap_or("");
            let shell = is_shell(event, input);
            let named = if shell {
                std::mem::take(&mut state.shell_edits)
            } else {
                edited_files(input)
            };
            let mut files: Vec<String> = named
                .iter()
                .map(|f| std::path::Path::new(cwd).join(f))
                .filter_map(|p| broker.in_workspace(&p.to_string_lossy()))
                .collect();
            if shell {
                files.truncate(MAX_BASH_EDIT_FILES);
            }
            if files.is_empty() {
                return Ok(None);
            }
```

e apague a linha seguinte `let mut files = files;` (o `files` já é `mut`). O resto do ramo
(coalescência, `context_after_edit`, `has_news`, `record`) fica igual.

- [ ] **Passo 4: rodar e ver passar**

Run: `cargo test --locked --test hooks`
Expected: PASS, os testes antigos e os 5 novos.

- [ ] **Passo 5: mutações**

| Mutação | Teste que deve falhar |
| --- | --- |
| `named` sempre de `edited_files` | `a_shell_edit_gets_the_edit_context_for_the_files_run_found` |
| `clone()` em vez de `mem::take` | o mesmo (`shell_edits` consumido) |
| sem o `truncate` | `a_large_shell_change_sends_at_most_the_cap` |
| `#[serde(skip)]` → `#[serde(default)]` | `shell_edits_are_never_saved_and_the_fingerprint_is` |

- [ ] **Passo 6: commit**

```bash
cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings
git add src/hook.rs tests/hooks.rs
git commit -m "hook: a Bash PostToolUse asks about the files its command changed (D-129)"
```

---

### Tarefa 3: `run` — impressão digital antes do ripwire e o atalho do Bash sem mudança

**Arquivos:**
- Modificar: `src/hook.rs`, função `run` (hoje em `src/hook.rs:535-636`): um bloco novo logo depois
  da definição do closure `finish` e **antes** de `let default = Policy::default();`.
- Teste: `tests/cli.rs` (testes novos no fim do arquivo).

**Interfaces:**
- Consome: `worktree::{fingerprint, changed}` (Tarefa 1); `SessionState.worktree`,
  `SessionState.shell_edits`, `hook::is_shell` (Tarefa 2).
- Produz: o comportamento; nenhuma interface nova.

**Desvio consciente da spec §4.4:** a spec diz que, nos eventos que não são Bash, a impressão é
tirada "depois de decidir a resposta". Aqui ela é tirada antes de subir o ripwire, em todos. Dá no
mesmo, porque um hook nunca edita arquivos, e assim um só bloco cobre todos os eventos. Registre
isto no D-129.

- [ ] **Passo 1: escrever os testes que falham**

No fim de `tests/cli.rs`. Todos usam `--ripwire` apontando para um binário que **não existe**:
se o hook tentar subir o ripwire, a saída traz `no context`; silêncio prova que não tentou.

```rust
mod shell_edits {
    use super::run;
    use ripwire_broker::state::StateStore;
    use serde_json::json;
    use std::path::Path;

    const NO_RIPWIRE: &str = "/nonexistent/ripwire";

    fn git_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("a.txt"), "a\n").unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec!["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "i"],
        ] {
            assert!(std::process::Command::new("git").arg("-C").arg(root).args(&args).status().unwrap().success());
        }
        dir
    }

    fn hook(event: &str, ws: &Path, state: &Path, input: serde_json::Value) -> String {
        let (code, out, err) = run(
            &[
                "hook",
                "claude-code",
                event,
                "--workspace",
                ws.to_str().unwrap(),
                "--state-dir",
                state.to_str().unwrap(),
                "--ripwire",
                NO_RIPWIRE,
            ],
            &input.to_string(),
        );
        assert_eq!(code, 0, "{err}");
        out
    }

    fn prompt(ws: &Path, state: &Path, text: &str) -> String {
        hook("user-prompt-submit", ws, state, json!({"session_id": "s", "cwd": ws, "hook_event_name": "UserPromptSubmit", "prompt": text}))
    }

    fn shell(ws: &Path, state: &Path, command: &str) -> String {
        hook("post-tool-use", ws, state, json!({"session_id": "s", "cwd": ws, "hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": command}, "tool_response": {}}))
    }

    fn edit(ws: &Path, state: &Path, file: &str) -> String {
        hook("post-tool-use", ws, state, json!({"session_id": "s", "cwd": ws, "hook_event_name": "PostToolUse", "tool_name": "Edit", "tool_input": {"file_path": ws.join(file)}, "tool_response": {}}))
    }

    fn saved(state: &Path) -> ripwire_broker::hook::SessionState {
        StateStore::new(state.to_path_buf()).load("s")
    }

    #[test]
    fn a_read_only_command_never_starts_ripwire() {
        let (ws, state) = (git_repo(), tempfile::tempdir().unwrap());
        prompt(ws.path(), state.path(), "task");
        let events = saved(state.path()).stats.events;

        let out = shell(ws.path(), state.path(), "cat a.txt");

        assert!(out.is_empty(), "silent, no ripwire launch: {out}");
        assert_eq!(saved(state.path()).stats.events, events, "not counted");
    }

    #[test]
    fn a_command_that_changed_a_file_goes_on_to_ripwire() {
        let (ws, state) = (git_repo(), tempfile::tempdir().unwrap());
        prompt(ws.path(), state.path(), "task");
        std::fs::write(ws.path().join("a.txt"), "changed\n").unwrap();

        let out = shell(ws.path(), state.path(), "echo changed > a.txt");

        assert!(out.contains("no context"), "it tried to launch ripwire: {out}");
    }

    #[test]
    fn an_edit_moves_the_baseline_so_the_next_command_is_not_blamed() {
        let (ws, state) = (git_repo(), tempfile::tempdir().unwrap());
        prompt(ws.path(), state.path(), "task");
        std::fs::write(ws.path().join("a.txt"), "by edit\n").unwrap();
        edit(ws.path(), state.path(), "a.txt");

        let out = shell(ws.path(), state.path(), "ls");

        assert!(out.is_empty(), "{out}");
    }

    #[test]
    fn without_a_baseline_nothing_is_blamed() {
        let (ws, state) = (git_repo(), tempfile::tempdir().unwrap());
        std::fs::write(ws.path().join("a.txt"), "changed\n").unwrap();

        let out = shell(ws.path(), state.path(), "echo changed > a.txt");

        assert!(out.is_empty(), "{out}");
        assert!(saved(state.path()).worktree.is_some(), "the baseline is taken now");
    }

    #[test]
    fn opted_out_commands_stay_silent_and_still_move_the_baseline() {
        let (ws, state) = (git_repo(), tempfile::tempdir().unwrap());
        prompt(ws.path(), state.path(), "#ripwire-off");
        std::fs::write(ws.path().join("a.txt"), "paused\n").unwrap();

        let out = shell(ws.path(), state.path(), "echo paused > a.txt");

        assert!(out.is_empty(), "{out}");
        let print = saved(state.path()).worktree.unwrap();
        assert!(print.entries.iter().any(|e| e.path.ends_with("a.txt")), "{print:?}");
    }

    #[test]
    fn outside_git_a_command_is_silent() {
        let (ws, state) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        prompt(ws.path(), state.path(), "task");
        std::fs::write(ws.path().join("a.txt"), "x").unwrap();

        let out = shell(ws.path(), state.path(), "echo x > a.txt");

        assert!(out.is_empty(), "{out}");
        assert_eq!(saved(state.path()).worktree, None);
    }

    #[test]
    fn stop_does_not_take_a_fingerprint() {
        let (ws, state) = (git_repo(), tempfile::tempdir().unwrap());
        hook("stop", ws.path(), state.path(), json!({"session_id": "s", "cwd": ws.path(), "hook_event_name": "Stop", "stop_hook_active": false}));
        assert_eq!(saved(state.path()).worktree, None);
    }
}
```

Se `run` não for visível dentro do `mod` (ele é uma `fn` livre do arquivo), o `use super::run;`
resolve; se `StateStore::new` tiver outra assinatura, ajuste para a de `src/state.rs`.

- [ ] **Passo 2: rodar e ver falhar**

Run: `cargo test --locked --test cli shell_edits`
Expected: FAIL. `a_read_only_command_never_starts_ripwire` traz `no context` na saída (hoje todo
hook sobe o ripwire), e os testes de `worktree` acham `None`.

- [ ] **Passo 3: implementar o mínimo**

Em `run`, logo depois do closure `finish` e antes de `let default = Policy::default();`:

```rust
    // A shell command is an edit only if the working tree says so, and asking costs a `git
    // status`, not a ripwire (D-129). Every Claude Code event but `Stop` moves the baseline, so a
    // command is never blamed for an edit made before it.
    if args.host == Host::ClaudeCode && args.event != Event::Stop {
        let before = state.worktree.take();
        state.worktree = root.as_deref().and_then(crate::worktree::fingerprint);
        if is_shell(args.event, &input) {
            let changed = match (&before, &state.worktree) {
                (Some(b), Some(a)) => crate::worktree::changed(b, a),
                _ => vec![],
            };
            if changed.is_empty() || state.opted_out {
                if state.worktree != before {
                    finish(&state);
                }
                return None;
            }
            state.shell_edits = changed;
        }
    }
```

`finish` é um closure que pega `state` por referência; se o borrow checker reclamar porque `state`
é mutado depois da definição do closure, mova este bloco para **antes** de `let finish = …`
(depois do `bind`) e, no ramo de saída, salve com o mesmo código do closure:

```rust
            if changed.is_empty() || state.opted_out {
                if state.worktree != before
                    && store.save(&session_id, &state).is_ok()
                    && publishes
                    && let (Some(r), Some(snap)) =
                        (&root, crate::statusline_state::project(&state, now()))
                {
                    let _ = crate::statusline_state::publish(store.dir(), &session_id, r, &snap);
                }
                return None;
            }
```

- [ ] **Passo 4: rodar e ver passar**

Run: `cargo test --locked --test cli shell_edits` e depois `cargo test --all-targets --locked`
Expected: PASS em tudo.

- [ ] **Passo 5: mutações**

| Mutação | Teste que deve falhar |
| --- | --- |
| tirar o `return None` (seguir para o ripwire sempre) | `a_read_only_command_never_starts_ripwire` |
| `changed.is_empty() \|\| state.opted_out` → só `changed.is_empty()` | `opted_out_commands_stay_silent_and_still_move_the_baseline` |
| refrescar a base só no Bash (`if is_shell` em volta do `fingerprint`) | `an_edit_moves_the_baseline_so_the_next_command_is_not_blamed` |
| `(None, Some(a))` tratado como "tudo mudou" | `without_a_baseline_nothing_is_blamed` |
| `args.event != Event::Stop` → `true` | `stop_does_not_take_a_fingerprint` |
| `state.shell_edits = changed;` removido | `a_command_that_changed_a_file_goes_on_to_ripwire` passa (ripwire falha antes); a mutação é pega por `a_shell_edit_gets_the_edit_context_for_the_files_run_found` só no nível do `handle` — registre no D-129 que o fio `run` → `respond` é coberto pela Tarefa 5 (fixture real, ripwire real) |

- [ ] **Passo 6: commit**

```bash
cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings
git add src/hook.rs tests/cli.rs
git commit -m "hook: fingerprint the tree before ripwire; a Bash that changed nothing stays silent (D-129)"
```

---

### Tarefa 4: `install` — `Bash` no matcher do Claude Code

**Arquivos:**
- Modificar: `src/install.rs:25`
- Teste: `tests/cli.rs:868` (asserção existente) e um teste novo de reinstalação.

**Interfaces:**
- Consome: nada.
- Produz: o matcher `Edit|Write|MultiEdit|NotebookEdit|Bash`.

- [ ] **Passo 1: escrever os testes que falham**

Em `tests/cli.rs`, troque a asserção da linha ~868:

```rust
    assert_eq!(
        s["hooks"]["PostToolUse"][0]["matcher"],
        "Edit|Write|MultiEdit|NotebookEdit|Bash"
    );
```

E um teste novo, perto dos testes de `install` (copie o preâmbulo — diretório temporário, workspace,
`--write` — do teste que contém a asserção acima; os nomes `read_json`, `commands` e `run` já existem
no arquivo):

```rust
#[test]
fn reinstalling_over_an_old_matcher_adds_bash() {
    let ws = tempfile::tempdir().unwrap();
    let settings = ws.path().join(".claude/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let old = serde_json::json!({"hooks": {"PostToolUse": [{"matcher": "Edit|Write|MultiEdit|NotebookEdit",
        "hooks": [{"type": "command", "command": "'/old/ripwire-broker' hook claude-code post-tool-use --workspace 'x'"}]}]}});
    std::fs::write(&settings, old.to_string()).unwrap();

    let (code, _, err) = run(
        &["install", "claude-code", "--workspace", ws.path().to_str().unwrap(), "--hooks", "--write"],
        "",
    );

    assert_eq!(code, 0, "{err}");
    let s = read_json(&settings);
    let groups = s["hooks"]["PostToolUse"].as_array().unwrap();
    assert_eq!(groups.len(), 1, "ours replaced, not duplicated: {s}");
    assert_eq!(groups[0]["matcher"], "Edit|Write|MultiEdit|NotebookEdit|Bash");
}
```

- [ ] **Passo 2: rodar e ver falhar**

Run: `cargo test --locked --test cli install`
Expected: FAIL nas duas asserções do matcher.

- [ ] **Passo 3: implementar**

`src/install.rs`:

```rust
        Host::ClaudeCode => "Edit|Write|MultiEdit|NotebookEdit|Bash",
```

- [ ] **Passo 4: rodar e ver passar**

Run: `cargo test --all-targets --locked`
Expected: PASS. Se outro teste fixar o matcher antigo, `grep -rn "NotebookEdit\"" tests/` e
atualize-o.

- [ ] **Passo 5: commit**

```bash
cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings
git add src/install.rs tests/cli.rs
git commit -m "install: Bash joins the Claude Code PostToolUse matcher (D-129)"
```

---

### Tarefa 5: fixture real do `PostToolUse` do Bash e o caminho inteiro com o ripwire real

**Precisa do mantenedor** para o Passo 1 (uma sessão real do Claude Code).

**Arquivos:**
- Criar: `tests/fixtures/hooks/claude_code_post_tool_use_bash.json`
- Teste: `tests/cli.rs` (um teste com o ripwire real, atrás de `require_ripwire!()`).

**Interfaces:**
- Consome: tudo das Tarefas 1–4.

- [ ] **Passo 1: capturar o payload (mantenedor)**

Na pasta de teste da validação manual (`statusline-manual/ws/`), acrescente a
`.claude/settings.local.json` um hook de captura e abra o Claude Code:

```json
{
  "hooks": {
    "PostToolUse": [
      {"matcher": "Bash", "hooks": [{"type": "command", "command": "cat > /tmp/ripwire-bash-hook.json"}]}
    ]
  }
}
```

Peça: "rode `echo '// x' >> src/lib.rs` com a ferramenta Bash". Depois remova o hook de captura.

- [ ] **Passo 2: gravar a fixture**

Troque o workspace por `__WORKSPACE__` e `transcript_path` por `__TRANSCRIPT__`, como nas outras
fixtures de hooks, sem dado pessoal, indentação de 1 espaço:

```bash
W=/Users/aquental/projects/ai/CECI/statusline-manual/ws
jq --indent 1 --arg w "$W" '
  .cwd |= sub($w; "__WORKSPACE__") | .transcript_path = "__TRANSCRIPT__"
  | walk(if type == "string" then sub($w; "__WORKSPACE__"; "g") else . end)' \
  /tmp/ripwire-bash-hook.json > tests/fixtures/hooks/claude_code_post_tool_use_bash.json
grep -nE 'aquental|/Users|/private' tests/fixtures/hooks/claude_code_post_tool_use_bash.json || echo limpo
```

Confira que `tool_name` é `"Bash"` e `tool_input.command` existe. Se o nome for outro, **pare**:
`is_shell` e o matcher dependem dele, e a spec precisa mudar.

- [ ] **Passo 3: teste com o ripwire real**

Em `tests/cli.rs`:

```rust
#[test]
fn a_real_bash_payload_after_a_shell_edit_injects_the_edit_context() {
    require_ripwire!();
    let repo = common::sample_repo();
    let state = tempfile::tempdir().unwrap();
    let dir = state.path().to_str().unwrap();
    let ws = repo.path().to_str().unwrap();
    let fixture = std::fs::read_to_string(format!(
        "{}/tests/fixtures/hooks/claude_code_post_tool_use_bash.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
    .replace("__WORKSPACE__", ws);
    let session = serde_json::from_str::<serde_json::Value>(&fixture).unwrap()["session_id"]
        .as_str()
        .unwrap()
        .to_string();
    run(
        &["hook", "claude-code", "user-prompt-submit", "--state-dir", dir, "--workspace", ws],
        &prompt_event(repo.path(), &session, "how is login validated?"),
    );
    let auth = repo.path().join("src/auth.py");
    let text = std::fs::read_to_string(&auth).unwrap();
    std::fs::write(&auth, text.replace("return user", "return user  # checked")).unwrap();

    let (code, out, err) = run(
        &["hook", "claude-code", "post-tool-use", "--state-dir", dir, "--workspace", ws],
        &fixture,
    );

    assert_eq!(code, 0, "{err}");
    let out: serde_json::Value = serde_json::from_str(&out).expect("an answer");
    let context = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    assert!(context.contains("context_after_edit"), "{context}");
    assert!(context.contains("auth.py"), "{context}");
}
```

Este teste também cobre o fio `run` → `shell_edits` → `respond` que a Tarefa 3 só mutou no nível
do `handle`. Se o `sample_repo` não produzir novidade (`has_news` falso) para essa edição, o
`out` fica vazio: troque a asserção para ler o estado salvo (`StateStore::new(..).load(&session)`)
e exigir `stats.events == 2` e `last_analysis.event == "PostToolUse"` no `statusline`.

- [ ] **Passo 4: rodar e ver passar**

Run: `cargo test --locked --test cli a_real_bash_payload`
Expected: PASS (ou "skipping: ripwire not on PATH" — rode onde o ripwire existe).

- [ ] **Passo 5: commit**

```bash
git add tests/fixtures/hooks/claude_code_post_tool_use_bash.json tests/cli.rs
git commit -m "hooks: a recorded Claude Code Bash PostToolUse and the whole path with real ripwire (D-129)"
```

---

### Tarefa 6: medir o custo

**Arquivos:** nenhum no repositório; os números vão para o D-129 (Tarefa 7).

- [ ] **Passo 1: medir um Bash só de leitura, em release**

```bash
cargo build --release --locked
B=$PWD/target/release/ripwire-broker
measure() {  # $1 = workspace git
  S=$(mktemp -d)
  python3 - "$B" "$1" "$S" <<'PY'
import json, subprocess, sys, time
b, ws, st = sys.argv[1:]
ev = lambda **k: json.dumps({"session_id": "m", "cwd": ws, **k})
args = lambda e: [b, "hook", "claude-code", e, "--workspace", ws, "--state-dir", st]
subprocess.run(args("user-prompt-submit"), input=ev(hook_event_name="UserPromptSubmit", prompt="x"), text=True, capture_output=True)
t = []
for _ in range(50):
    s = time.perf_counter()
    subprocess.run(args("post-tool-use"), input=ev(hook_event_name="PostToolUse", tool_name="Bash", tool_input={"command": "ls"}), text=True, capture_output=True)
    t.append((time.perf_counter() - s) * 1000)
t.sort()
print(f"{ws}: p50 {t[24]:.1f} ms, p95 {t[47]:.1f} ms, max {t[-1]:.1f} ms")
PY
}
measure "$PWD"
measure /Users/aquental/projects/ai/CECI/ceci_app   # o repositório grande; troque se preferir outro
```

Isto é o tempo inteiro do processo do hook num Bash só de leitura (subida do binário + `git`),
que **substitui** a subida do ripwire de antes. Para isolar o acréscimo, meça também
`git -C <ws> status --porcelain=v1 -z --untracked-files=all` sozinho (50 vezes, mesmo script).

- [ ] **Passo 2: decidir**

p95 do processo inteiro ≤ 50 ms no repositório grande: segue. Acima: **pare** e reporte ao
mantenedor antes do merge (spec §2.3). Anote os números para o D-129.

---

### Tarefa 7: documentação e changelog

**Arquivos:**
- Modificar: `spec/ripwire-broker-mcp.md` (PRD, seção dos hooks — 8.4 — e o §24.10 do roteiro manual)
- Modificar: `README.md` (tabela de hooks e nota de atualização)
- Modificar: `spec/changelog.md` (linha na tabela + seção D-129)
- Modificar: `spec/plan/xtd/proposta-edicoes-por-shell.md` (Status: feito em D-129)
- Fora do repositório: `statusline-manual/ROTEIRO.md` (passo 3.2, variante Bash)

- [ ] **Passo 1: PRD**

Na seção 8.4, depois do parágrafo que descreve o PostToolUse, acrescente:

```markdown
**Edições pelo shell (D-129).** No Claude Code o PostToolUse também casa `Bash`. Antes de subir o
ripwire, o hook compara uma impressão digital da árvore do git (`git status` e `mtime`/`size` dos
arquivos sujos) com a do hook anterior: só um comando que mudou arquivos segue para o
`context_after_edit`, com no máximo 50 arquivos; um comando só de leitura não sobe o ripwire e não
conta evento. Todo evento do Claude Code menos o `Stop` atualiza a impressão. Limites: só em
workspace git (fora dele a edição pelo shell só é vista pelo gate do `Stop`); mais de 5000 arquivos
sujos desliga a detecção; uma mudança feita por outro processo durante o comando é atribuída a ele.
Custo medido: <p50/p95 da Tarefa 6>.
```

Substitua `<p50/p95 da Tarefa 6>` pelos números medidos.

- [ ] **Passo 2: README**

Na descrição dos hooks, troque a menção ao matcher para `Edit|Write|MultiEdit|NotebookEdit|Bash` (se
houver) e acrescente na seção de atualização/migração:

```markdown
- **D-129:** edits made through the Bash tool now reach the edit hook in git workspaces. Re-run
  `ripwire-broker install claude-code --workspace DIR --hooks --write` to add `Bash` to the
  `PostToolUse` matcher; older installs keep working without it.
```

- [ ] **Passo 3: changelog**

Linha no topo da tabela (com a data e hora reais) e a seção `## D-129 — Edições pelo shell chegam
ao hook de edição` no fim do arquivo, no formato do D-127: o que mudou, a tabela "Mudança · Teste ·
Mutação (pega)" das Tarefas 1–4, a fixture da Tarefa 5, os números da Tarefa 6, as mutações que
nenhum teste pegou (com o porquê), e a contagem final de `cargo test --all-targets --locked` com e
sem `--features online`. Diga que fecha a divergência 2 do D-128.

- [ ] **Passo 4: proposta e roteiro**

Na proposta, troque a linha de Status por `**feito em [D-129](../../changelog.md#d-129--edições-pelo-shell-chegam-ao-hook-de-edição)**`.
No `ROTEIRO.md` local, no passo 3.2, acrescente a variante: "peça também uma edição com Bash (ex.:
`echo '// x' >> src/lib.rs`): `stats.events` sobe, e o `hook-log` ganha linha `PostToolUse` se houve
novidade".

- [ ] **Passo 5: verificação final e commit**

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo clippy --all-targets --locked --features online -- -D warnings
cargo test --all-targets --locked
cargo test --all-targets --locked --features online
cargo tree --locked -e normal | grep -Ei 'reqwest|secrecy|rustls|hyper'   # CA-10: sem saída
git add spec/ripwire-broker-mcp.md README.md spec/changelog.md spec/plan/xtd/proposta-edicoes-por-shell.md
git commit -m "docs: shell edits reach the edit hook (D-129)"
```
