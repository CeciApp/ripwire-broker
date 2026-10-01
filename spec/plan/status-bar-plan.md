# Barra de status — Plano de implementação

> **Para agentes:** sub-skill obrigatória: `superpowers:subagent-driven-development` (recomendada) ou
> `superpowers:executing-plans`, tarefa por tarefa. Os passos usam checkbox (`- [ ]`).

**Objetivo:** um subcomando `ripwire-broker statusline` que imprime a barra de status do Claude Code
a partir de uma projeção pequena publicada pelos hooks, sem rede, sem MCP e sem iniciar o ripwire.

**Arquitetura:** os hooks, que já salvam `SessionState` sob lock, passam a guardar um resumo da
última análise e da última entrega, e publicam depois do `save` uma projeção atômica em
`<state-dir>/statusline/<hash>.json`. O `statusline` lê só esse arquivo, sem lock, e passa a
projeção e o JSON do host a uma função pura de renderização. O instalador ganha `--statusline`, que
entra na **mesma** alteração do `.claude/settings.json` que os hooks.

**Stack:** Rust 1.98.1 (edition 2024), `serde`/`serde_json`, `sha2`, e uma dependência nova:
`unicode-width` (decisão D1 abaixo).

**Spec:** o [§24 do PRD](../ripwire-broker-mcp.md#24-barra-de-status-do-claude-code), que transporta a
antiga `spec/status-bar.md` ([D-122](../changelog.md#d-122--a-barra-de-status-entra-no-prd)). Este plano
argumenta a partir dele: leia os dois. **Onde este plano diz "spec §N", leia §24.N do PRD** (a seção
`N` da spec virou o §24.`N`); as decisões D1 a D6 abaixo estão também no §24.13.

## Deve vir antes da Fase 6?

**Sim.**

- **É independente.** Não depende de nenhum item da Fase 6, e nenhum item da Fase 6 depende dela.
  A ordem, então, é decidida por custo e valor.
- **É pequena e local:** um subcomando, um arquivo de projeção, um merge no instalador. Não tem
  rede, não mexe no CA-10 e não precisa das decisões abertas da Fase 6 (§21.4, motivação).
- **Ajuda uma medição em andamento.** A medição de `session_hits` (PRD §21.3) depende de os hooks
  estarem ativos em uso real por dias. Hoje, nada na interface mostra se eles rodam: na primeira
  sessão desta máquina, `hook-stats` só mostrou injeção depois de vários prompts. A barra torna isso
  visível a cada turno (`hooks on`, `inj 7`), e um hook quebrado aparece em minutos, não depois de 20
  sessões perdidas.
- **A Fase 6 é grande e ainda não tem desenho.** Começar por ela adiaria a barra por semanas sem
  ganho.

## Decisões para a revisão do mantenedor

A spec deixa estes pontos em aberto, ou os resolve de um jeito que o código revelou ser ambíguo. O
plano segue a recomendação de cada um; se alguma mudar, as tarefas afetadas estão indicadas.

| # | ponto | recomendação seguida | tarefas |
| --- | --- | --- | --- |
| D1 | largura Unicode (§5.3) | **`unicode-width = "0.2"`**: crate novo no grafo (conferido: não está no `Cargo.lock`), sem dependências, sem rede, o mesmo que o rustc usa. A alternativa é uma tabela Unicode escrita à mão, fácil de errar | 3 |
| D2 | o que sobra sob largura extrema (§5.3) | a spec preserva "prefixo, contexto, pausa dos hooks e alerta da última análise". **`hooks on`, `hooks sem dados`, `última: pronta` e `última: incerta` não são pausa nem alerta**, e saem depois do modelo. Essenciais: `rw-brkr`, `ctx N%`, `hooks off`, `última: atenção`, `última: erro` | 3 |
| D3 | quais hooks publicam | **só os do Claude Code.** A barra só existe lá; o host entra na chave do arquivo e no snapshot, então publicar para o Codex depois é uma linha | 7 |
| D4 | falha do `local::launch` | a análise vira `erro` e o estado é salvo e publicado. **Os contadores não mudam:** hoje esse caminho não conta evento, e mudar isso mudaria o `hook-stats` | 7 |
| D5 | barra alheia herdada (§7) | barra alheia **no settings do usuário**: a do broker não é escrita no projeto, porque a sombrearia; vira nota com o trecho manual. Barra alheia **no `settings.local.json`**: a do broker é escrita, com nota de que a local prevalece. Settings do usuário ilegível: nada é escrito, nota com trecho manual | 8 |
| D6 | payload de agente (§8) | presença de `agent` (objeto) no JSON do host: mostra só os segmentos do host e `agente`, sem ler snapshot | 2, 5 |

**Defeito existente, fora do escopo, registrado aqui porque a barra o torna visível:** quando o
`local::launch` falha, `hook::run` retorna antes de `handle`, e o `#ripwire-on` desse prompt não é
processado. Com o ripwire ausente, a barra mostraria `hooks off` até um prompt com o ripwire de pé.
Não corrigir nesta entrega; anotar no D-123 e no handoff.

## Restrições globais

Copiadas da spec. Todo requisito de tarefa inclui estas implicitamente.

- Prefixo fixo `rw-brkr`; o executável continua `ripwire-broker`.
- Separador ` · ` entre segmentos; uma linha, sem quebra interna.
- Effort: `low`→`low`, `medium`→`mid`, `high`→`hig`, `xhigh`→`xtr`, `max`→`max`; qualquer outro valor é omitido.
- `--color never` é o padrão; `--color always` gera ANSI mesmo sem TTY e mesmo com `NO_COLOR`.
- Cores do `ctx`, pelo inteiro exibido: `0 <= ctx < 40` → `"\x1b[38;5;250m"`; `40 <= ctx < 60` → `"\x1b[97m"`; `60 <= ctx <= 80` → `"\x1b[33m"`; `80 < ctx <= 100` → `"\x1b[31m"`; reset `"\x1b[0m"` ao fim de cada segmento colorido.
- Largura: `--width N`, depois `COLUMNS`, depois 100.
- Limites: stdin até 256 KiB; snapshot até 16 KiB; ler até limite + 1 para detectar excesso.
- Permissões: diretórios `0700`, arquivos `0600`; snapshot que não é arquivo regular, ou é symlink, conta como ausente.
- `statusline` nunca chama `settings`, `local::launch`, `Broker::connect` nem `Broker::status`, e nunca cria arquivo ou diretório.
- Sem `session_id`, nenhuma sessão é lida (nunca a `default`).
- Snapshot sem prompts, código, caminhos em claro, símbolos ou fingerprints.
- Comando instalado: `'<binário>' statusline --workspace '<raiz>' --color never`; sem `refreshInterval` automático.
- Nenhum helper Node, Python ou shell obrigatório; nenhum framework de CLI novo.
- Commits e código em inglês; docs de `spec/` em português; toda decisão vira `D-NNN` em `spec/changelog.md`.
- TDD por costura pública: o teste vermelho antes, em `tests/`; nada de testar função privada. Mutação para provar que um teste morde.

## Foco da revisão

Entradas que a spec implica e que seria fácil não testar. Cada uma tem um teste na tarefa dona do
código.

1. **Campo do host com o tipo errado** (`model` como string, `used_percentage` como `"32"`, `effort`
   como número): o segmento some e o resto renderiza. Teste em `host_fields_with_the_wrong_type_are_omitted`, na Tarefa 2.
2. **Snapshot gravado por uma versão mais nova** (`schema_version: 2`): `hooks sem dados`, nunca
   uma leitura parcial. Teste em `a_newer_schema_reads_as_incompatible`, na Tarefa 4.
3. **Diretório `statusline/` impossível de criar** (um arquivo com esse nome no state-dir): a saída
   do hook não muda, e o evento seguinte, já sem o obstáculo, publica os totais certos. Teste em
   `a_failed_publication_changes_nothing_and_the_next_event_repairs_it`, na Tarefa 7.
4. **Duas janelas do Claude no mesmo workspace** (sessões diferentes): cada barra mostra a sua.
   Teste em `sessions_hosts_and_workspaces_do_not_share_a_projection`, na Tarefa 4.
5. **`session_id` com `/`, `..` ou 10 KiB**: o nome do arquivo é um hash, e nada escapa do
   diretório. Teste em `hostile_session_ids_stay_inside_the_directory`, na Tarefa 4.

---

## Mapa de arquivos

| arquivo | responsabilidade | tarefas |
| --- | --- | --- |
| `Cargo.toml` | `unicode-width = "0.2"` em `[dependencies]` | 3 |
| `src/cli.rs` | `StatuslineArgs`, `Color`, `Command::Statusline`, `InstallArgs.statusline`, flags novas, `USAGE` | 1 |
| `src/statusline.rs` (novo) | `HostInput`, `parse_input`, `model_label`, `Options`, `render`, `sanitize` | 2, 3 |
| `src/statusline_state.rs` (novo) | `Snapshot`, `Analysis`, `Delivery`, `Summary`, `Read`, `bind`, `project`, `publish`, `read` | 4, 6 |
| `src/state.rs` | extrair `write_private` de `StateStore::save`, sem mudar formato nem lugar | 4 |
| `src/lib.rs` | `pub mod statusline; pub mod statusline_state;` | 2, 4 |
| `src/main.rs` | despacho de `Command::Statusline` antes de qualquer configuração | 5 |
| `src/hook.rs` | `SessionState.statusline`, registro de análise e entrega, publicação no `run` | 6, 7 |
| `src/install.rs` | `--statusline`: merge no mesmo `Change`, propriedade estrutural, conflitos | 8 |
| `tests/statusline.rs` (novo) | renderização, CLI do `statusline`, armazenamento | 2, 3, 4, 5 |
| `tests/hooks.rs` | produção do resumo via `hook::handle` | 6 |
| `tests/cli.rs` | parse, publicação via binário `hook`, instalação | 1, 7, 8 |
| `README.md`, `spec/changelog.md`, `handoff.md`, `spec/ripwire-broker-mcp.md` | docs, D-123, migração, medição | 9 |

**Como rodar um teste:** `cargo test --test statusline nome_do_teste`. **Verificação final:**
`cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`cargo test --all-targets --locked`, e os dois últimos também com `--features online`. Verificar
com `if cargo …; then`, nunca com pipe (armadilha do handoff).

---

### Tarefa 1: CLI do `statusline` e do `install --statusline`

**Arquivos:**
- Modificar: `src/cli.rs` (`USAGE`, structs, `Flags`, `SWITCHES`, `flags`, `parse`)
- Teste: `tests/cli.rs` (perto dos testes de `parse` existentes, ~linha 40)

**Interfaces:**
- Produz:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum Color { Never, Always }

  #[derive(Debug, Clone, PartialEq)]
  pub struct StatuslineArgs {
      pub workspace: Option<PathBuf>,
      pub state_dir: Option<PathBuf>,
      pub detail: bool,
      pub width: Option<usize>,
      pub color: Color,
  }
  // Command::Statusline(StatuslineArgs)
  // InstallArgs { ..., pub statusline: bool }
  ```

- [ ] **Passo 1: testes vermelhos.** Em `tests/cli.rs`, que já tem um helper `parse(&[&str])`:

```rust
#[test]
fn statusline_parses_its_flags_and_defaults_to_no_color() {
    let Ok(Command::Statusline(s)) = parse(&["statusline"]) else { panic!() };
    assert_eq!(s.workspace, None);
    assert_eq!(s.color, Color::Never);
    assert!(!s.detail);
    assert_eq!(s.width, None);

    let Ok(Command::Statusline(s)) = parse(&[
        "statusline", "--workspace", "/r", "--state-dir", "/s",
        "--detail", "--width", "80", "--color", "always",
    ]) else { panic!() };
    assert_eq!(s.workspace, Some("/r".into()));
    assert_eq!(s.state_dir, Some("/s".into()));
    assert!(s.detail);
    assert_eq!(s.width, Some(80));
    assert_eq!(s.color, Color::Always);
}

#[test]
fn statusline_refuses_bad_values_and_foreign_flags() {
    for args in [
        &["statusline", "--color", "auto"][..],
        &["statusline", "--width", "wide"][..],
        &["statusline", "--ripwire", "x"][..],
        &["statusline", "extra"][..],
    ] {
        assert!(parse(args).is_err(), "{args:?}");
    }
}

#[test]
fn install_takes_statusline_only_for_claude_code() {
    let Ok(Command::Install(i)) =
        parse(&["install", "claude-code", "--workspace", "/r", "--statusline"])
    else { panic!() };
    assert!(i.statusline && !i.hooks);
    assert!(parse(&["install", "codex", "--workspace", "/r", "--statusline"]).is_err());
}

#[test]
fn statusline_help_does_not_read_stdin() {
    // `run` writes nothing to stdin and would block on a reader; --help must return at once.
    let (code, out, _) = run(&["statusline", "--help"], "");
    assert_eq!(code, 0);
    assert!(out.contains("statusline"), "{out}");
}
```

Acrescentar `Color` ao `use ripwire_broker::cli::{...}` do arquivo.

- [ ] **Passo 2: confirmar o vermelho.** `cargo test --test cli statusline` → falha de compilação
  (`Color`, `Command::Statusline` não existem).

- [ ] **Passo 3: implementar.** Em `src/cli.rs`:

  - `USAGE`: acrescentar, depois da linha do `install`, e trocar a do `install`:
    ```text
           ripwire-broker install <claude-code|codex> --workspace DIR [--hooks] [--statusline] [--write] [--codex-home DIR] [--online]
           ripwire-broker statusline [--workspace DIR] [--state-dir DIR] [--detail] [--width N] [--color never|always]
    ```
  - Os tipos `Color` e `StatuslineArgs` da seção Interfaces; `statusline: bool` em `InstallArgs`;
    a variante `Statusline(StatuslineArgs)` em `Command`, com doc comment
    `/// Prints the Claude Code status line from the hooks' projection; never starts ripwire.`
  - `Flags`: campos `width: Option<u64>` e `color: Option<String>`.
  - `SWITCHES`: `"--detail"` e `"--statusline"`.
  - Em `flags()`, no `match a.as_str()`:
    ```rust
    "--width" => f.width = Some(number(&value)?),
    "--color" => f.color = Some(value),
    ```
  - Em `parse()`, um braço novo antes de `Some(other)`:
    ```rust
    Some("statusline") => {
        let f = flags(it, &["--workspace", "--state-dir", "--detail", "--width", "--color"])?;
        no_words(&f)?;
        let color = match f.color.as_deref() {
            None | Some("never") => Color::Never,
            Some("always") => Color::Always,
            Some(other) => {
                return Err(usage(format_args!("--color takes never or always, not '{other}'")));
            }
        };
        Ok(Command::Statusline(StatuslineArgs {
            workspace: f.workspace.clone(),
            state_dir: f.state_dir.clone(),
            detail: f.on("--detail"),
            width: f.width.map(|w| w as usize),
            color,
        }))
    }
    ```
  - No braço `install`: aceitar `"--statusline"`, preencher `statusline: f.on("--statusline")` e,
    antes do `Ok`:
    ```rust
    if f.on("--statusline") && host != Host::ClaudeCode {
        return Err(usage("--statusline is only available to claude-code"));
    }
    ```
  - Em `src/main.rs`, para compilar: um braço provisório
    `Ok(Command::Statusline(_)) => return ExitCode::SUCCESS,` (a Tarefa 5 o substitui).
  - Doc comment do módulo: incluir `statusline` na lista de comandos.

- [ ] **Passo 4: verde.** `cargo test --test cli` → passa, inclusive os testes antigos.

- [ ] **Passo 5: commit.**
  ```bash
  git add src/cli.rs src/main.rs tests/cli.rs
  git commit -m "CLI: statusline subcommand and install --statusline"
  ```

---

### Tarefa 2: entrada do host e segmentos da barra

Sem largura nem cor: esta tarefa monta a lista de segmentos e os junta. A Tarefa 3 faz caber e
colore.

**Arquivos:**
- Criar: `src/statusline.rs`
- Modificar: `src/lib.rs` (`pub mod statusline;`)
- Criar: `tests/statusline.rs`

**Interfaces:**
- Consome (da Tarefa 4, mas só o tipo; a Tarefa 2 vem antes, então ela **define** um esboço mínimo
  que a Tarefa 4 completa): `ripwire_broker::statusline_state::{Snapshot, VisibleStats, Analysis,
  AnalysisStatus, Delivery}`. **Para não inverter a ordem, a Tarefa 2 cria `src/statusline_state.rs`
  só com esses tipos (código abaixo), e a Tarefa 4 acrescenta as funções.**
- Produz:
  ```rust
  pub struct HostInput {
      pub session_id: Option<String>,
      pub project_dir: Option<PathBuf>,
      pub current_dir: Option<PathBuf>,
      pub cwd: Option<PathBuf>,
      pub model_name: Option<String>,
      pub model_id: Option<String>,
      pub effort: Option<String>,
      pub ctx_percent: Option<f64>,
      pub agent: bool,
  }
  pub fn parse_input(text: &str) -> HostInput;            // tolerante: inválido → Default
  pub fn model_label(name: Option<&str>, id: Option<&str>, effort: Option<&str>) -> Option<String>;
  pub fn sanitize(text: &str) -> String;                  // tira controles, ESC, newline
  pub struct Options { pub detail: bool, pub width: usize, pub color: bool }
  pub fn render(input: &HostInput, snapshot: Option<&Snapshot>, options: &Options, now: u64) -> String;
  ```

- [ ] **Passo 1: os tipos do snapshot** (só dados, sem testes próprios). Criar `src/statusline_state.rs`:

```rust
//! The status line's projection (PRD §24.6.3): a small private file per host session
//! and workspace, written by the hooks after they save their state, read by `statusline` without a
//! lock. Counts and kinds only: no prompt, code, plain path, symbol or fingerprint.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub host: String,
    pub workspace_key: String,
    pub updated_at: u64,
    pub opted_out: bool,
    pub stats: VisibleStats,
    pub last_analysis: Option<Analysis>,
    pub last_delivery: Option<Delivery>,
}

/// The session's tally since this workspace was bound to it (`SessionTally - baseline`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VisibleStats {
    pub events: u64,
    pub injections: u64,
    pub delivered: u64,
    pub session_hits: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub at: u64,
    pub event: String,
    pub status: AnalysisStatus,
    /// A `BrokerError::error` kind (a fixed vocabulary), never a message.
    pub error_kind: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus { Ready, AttentionRequired, Unknown, Error }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delivery {
    pub at: u64,
    pub estimated_tokens: u32,
}
```

E em `src/lib.rs`: `pub mod statusline;` e `pub mod statusline_state;`, em ordem alfabética.

- [ ] **Passo 2: testes vermelhos.** Criar `tests/statusline.rs`:

```rust
//! The status line (PRD §24): rendering, the `statusline` command and its projection.
use ripwire_broker::statusline::{self, HostInput, Options, parse_input, model_label, render};
use ripwire_broker::statusline_state::*;

const WIDE: Options = Options { detail: false, width: 1000, color: false };

fn host(json: &str) -> HostInput { parse_input(json) }

fn snap(opted_out: bool, injections: u64, hits: u64, last: Option<AnalysisStatus>) -> Snapshot {
    Snapshot {
        schema_version: SCHEMA_VERSION,
        host: "claude-code".into(),
        workspace_key: "k".into(),
        updated_at: 1_000,
        opted_out,
        stats: VisibleStats { events: 3, injections, delivered: 25, session_hits: hits },
        last_analysis: last.map(|status| Analysis {
            at: 990, event: "Stop".into(), status,
            error_kind: (status == AnalysisStatus::Error).then(|| "upstream_unavailable".into()),
        }),
        last_delivery: Some(Delivery { at: 970, estimated_tokens: 1240 }),
    }
}

const SONNET: &str = r#"{"session_id":"s","model":{"display_name":"Sonnet","id":"claude-sonnet-4-6"},
  "effort":{"level":"high"},"context_window":{"used_percentage":32}}"#;

#[test]
fn the_spec_examples_render_as_written() {
    let s = snap(false, 7, 18, Some(AnalysisStatus::AttentionRequired));
    assert_eq!(
        render(&host(SONNET), Some(&s), &WIDE, 1_000),
        "rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks on · última: atenção · inj 7 · não reenviados 18"
    );
    let off = snap(true, 7, 18, None);
    assert_eq!(
        render(&host(SONNET), Some(&off), &WIDE, 1_000),
        "rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks off · inj 7 · não reenviados 18"
    );
    assert_eq!(render(&host(SONNET), None, &WIDE, 1_000),
               "rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks sem dados");
    let err = snap(false, 0, 0, Some(AnalysisStatus::Error));
    assert!(render(&host(SONNET), Some(&err), &WIDE, 1_000).contains("última: erro"));
}

#[test]
fn status_words_map_one_to_one() {
    for (status, word) in [
        (AnalysisStatus::Ready, "última: pronta"),
        (AnalysisStatus::AttentionRequired, "última: atenção"),
        (AnalysisStatus::Unknown, "última: incerta"),
        (AnalysisStatus::Error, "última: erro"),
    ] {
        let line = render(&host(SONNET), Some(&snap(false, 1, 1, Some(status))), &WIDE, 1_000);
        assert!(line.contains(word), "{line}");
    }
}

#[test]
fn model_versions_come_from_the_name_or_an_unambiguous_id() {
    assert_eq!(model_label(Some("Sonnet"), Some("claude-sonnet-4-6"), None).as_deref(), Some("Sonnet 4.6"));
    assert_eq!(model_label(Some("Opus 4.6"), Some("claude-opus-4-6"), None).as_deref(), Some("Opus 4.6"));
    assert_eq!(model_label(Some("Haiku"), Some("claude-haiku-4-5-20251001"), None).as_deref(), Some("Haiku 4.5"));
    // A date is not a version; an alias or another family gives no version.
    assert_eq!(model_label(Some("Haiku"), Some("claude-haiku-20251001"), None).as_deref(), Some("Haiku"));
    assert_eq!(model_label(Some("Sonnet"), Some("sonnet"), None).as_deref(), Some("Sonnet"));
    assert_eq!(model_label(Some("Sonnet"), Some("claude-opus-4-6"), None).as_deref(), Some("Sonnet"));
    assert_eq!(model_label(Some("Sonnet"), None, None).as_deref(), Some("Sonnet"));
    assert_eq!(model_label(None, Some("claude-sonnet-4-6"), Some("high")), None);
}

#[test]
fn effort_has_five_labels_and_nothing_else() {
    for (level, label) in [("low", "low"), ("medium", "mid"), ("high", "hig"), ("xhigh", "xtr"), ("max", "max")] {
        assert_eq!(model_label(Some("Opus 5.5"), None, Some(level)).unwrap(), format!("Opus 5.5 {label}"));
    }
    for odd in ["", "HIGH", "ultra"] {
        assert_eq!(model_label(Some("Opus 5.5"), None, Some(odd)).unwrap(), "Opus 5.5");
    }
}

#[test]
fn zero_context_shows_and_missing_or_out_of_range_does_not() {
    let line = |pct: &str| render(&host(&format!(
        r#"{{"model":{{"display_name":"Opus"}},"context_window":{{"used_percentage":{pct}}}}}"#)),
        None, &WIDE, 0);
    assert!(line("0").contains("ctx 0%"));
    assert!(line("32.5").contains("ctx 33%"), "rounded: {}", line("32.5"));
    for absent in ["null", "-1", "100.6", "\"32\""] {
        assert!(!line(absent).contains("ctx"), "{absent}: {}", line(absent));
    }
}

#[test]
fn host_fields_with_the_wrong_type_are_omitted() {
    let line = render(&host(r#"{"model":"Sonnet","effort":3,"context_window":[]}"#), None, &WIDE, 0);
    assert_eq!(line, "rw-brkr · hooks sem dados");
}

#[test]
fn invalid_or_empty_input_degrades_to_the_prefix() {
    for text in ["", "{", "[]", "null", "\u{0}"] {
        assert_eq!(render(&host(text), None, &WIDE, 0), "rw-brkr · hooks sem dados", "{text:?}");
    }
}

#[test]
fn external_text_cannot_reach_the_terminal_as_control() {
    let line = render(&host(r#"{"model":{"display_name":"Son\u001b[31mnet\nX\u0007"}}"#), None, &WIDE, 0);
    assert!(!line.chars().any(|c| c.is_control()), "{line:?}");
    assert!(line.contains("Son[31mnetX"), "{line:?}");
}

#[test]
fn an_agent_payload_shows_host_segments_only() {
    let agent = r#"{"session_id":"s","agent":{"name":"reviewer"},"model":{"display_name":"Opus 5.5"}}"#;
    let s = snap(false, 7, 18, Some(AnalysisStatus::AttentionRequired));
    assert_eq!(render(&host(agent), Some(&s), &WIDE, 1_000), "rw-brkr · Opus 5.5 · agente");
}

#[test]
fn detail_adds_delivered_reuse_last_context_and_age() {
    let opts = Options { detail: true, ..WIDE };
    let s = snap(false, 7, 18, Some(AnalysisStatus::Ready));
    let line = render(&host(SONNET), Some(&s), &opts, 1_020);
    assert!(line.ends_with("· entregues 25 · reuso 42% · último contexto ~1,2k tok · há 20s"), "{line}");
    let old = render(&host(SONNET), Some(&s), &opts, 1_000 + 301);
    assert!(old.contains("dados antigos"), "{old}");
    let mut none = s.clone();
    none.stats.delivered = 0;
    none.stats.session_hits = 0;
    none.last_delivery = None;
    let line = render(&host(SONNET), Some(&none), &opts, 1_020);
    assert!(!line.contains("reuso") && !line.contains("contexto ~"), "no fictitious rate: {line}");
}
```

  A conta do `reuso`: 18 / (18 + 25) = 41,86 %, que arredonda para 42 %.

- [ ] **Passo 3: confirmar o vermelho.** `cargo test --test statusline` → não compila
  (`statusline::render` não existe).

- [ ] **Passo 4: implementar `src/statusline.rs`.**

```rust
//! The `statusline` renderer (PRD §24.5): the host's JSON and the hooks' projection
//! become one line. Pure: no clock, no disk, no environment; the caller passes all of them.

use crate::statusline_state::{AnalysisStatus, Snapshot};
use serde_json::Value;
use std::path::PathBuf;

pub const PREFIX: &str = "rw-brkr";
pub const SEPARATOR: &str = " · ";
/// After this many seconds, `--detail` says the data is old (§5.2).
pub const STALE_SECS: u64 = 300;
const MAX_MODEL_CHARS: usize = 32;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostInput {
    pub session_id: Option<String>,
    pub project_dir: Option<PathBuf>,
    pub current_dir: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub model_name: Option<String>,
    pub model_id: Option<String>,
    pub effort: Option<String>,
    pub ctx_percent: Option<f64>,
    pub agent: bool,
}

fn text(v: &Value, path: &[&str]) -> Option<String> {
    let mut v = v;
    for key in path {
        v = v.get(key)?;
    }
    v.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

/// Every field is optional and type-checked: a wrong type is an absent field (spec §2).
pub fn parse_input(text_in: &str) -> HostInput {
    let Ok(v) = serde_json::from_str::<Value>(text_in) else {
        return HostInput::default();
    };
    if !v.is_object() {
        return HostInput::default();
    }
    HostInput {
        session_id: text(&v, &["session_id"]),
        project_dir: text(&v, &["workspace", "project_dir"]).map(PathBuf::from),
        current_dir: text(&v, &["workspace", "current_dir"]).map(PathBuf::from),
        cwd: text(&v, &["cwd"]).map(PathBuf::from),
        model_name: text(&v, &["model", "display_name"]),
        model_id: text(&v, &["model", "id"]),
        effort: text(&v, &["effort", "level"]),
        ctx_percent: v
            .get("context_window")
            .and_then(|c| c.get("used_percentage"))
            .and_then(Value::as_f64),
        agent: v.get("agent").is_some_and(Value::is_object),
    }
}

/// Drops control characters (ESC, newline, BEL...), so external text cannot steer the terminal.
pub fn sanitize(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

fn effort_label(level: &str) -> Option<&'static str> {
    match level {
        "low" => Some("low"),
        "medium" => Some("mid"),
        "high" => Some("hig"),
        "xhigh" => Some("xtr"),
        "max" => Some("max"),
        _ => None,
    }
}

/// `claude-<family>-<n>[-<n>...]`: the 1–2 digit parts after the family, stopped by the first part
/// that is not one (a date, `[1m]`...). Only when the family is the display name's first word.
fn version_from_id(name: &str, id: &str) -> Option<String> {
    let family = name.split_whitespace().next()?.to_ascii_lowercase();
    let rest = id.strip_prefix("claude-")?.strip_prefix(family.as_str())?.strip_prefix('-')?;
    let parts: Vec<&str> = rest
        .split('-')
        .take_while(|p| (1..=2).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
        .collect();
    (!parts.is_empty()).then(|| parts.join("."))
}

pub fn model_label(name: Option<&str>, id: Option<&str>, effort: Option<&str>) -> Option<String> {
    let name = sanitize(name?.trim());
    if name.is_empty() {
        return None;
    }
    let mut label = name.clone();
    if !name.bytes().any(|b| b.is_ascii_digit())
        && let Some(v) = id.and_then(|id| version_from_id(&name, &sanitize(id)))
    {
        label = format!("{name} {v}");
    }
    if label.chars().count() > MAX_MODEL_CHARS {
        label = label.chars().take(MAX_MODEL_CHARS - 1).chain(['…']).collect();
    }
    if let Some(e) = effort.and_then(effort_label) {
        label = format!("{label} {e}");
    }
    Some(label)
}

/// How a segment is drawn, and how early it goes when the line is too wide (Task 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style { Plain, Gray, White, Yellow, Red }

/// Dropped first to last: Detail, Counter, Model, Soft. Essential is never dropped (D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Keep { Detail, Counter, Model, Soft, Essential }

#[derive(Debug, Clone, PartialEq)]
pub struct Segment { pub text: String, pub keep: Keep, pub style: Style }

fn seg(text: impl Into<String>, keep: Keep, style: Style) -> Segment {
    Segment { text: text.into(), keep, style }
}

pub fn ctx_style(pct: u32) -> Style {
    match pct {
        0..=39 => Style::Gray,
        40..=59 => Style::White,
        60..=80 => Style::Yellow,
        _ => Style::Red,
    }
}

fn tokens(n: u32) -> String {
    if n < 1000 {
        return format!("~{n} tok");
    }
    let k = format!("{:.1}", f64::from(n) / 1000.0).replace('.', ",");
    format!("~{}k tok", k.strip_suffix(",0").unwrap_or(&k))
}

fn age(secs: u64) -> String {
    match secs {
        0..=59 => format!("há {secs}s"),
        60..=3599 => format!("há {}min", secs / 60),
        _ => format!("há {}h", secs / 3600),
    }
}

/// The segments in display order, before fitting (Task 3 drops and colors them).
pub fn segments(input: &HostInput, snapshot: Option<&Snapshot>, detail: bool, now: u64) -> Vec<Segment> {
    let mut out = vec![seg(PREFIX, Keep::Essential, Style::Plain)];
    if let Some(m) = model_label(input.model_name.as_deref(), input.model_id.as_deref(), input.effort.as_deref()) {
        out.push(seg(m, Keep::Model, Style::Plain));
    }
    if let Some(p) = input.ctx_percent.filter(|p| (0.0..=100.0).contains(p)) {
        let shown = p.round() as u32;
        out.push(seg(format!("ctx {shown}%"), Keep::Essential, ctx_style(shown)));
    }
    if input.agent {
        out.push(seg("agente", Keep::Soft, Style::Plain));
        return out;
    }
    let Some(s) = snapshot else {
        out.push(seg("hooks sem dados", Keep::Soft, Style::Plain));
        return out;
    };
    out.push(match s.opted_out {
        true => seg("hooks off", Keep::Essential, Style::Plain),
        false => seg("hooks on", Keep::Soft, Style::Plain),
    });
    if let Some(a) = &s.last_analysis {
        out.push(match a.status {
            AnalysisStatus::Ready => seg("última: pronta", Keep::Soft, Style::Plain),
            AnalysisStatus::Unknown => seg("última: incerta", Keep::Soft, Style::Plain),
            AnalysisStatus::AttentionRequired => seg("última: atenção", Keep::Essential, Style::Yellow),
            AnalysisStatus::Error => seg("última: erro", Keep::Essential, Style::Red),
        });
    }
    out.push(seg(format!("inj {}", s.stats.injections), Keep::Counter, Style::Plain));
    out.push(seg(format!("não reenviados {}", s.stats.session_hits), Keep::Counter, Style::Plain));
    if detail {
        out.push(seg(format!("entregues {}", s.stats.delivered), Keep::Detail, Style::Plain));
        let whole = s.stats.session_hits + s.stats.delivered;
        if whole > 0 {
            let rate = (s.stats.session_hits as f64 * 100.0 / whole as f64).round();
            out.push(seg(format!("reuso {rate}%"), Keep::Detail, Style::Plain));
        }
        if let Some(d) = &s.last_delivery {
            out.push(seg(format!("último contexto {}", tokens(d.estimated_tokens)), Keep::Detail, Style::Plain));
        }
        let elapsed = now.saturating_sub(s.updated_at);
        out.push(seg(age(elapsed), Keep::Detail, Style::Plain));
        if elapsed > STALE_SECS {
            out.push(seg("dados antigos", Keep::Detail, Style::Plain));
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options { pub detail: bool, pub width: usize, pub color: bool }

/// This task: join everything. Task 3 replaces the body with fit-then-color.
pub fn render(input: &HostInput, snapshot: Option<&Snapshot>, options: &Options, now: u64) -> String {
    segments(input, snapshot, options.detail, now)
        .into_iter()
        .map(|s| s.text)
        .collect::<Vec<_>>()
        .join(SEPARATOR)
}
```

  Observação: `format!("reuso {rate}%")` com `f64` imprime `42`, sem casas, porque `round()`
  devolve inteiro exato. Se o clippy reclamar do cast, use `rate as u64`.

- [ ] **Passo 5: verde.** `cargo test --test statusline` → passa.

- [ ] **Passo 6: mutação.** Trocar `"hig"` por `"hgh"` em `effort_label`, e `p.round()` por
  `p.floor()`: cada uma tem de derrubar um teste. Desfazer e `touch src/statusline.rs` (armadilha
  do mtime).

- [ ] **Passo 7: commit.**
  ```bash
  git add src/statusline.rs src/statusline_state.rs src/lib.rs tests/statusline.rs
  git commit -m "statusline: host input, model label and segments"
  ```

---

### Tarefa 3: largura visual e cores

**Arquivos:**
- Modificar: `Cargo.toml` (`unicode-width = "0.2"`), `src/statusline.rs` (`render`, `fit`, `paint`)
- Teste: `tests/statusline.rs`

**Interfaces:**
- Consome: `segments`, `Segment`, `Keep`, `Style`, `Options` (Tarefa 2).
- Produz: `pub fn width(text: &str) -> usize` e o `render` definitivo.

**Algoritmo (D2):** com `w = options.width`:
1. Remover, em ordem, todos os segmentos `Detail`, depois `Counter`, depois `Model`, depois `Soft`,
   **um nível inteiro por vez**, parando no primeiro nível que faz a linha caber.
2. Só essenciais e ainda não cabe: manter o prefixo e os alertas (`hooks off`, `última: atenção`,
   `última: erro`), sem o `ctx`.
3. Ainda não cabe: só `rw-brkr`.
4. `w` menor que 7: `rw-brkr` truncado em `w` colunas.
5. A cor entra **depois**: os escapes não contam como colunas.

- [ ] **Passo 1: testes vermelhos.** Em `tests/statusline.rs`:

```rust
use ripwire_broker::statusline::width;

fn opts(w: usize) -> Options { Options { detail: true, width: w, color: false } }

#[test]
fn lines_fit_40_80_and_120_columns_and_shed_in_order() {
    let s = snap(true, 7, 18, Some(AnalysisStatus::AttentionRequired));
    for w in [40, 80, 120] {
        let line = render(&host(SONNET), Some(&s), &opts(w), 1_020);
        assert!(width(&line) <= w, "{w}: {line}");
        assert!(line.starts_with("rw-brkr"), "{line}");
        assert!(line.contains("hooks off") && line.contains("última: atenção"), "alerts stay: {line}");
    }
    let line40 = render(&host(SONNET), Some(&s), &opts(40), 1_020);
    assert!(!line40.contains("entregues") && !line40.contains("inj "), "details and counters go first: {line40}");
}

#[test]
fn extreme_widths_keep_alerts_then_the_prefix() {
    let s = snap(true, 7, 18, Some(AnalysisStatus::Error));
    // 7 + 3 + 7 + 3 + 9 + 3 + 12 = 44 columns: the essentials exactly.
    let line = render(&host(SONNET), Some(&s), &opts(44), 1_020);
    assert_eq!(line, "rw-brkr · ctx 32% · hooks off · última: erro");
    assert_eq!(render(&host(SONNET), Some(&s), &opts(43), 1_020), "rw-brkr · hooks off · última: erro");
    assert_eq!(render(&host(SONNET), Some(&s), &opts(34), 1_020), "rw-brkr · hooks off · última: erro");
    assert_eq!(render(&host(SONNET), Some(&s), &opts(10), 1_020), "rw-brkr");
    assert_eq!(render(&host(SONNET), Some(&s), &opts(3), 1_020), "rw-");
}

#[test]
fn width_is_visual_not_bytes() {
    assert_eq!(width("não"), 3);
    assert_eq!(width("日本"), 4);
    let wide = r#"{"model":{"display_name":"モデル名前テスト"},"context_window":{"used_percentage":5}}"#;
    let line = render(&host(wide), None, &Options { detail: false, width: 30, color: false }, 0);
    assert!(width(&line) <= 30, "{line}");
}

fn ctx_line(pct: f64) -> String {
    render(&host(&format!(r#"{{"context_window":{{"used_percentage":{pct}}}}}"#)), None,
           &Options { detail: false, width: 100, color: true }, 0)
}

#[test]
fn ctx_colors_follow_the_rounded_value_and_reset_after_the_segment() {
    for (pct, code) in [
        (0.0, "\x1b[38;5;250m"), (39.0, "\x1b[38;5;250m"), (39.4, "\x1b[38;5;250m"),
        (39.5, "\x1b[97m"), (40.0, "\x1b[97m"), (59.0, "\x1b[97m"),
        (60.0, "\x1b[33m"), (80.0, "\x1b[33m"), (80.4, "\x1b[33m"),
        (80.5, "\x1b[31m"), (81.0, "\x1b[31m"), (100.0, "\x1b[31m"),
    ] {
        let line = ctx_line(pct);
        let shown = pct.round() as u32;
        assert!(line.contains(&format!("{code}ctx {shown}%\x1b[0m")), "{pct}: {line:?}");
    }
}

#[test]
fn color_never_has_no_escape_and_alerts_are_colored_when_asked() {
    let s = snap(false, 1, 1, Some(AnalysisStatus::Error));
    let plain = render(&host(SONNET), Some(&s), &Options { detail: false, width: 200, color: false }, 0);
    assert!(!plain.contains('\x1b'), "{plain:?}");
    let colored = render(&host(SONNET), Some(&s), &Options { detail: false, width: 200, color: true }, 0);
    assert!(colored.contains("\x1b[31múltima: erro\x1b[0m"), "{colored:?}");
    // The fit is computed without escapes: same visible text either way.
    let stripped = colored.replace("\x1b[38;5;250m", "").replace("\x1b[31m", "").replace("\x1b[0m", "");
    assert_eq!(stripped, plain);
}
```

- [ ] **Passo 2: vermelho.** `cargo test --test statusline` → falha (`width` não existe).

- [ ] **Passo 3: implementar.** `Cargo.toml`, em `[dependencies]`, com um comentário de uma linha:
  ```toml
  # Visual width of the status line (PRD §24.5.3, D-123): no deps, no network.
  unicode-width = "0.2"
  ```
  Em `src/statusline.rs`:

```rust
use unicode_width::UnicodeWidthStr;

pub fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn joined(segs: &[Segment]) -> String {
    segs.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(SEPARATOR)
}

/// The first `cols` columns of `text`, cut on a character boundary.
fn truncate(text: &str, cols: usize) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if width(&out) + width(c.encode_utf8(&mut [0; 4])) > cols {
            break;
        }
        out.push(c);
    }
    out
}

fn fit(mut segs: Vec<Segment>, cols: usize) -> Vec<Segment> {
    for level in [Keep::Detail, Keep::Counter, Keep::Model, Keep::Soft] {
        if width(&joined(&segs)) <= cols {
            return segs;
        }
        segs.retain(|s| s.keep != level);
    }
    if width(&joined(&segs)) <= cols {
        return segs;
    }
    segs.retain(|s| s.text == PREFIX || !s.text.starts_with("ctx "));
    if width(&joined(&segs)) <= cols {
        return segs;
    }
    vec![seg(truncate(PREFIX, cols), Keep::Essential, Style::Plain)]
}

fn paint(s: &Segment) -> String {
    let code = match s.style {
        Style::Plain => return s.text.clone(),
        Style::Gray => "\x1b[38;5;250m",
        Style::White => "\x1b[97m",
        Style::Yellow => "\x1b[33m",
        Style::Red => "\x1b[31m",
    };
    format!("{code}{}\x1b[0m", s.text)
}

pub fn render(input: &HostInput, snapshot: Option<&Snapshot>, options: &Options, now: u64) -> String {
    let segs = fit(segments(input, snapshot, options.detail, now), options.width);
    segs.iter()
        .map(|s| if options.color { paint(s) } else { s.text.clone() })
        .collect::<Vec<_>>()
        .join(SEPARATOR)
}
```

- [ ] **Passo 4: verde.** `cargo test --test statusline` → passa.

- [ ] **Passo 5: mutação.** Trocar `60..=80` por `60..=79` em `ctx_style`, e trocar a ordem de
  `Keep::Counter` e `Keep::Model` no `for` de `fit`: as duas têm de cair. Desfazer e `touch`.

- [ ] **Passo 6: CA-10 continua.** `cargo test --test mcp_surface the_build_has_no_network_stack`
  → passa (`unicode-width` não é crate de rede).

- [ ] **Passo 7: commit.**
  ```bash
  git add Cargo.toml Cargo.lock src/statusline.rs tests/statusline.rs
  git commit -m "statusline: fit to visual width, then color"
  ```

---

### Tarefa 4: a projeção em disco

**Arquivos:**
- Modificar: `src/statusline_state.rs` (funções), `src/state.rs` (extrair `write_private`)
- Teste: `tests/statusline.rs`

**Interfaces:**
- Consome: os tipos da Tarefa 2.
- Produz:
  ```rust
  pub const MAX_SNAPSHOT_BYTES: u64 = 16 * 1024;
  pub const HOST: &str = "claude-code";
  pub enum Read { Missing, Corrupt, Incompatible, Valid(Snapshot) }
  pub fn workspace_key(root: &Path) -> String;
  pub fn path(state_dir: &Path, host: &str, session_id: &str, root: &Path) -> PathBuf;
  pub fn publish(state_dir: &Path, session_id: &str, root: &Path, snapshot: &Snapshot) -> std::io::Result<()>;
  pub fn read(state_dir: &Path, host: &str, session_id: &str, root: &Path) -> Read;
  // em src/state.rs:
  pub(crate) fn write_private(dir: &Path, path: &Path, bytes: &[u8]) -> std::io::Result<()>;
  ```

- [ ] **Passo 1: testes vermelhos.** Em `tests/statusline.rs`:

```rust
use std::path::Path;

fn valid(root: &Path) -> Snapshot {
    Snapshot { workspace_key: workspace_key(root), ..snap(false, 2, 3, Some(AnalysisStatus::Ready)) }
}

#[test]
fn a_published_snapshot_reads_back_and_files_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/repo/a");
    publish(state.path(), "s-1", root, &valid(root)).unwrap();
    assert_eq!(read(state.path(), HOST, "s-1", root), Read::Valid(valid(root)));
    let file = path(state.path(), HOST, "s-1", root);
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(std::fs::metadata(file.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(!text.contains("s-1") && !text.contains("/repo/a"), "no plain id or path: {text}");
}

#[test]
fn sessions_hosts_and_workspaces_do_not_share_a_projection() {
    let state = tempfile::tempdir().unwrap();
    let (a, b) = (Path::new("/repo/a"), Path::new("/repo/a-worktree"));
    publish(state.path(), "s-1", a, &valid(a)).unwrap();
    assert_eq!(read(state.path(), HOST, "s-2", a), Read::Missing, "another window");
    assert_eq!(read(state.path(), HOST, "s-1", b), Read::Missing, "another worktree");
    assert_eq!(read(state.path(), "codex", "s-1", a), Read::Missing, "another host");
}

#[test]
fn hostile_session_ids_stay_inside_the_directory() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    for id in ["../../etc/passwd", "a/b", &"x".repeat(10_000)] {
        let p = path(state.path(), HOST, id, root);
        assert_eq!(p.parent().unwrap(), state.path().join("statusline"), "{id}");
        publish(state.path(), id, root, &valid(root)).unwrap();
    }
}

#[test]
fn absence_corruption_and_size_are_told_apart() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    assert_eq!(read(state.path(), HOST, "s", root), Read::Missing);
    publish(state.path(), "s", root, &valid(root)).unwrap();
    let file = path(state.path(), HOST, "s", root);
    std::fs::write(&file, "{not json").unwrap();
    assert_eq!(read(state.path(), HOST, "s", root), Read::Corrupt);
    std::fs::write(&file, " ".repeat(MAX_SNAPSHOT_BYTES as usize + 1)).unwrap();
    assert_eq!(read(state.path(), HOST, "s", root), Read::Corrupt, "over the limit");
}

#[test]
fn a_newer_schema_reads_as_incompatible() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    publish(state.path(), "s", root, &valid(root)).unwrap();
    let file = path(state.path(), HOST, "s", root);
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    v["schema_version"] = 2.into();
    v["stats"] = "reshaped".into();
    std::fs::write(&file, v.to_string()).unwrap();
    assert_eq!(read(state.path(), HOST, "s", root), Read::Incompatible);
}

#[test]
fn a_snapshot_for_another_workspace_key_is_incompatible() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    let wrong = Snapshot { workspace_key: workspace_key(Path::new("/other")), ..valid(root) };
    publish(state.path(), "s", root, &wrong).unwrap();
    assert_eq!(read(state.path(), HOST, "s", root), Read::Incompatible);
}

#[test]
fn symlinks_and_directories_read_as_missing() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    let file = path(state.path(), HOST, "s", root);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let target = state.path().join("elsewhere.json");
    std::fs::write(&target, serde_json::to_string(&valid(root)).unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &file).unwrap();
    assert_eq!(read(state.path(), HOST, "s", root), Read::Missing);
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert_eq!(read(state.path(), HOST, "s", root), Read::Missing);
}

#[test]
fn reading_creates_nothing() {
    let state = tempfile::tempdir().unwrap();
    let missing = state.path().join("never");
    assert_eq!(read(&missing, HOST, "s", Path::new("/r")), Read::Missing);
    assert!(!missing.exists());
}

#[test]
fn concurrent_readers_see_whole_versions_only() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    publish(state.path(), "s", root, &valid(root)).unwrap();
    let dir = state.path().to_path_buf();
    let writer = std::thread::spawn(move || {
        for i in 0..300 {
            let mut s = valid(Path::new("/r"));
            s.stats.injections = i;
            publish(&dir, "s", Path::new("/r"), &s).unwrap();
        }
    });
    for _ in 0..300 {
        assert!(matches!(read(state.path(), HOST, "s", root), Read::Valid(_)));
    }
    writer.join().unwrap();
}
```

- [ ] **Passo 2: vermelho.** `cargo test --test statusline` → não compila.

- [ ] **Passo 3: extrair o helper em `src/state.rs`.** O corpo de `StateStore::save` passa a ser
  `write_private(&self.dir, &self.path(session_id), serde_json::to_string(state)?.as_bytes())`, com:

```rust
/// Writes `bytes` to `path` through a private temporary file in `dir` and a rename: readers see
/// the old file or the new one, never half of it. Creates `dir` as 0700; the file is 0600.
pub(crate) fn write_private(dir: &Path, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&tmp, path)
}
```

  O nome do temporário não muda (`<hash>.tmp<pid>`): o `sessions()` já ignora tudo que não termina
  em `.json`. Rodar `cargo test --test hooks` → continua verde.

- [ ] **Passo 4: implementar as funções em `src/statusline_state.rs`.**

```rust
use sha2::{Digest, Sha256};
use std::io::Read as _;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub const MAX_SNAPSHOT_BYTES: u64 = 16 * 1024;
/// The only host that renders a status line today (D3).
pub const HOST: &str = "claude-code";

#[derive(Debug, Clone, PartialEq)]
pub enum Read { Missing, Corrupt, Incompatible, Valid(Snapshot) }

fn hex(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn workspace_key(root: &Path) -> String {
    hex(root.as_os_str().as_bytes())
}

/// sha256 over a length-prefixed tuple: no two (host, session, root) share an encoding, and no
/// session id ever becomes part of a path.
pub fn path(state_dir: &Path, host: &str, session_id: &str, root: &Path) -> PathBuf {
    let mut h = Sha256::new();
    for part in [b"ripwire-broker/statusline/v1".as_slice(), host.as_bytes(),
                 session_id.as_bytes(), root.as_os_str().as_bytes()] {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part);
    }
    state_dir.join("statusline").join(format!("{:x}.json", h.finalize()))
}

pub fn publish(state_dir: &Path, session_id: &str, root: &Path, snapshot: &Snapshot) -> std::io::Result<()> {
    let file = path(state_dir, &snapshot.host, session_id, root);
    let dir = file.parent().expect("statusline dir");
    crate::state::write_private(dir, &file, serde_json::to_string(snapshot)?.as_bytes())
}

/// Never waits for the hooks' lock and never creates anything.
pub fn read(state_dir: &Path, host: &str, session_id: &str, root: &Path) -> Read {
    let file = path(state_dir, host, session_id, root);
    match std::fs::symlink_metadata(&file) {
        Ok(m) if m.file_type().is_file() => {}
        _ => return Read::Missing,
    }
    let Ok(f) = std::fs::File::open(&file) else { return Read::Missing };
    let mut text = String::new();
    if f.take(MAX_SNAPSHOT_BYTES + 1).read_to_string(&mut text).is_err()
        || text.len() as u64 > MAX_SNAPSHOT_BYTES
    {
        return Read::Corrupt;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return Read::Corrupt };
    if v.get("schema_version").and_then(serde_json::Value::as_u64) != Some(u64::from(SCHEMA_VERSION)) {
        return Read::Incompatible;
    }
    let Ok(s) = serde_json::from_value::<Snapshot>(v) else { return Read::Corrupt };
    if s.host != host || s.workspace_key != workspace_key(root) {
        return Read::Incompatible;
    }
    Read::Valid(s)
}
```

- [ ] **Passo 5: verde.** `cargo test --test statusline` e `cargo test --test hooks` → passam.

- [ ] **Passo 6: mutação.** Remover o prefixo de tamanho do `path` (só concatenar), e trocar
  `is_file()` por `!is_dir()`: a primeira não derruba nenhum teste atual, então **acrescentar**
  `assert_ne!(path(d, "a", "bc", r), path(d, "ab", "c", r))` em
  `sessions_hosts_and_workspaces_do_not_share_a_projection` e confirmar que ela cai; a segunda
  derruba `symlinks_and_directories_read_as_missing`. Desfazer e `touch`.

- [ ] **Passo 7: commit.**
  ```bash
  git add src/statusline_state.rs src/state.rs tests/statusline.rs
  git commit -m "statusline: private atomic projection, typed read"
  ```

---

### Tarefa 5: o comando `statusline` no binário

**Arquivos:**
- Modificar: `src/main.rs` (substituir o braço provisório da Tarefa 1)
- Modificar: `src/statusline.rs` (`resolve_root`, `MAX_STDIN_BYTES`)
- Teste: `tests/statusline.rs`

**Interfaces:**
- Consome: `StatuslineArgs`, `Color` (T1); `parse_input`, `render`, `Options` (T2/T3); `read`, `Read`, `HOST` (T4); `StateStore::default_dir`.
- Produz: `pub const MAX_STDIN_BYTES: u64 = 256 * 1024;` e
  `pub fn resolve_root(flag: Option<&Path>, input: &HostInput) -> Option<PathBuf>`.

- [ ] **Passo 1: testes vermelhos.** Em `tests/statusline.rs`, um helper que roda o binário com
  ambiente controlado:

```rust
use std::io::Write;
use std::process::{Command as Proc, Stdio};

fn run_bar(args: &[&str], stdin: &[u8], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut cmd = Proc::new(env!("CARGO_BIN_EXE_ripwire-broker"));
    cmd.arg("statusline").args(args).env_remove("COLUMNS").env_remove("NO_COLOR");
    for (k, v) in env { cmd.env(k, v); }
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let _ = child.stdin.take().unwrap().write_all(stdin);
    let out = child.wait_with_output().unwrap();
    (out.status.code().unwrap_or(-1),
     String::from_utf8_lossy(&out.stdout).into_owned(),
     String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn the_manual_example_of_the_spec() {
    let ws = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let input = format!(
        r#"{{"session_id":"demo","workspace":{{"project_dir":"{}"}},"model":{{"display_name":"Sonnet","id":"claude-sonnet-4-6"}},"effort":{{"level":"high"}},"context_window":{{"used_percentage":32}}}}"#,
        ws.path().display());
    let (code, out, err) = run_bar(
        &["--workspace", ws.path().to_str().unwrap(), "--state-dir", state.path().to_str().unwrap()],
        input.as_bytes(), &[]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(out, "rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks sem dados\n");
    assert!(err.is_empty(), "{err}");
    let (_, colored, _) = run_bar(
        &["--workspace", ws.path().to_str().unwrap(), "--state-dir", state.path().to_str().unwrap(), "--color", "always"],
        input.as_bytes(), &[("NO_COLOR", "1")]);
    assert!(colored.contains("\x1b[38;5;250mctx 32%\x1b[0m"), "{colored:?}");
}

#[test]
fn it_reads_the_projection_of_this_session_and_workspace() {
    let ws = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();
    publish(state.path(), "s-1", &root, &Snapshot { workspace_key: workspace_key(&root),
        ..snap(false, 7, 18, Some(AnalysisStatus::AttentionRequired)) }).unwrap();
    let input = r#"{"session_id":"s-1","model":{"display_name":"Opus 5.5"}}"#;
    let (_, out, _) = run_bar(&["--workspace", ws.path().to_str().unwrap(), "--state-dir",
        state.path().to_str().unwrap()], input.as_bytes(), &[]);
    assert_eq!(out.trim_end(), "rw-brkr · Opus 5.5 · hooks on · última: atenção · inj 7 · não reenviados 18");
}

#[test]
fn without_a_session_id_nothing_is_read() {
    let ws = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();
    publish(state.path(), "default", &root, &Snapshot { workspace_key: workspace_key(&root),
        ..snap(false, 7, 18, None) }).unwrap();
    let (_, out, _) = run_bar(&["--workspace", ws.path().to_str().unwrap(), "--state-dir",
        state.path().to_str().unwrap()], br#"{"model":{"display_name":"Opus"}}"#, &[]);
    assert!(out.contains("hooks sem dados"), "{out}");
}

#[test]
fn bad_stdin_degrades_and_still_exits_zero() {
    let state = tempfile::tempdir().unwrap();
    let sd = state.path().to_str().unwrap();
    let huge = vec![b' '; 256 * 1024 + 1];
    for stdin in [&b""[..], b"{", b"\xff\xfe", &huge[..]] {
        let (code, out, err) = run_bar(&["--state-dir", sd], stdin, &[]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, "rw-brkr · hooks sem dados\n");
        assert!(err.len() < 200, "no long logs: {err}");
    }
}

#[test]
fn width_comes_from_the_flag_then_columns_then_100() {
    let input = br#"{"model":{"display_name":"Sonnet"},"context_window":{"used_percentage":32}}"#;
    let (_, by_flag, _) = run_bar(&["--width", "20"], input, &[("COLUMNS", "200")]);
    assert_eq!(by_flag.trim_end(), "rw-brkr · ctx 32%");
    let (_, by_env, _) = run_bar(&[], input, &[("COLUMNS", "20")]);
    assert_eq!(by_env.trim_end(), "rw-brkr · ctx 32%");
}

#[test]
fn the_workspace_order_is_flag_project_dir_current_dir_cwd() {
    use ripwire_broker::statusline::resolve_root;
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let input = parse_input(&format!(
        r#"{{"workspace":{{"project_dir":"{}","current_dir":"{}"}},"cwd":"{}"}}"#,
        a.path().display(), b.path().display(), b.path().display()));
    assert_eq!(resolve_root(Some(b.path()), &input), Some(b.path().canonicalize().unwrap()));
    assert_eq!(resolve_root(None, &input), Some(a.path().canonicalize().unwrap()));
    assert_eq!(resolve_root(Some(Path::new("/does/not/exist")), &input), None, "no silent fallback");
}

#[test]
fn the_status_line_never_starts_ripwire() {
    let bin = tempfile::tempdir().unwrap();
    let marker = bin.path().join("ran");
    let fake = bin.path().join("ripwire");
    std::fs::write(&fake, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let ws = tempfile::tempdir().unwrap();
    let path = format!("{}:/usr/bin:/bin", bin.path().display());
    let (code, _, _) = run_bar(&["--workspace", ws.path().to_str().unwrap()],
        br#"{"session_id":"s"}"#, &[("PATH", &path), ("HOME", ws.path().to_str().unwrap())]);
    assert_eq!(code, 0);
    assert!(!marker.exists(), "statusline must not start ripwire");
    assert_eq!(std::fs::read_dir(ws.path()).unwrap().count(), 0, "and must not create files");
}
```

- [ ] **Passo 2: vermelho.** `cargo test --test statusline` → falham (o braço provisório não
  imprime nada).

- [ ] **Passo 3: implementar.** Em `src/statusline.rs`:

```rust
pub const MAX_STDIN_BYTES: u64 = 256 * 1024;

/// `--workspace`, else the host's project dir, current dir, cwd (spec §6.1). Canonical; a root that
/// does not resolve gives `None` and never falls through to the next candidate.
pub fn resolve_root(flag: Option<&std::path::Path>, input: &HostInput) -> Option<PathBuf> {
    let candidate = flag.map(PathBuf::from)
        .or_else(|| input.project_dir.clone())
        .or_else(|| input.current_dir.clone())
        .or_else(|| input.cwd.clone())?;
    candidate.canonicalize().ok()
}
```

  Em `src/main.rs`, o braço de `Command::Statusline`, **antes** de qualquer outro trabalho, no lugar
  do provisório:

```rust
Ok(Command::Statusline(a)) => {
    // Status line mode (PRD §24): local reads only, always exit 0, one line.
    use ripwire_broker::statusline::{self, MAX_STDIN_BYTES, Options};
    use ripwire_broker::statusline_state::{self as projection, HOST, Read};
    let mut raw = Vec::new();
    let _ = std::io::stdin().take(MAX_STDIN_BYTES + 1).read_to_end(&mut raw);
    let text = match raw.len() as u64 > MAX_STDIN_BYTES {
        true => String::new(),
        false => String::from_utf8(raw).unwrap_or_default(),
    };
    let input = statusline::parse_input(&text);
    let snapshot = match (&input.session_id, input.agent) {
        (Some(session), false) => {
            let root = statusline::resolve_root(a.workspace.as_deref(), &input);
            let dir = a.state_dir.clone().or_else(StateStore::default_dir);
            match (root, dir) {
                (Some(root), Some(dir)) => match projection::read(&dir, HOST, session, &root) {
                    Read::Valid(s) => Some(s),
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    };
    let width = a.width
        .or_else(|| std::env::var("COLUMNS").ok().and_then(|c| c.parse().ok()))
        .unwrap_or(100);
    let options = Options { detail: a.detail, width, color: a.color == cli::Color::Always };
    println!("{}", statusline::render(&input, snapshot.as_ref(), &options, hook::now()));
    return ExitCode::SUCCESS;
}
```

  `use std::io::Read;` já está em `main.rs`; conferir que `take` e `read_to_end` resolvem.

- [ ] **Passo 4: verde.** `cargo test --test statusline` → passa. Rodar também
  `cargo test --test mcp_surface` (stdout do `serve` continua só MCP).

- [ ] **Passo 5: mutação.** Remover o `.take(MAX_STDIN_BYTES + 1)` (ler tudo) e trocar
  `(Some(session), false)` por `(Some(session), _)`: a primeira derruba
  `bad_stdin_degrades_and_still_exits_zero`; a segunda, o teste de agente **só se** houver um teste
  binário de agente, então **acrescentar** em `it_reads_the_projection_of_this_session_and_workspace`
  uma segunda chamada com `"agent":{"name":"x"}` que espera `rw-brkr · Opus 5.5 · agente`. Desfazer
  e `touch`.

- [ ] **Passo 6: commit.**
  ```bash
  git add src/main.rs src/statusline.rs tests/statusline.rs
  git commit -m "statusline: the binary command, local reads only"
  ```

---

### Tarefa 6: os hooks guardam a última análise e a última entrega

**Arquivos:**
- Modificar: `src/statusline_state.rs` (`Summary`, `bind`, `project`), `src/hook.rs`
  (`SessionState.statusline`, `record`, `respond`, `handle`)
- Teste: `tests/hooks.rs`

**Interfaces:**
- Consome: `Analysis`, `AnalysisStatus`, `Delivery`, `Snapshot`, `VisibleStats`, `SCHEMA_VERSION`, `HOST`.
- Produz:
  ```rust
  // statusline_state.rs
  #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
  pub struct Summary {
      pub workspace_key: String,
      /// `SessionTally` when this workspace was bound: the bar counts from here (spec §6.3).
      pub baseline: crate::hook::SessionTally,
      pub last_analysis: Option<Analysis>,
      pub last_delivery: Option<Delivery>,
  }
  pub fn bind(state: &mut crate::hook::SessionState, workspace_key: &str);
  pub fn project(state: &crate::hook::SessionState, now: u64) -> Option<Snapshot>;
  // hook.rs: SessionState { ..., #[serde(default, skip_serializing_if = "Option::is_none")] pub statusline: Option<Summary> }
  ```
  `SessionTally` precisa de `Default` (já tem) e continua a única fonte dos contadores.

- [ ] **Passo 1: testes vermelhos.** Em `tests/hooks.rs`, reaproveitando `hook_broker`, `event`,
  `post_tool_use`, `stop`, `finish_fake` e `edit_fake` que já existem:

```rust
use ripwire_broker::statusline_state::{self as projection, AnalysisStatus};

fn bound() -> SessionState {
    let mut s = SessionState::default();
    projection::bind(&mut s, "k");
    s
}

fn last(state: &SessionState) -> Option<AnalysisStatus> {
    state.statusline.as_ref()?.last_analysis.as_ref().map(|a| a.status)
}

#[tokio::test]
async fn an_injection_records_the_analysis_and_the_delivery() {
    let (b, _, ws) = hook_broker(FakeUpstream::new().answer("explore", "explore_export_auth")).await;
    let mut state = bound();
    hook::handle(Host::ClaudeCode, Event::UserPromptSubmit,
        &event("claude_code_user_prompt_submit", ws.path()), &b, &mut state, &Policy::default()).await;
    let summary = state.statusline.as_ref().unwrap();
    assert_eq!(summary.last_analysis.as_ref().unwrap().event, "UserPromptSubmit");
    assert!(summary.last_delivery.as_ref().unwrap().estimated_tokens > 0);
    let snap = projection::project(&state, 5).unwrap();
    assert_eq!(snap.stats.injections, state.stats.injections);
}

#[tokio::test]
async fn a_silent_ready_stop_replaces_an_earlier_attention() {
    // First Stop: attention (the gate blocks). Second: ready, which the hook answers with silence.
    let (b, _, ws) = hook_broker(finish_fake()).await;
    let mut state = bound();
    let input = event("claude_code_stop", ws.path());
    hook::handle(Host::ClaudeCode, Event::Stop, &input, &b, &mut state,
        &Policy { gate: true, ..Policy::default() }).await;
    assert_eq!(last(&state), Some(AnalysisStatus::AttentionRequired));
    let delivered_at = state.statusline.as_ref().unwrap().last_delivery.clone();

    // The same fake that makes the gate `ready` in an_unknown_gate_never_blocks_and_a_ready_gate_stays_silent.
    let (ready, _, _) = hook_broker(FakeUpstream::new()
        .answer("situational_awareness", "situational_awareness_clean")
        .answer("quality_delta", "quality_delta_clean")).await;
    let out = hook::handle(Host::ClaudeCode, Event::Stop, &input, &ready, &mut state, &Policy::default()).await;
    assert!(out.is_none(), "ready stays silent");
    assert_eq!(last(&state), Some(AnalysisStatus::Ready));
    assert_eq!(state.statusline.as_ref().unwrap().last_delivery, delivered_at, "silence delivers nothing");
}

#[tokio::test]
async fn an_event_without_analysis_keeps_the_summary() {
    let (b, fake, ws) = hook_broker(finish_fake()).await;
    let mut state = bound();
    hook::handle(Host::ClaudeCode, Event::Stop, &event("claude_code_stop", ws.path()), &b, &mut state,
        &Policy { gate: true, ..Policy::default() }).await;
    let before = state.statusline.clone();
    // A second prompt without --every-prompt asks nothing upstream.
    let calls = fake.called().len();
    state.prompts_seen = 1;
    hook::handle(Host::ClaudeCode, Event::UserPromptSubmit,
        &event("claude_code_user_prompt_submit", ws.path()), &b, &mut state, &Policy::default()).await;
    assert_eq!(fake.called().len(), calls);
    assert_eq!(state.statusline, before);
}

#[tokio::test]
async fn opt_out_and_opt_in_show_in_the_projection() {
    let (b, _, ws) = hook_broker(FakeUpstream::new().answer("explore", "explore_export_auth")).await;
    let mut state = bound();
    let mut input = event("claude_code_user_prompt_submit", ws.path());
    input["prompt"] = "stop it #ripwire-off".into();
    hook::handle(Host::ClaudeCode, Event::UserPromptSubmit, &input, &b, &mut state, &Policy::default()).await;
    assert!(projection::project(&state, 0).unwrap().opted_out);
    input["prompt"] = "back #ripwire-on".into();
    hook::handle(Host::ClaudeCode, Event::UserPromptSubmit, &input, &b, &mut state,
        &Policy { every_prompt: true, ..Policy::default() }).await;
    assert!(!projection::project(&state, 0).unwrap().opted_out);
}

#[tokio::test]
async fn a_broker_failure_becomes_an_error_analysis_with_its_kind_only() {
    let (b, _, ws) = hook_broker(FakeUpstream::new().fail(
        "explore",
        ripwire_broker::upstream::UpstreamError::Refused("boom /secret/path".into()),
    )).await;
    let mut state = bound();
    hook::handle(Host::ClaudeCode, Event::UserPromptSubmit,
        &event("claude_code_user_prompt_submit", ws.path()), &b, &mut state, &Policy::default()).await;
    let a = state.statusline.as_ref().unwrap().last_analysis.clone().unwrap();
    assert_eq!(a.status, AnalysisStatus::Error);
    let kind = a.error_kind.unwrap();
    assert!(!kind.contains("secret") && !kind.contains(' '), "a kind, not a message: {kind}");
}

#[test]
fn rebinding_starts_a_new_visual_baseline_and_legacy_state_loads() {
    let legacy = r#"{"memory":{"seen":[]},"prompts_seen":3,"opted_out":false,
        "stats":{"started_at":1,"events":40,"injections":9,"delivered":30,"session_hits":12}}"#;
    let mut state: SessionState = serde_json::from_str(legacy).expect("legacy state still loads");
    assert!(state.statusline.is_none());
    projection::bind(&mut state, "k1");
    assert_eq!(projection::project(&state, 0).unwrap().stats.injections, 0, "old totals are not this workspace's");
    state.stats.injections += 2;
    assert_eq!(projection::project(&state, 0).unwrap().stats.injections, 2);
    projection::bind(&mut state, "k1");
    assert_eq!(projection::project(&state, 0).unwrap().stats.injections, 2, "same key: no reset");
    projection::bind(&mut state, "k2");
    let p = projection::project(&state, 0).unwrap();
    assert_eq!((p.stats.injections, p.last_analysis.is_none()), (0, true), "another root: fresh");
}
```

  Os fakes acima são os mesmos de `tests/hooks.rs`: `finish_fake()` (linha 368) produz
  `attention_required`, e `situational_awareness_clean` + `quality_delta_clean` produzem `ready`
  (linha 447). `FakeUpstream::fail(tool, UpstreamError)` está em `tests/common/fake.rs:112`.

- [ ] **Passo 2: vermelho.** `cargo test --test hooks` → não compila (`statusline` em `SessionState`).

- [ ] **Passo 3: implementar.**

  Em `src/statusline_state.rs`:

```rust
use crate::hook::{SessionState, SessionTally};

/// Binds the session to a workspace. A new key (or a state saved before the status line existed)
/// starts a fresh visual baseline and forgets the summaries of the previous root (spec §6.3).
pub fn bind(state: &mut SessionState, workspace_key: &str) {
    if state.statusline.as_ref().is_some_and(|s| s.workspace_key == workspace_key) {
        return;
    }
    state.statusline = Some(Summary {
        workspace_key: workspace_key.into(),
        baseline: state.stats.clone(),
        last_analysis: None,
        last_delivery: None,
    });
}

/// The projection of a bound state: counters since the binding, replaced whole on every write.
pub fn project(state: &SessionState, now: u64) -> Option<Snapshot> {
    let s = state.statusline.as_ref()?;
    let since = |now: u64, then: u64| now.saturating_sub(then);
    let (t, b): (&SessionTally, &SessionTally) = (&state.stats, &s.baseline);
    Some(Snapshot {
        schema_version: SCHEMA_VERSION,
        host: HOST.into(),
        workspace_key: s.workspace_key.clone(),
        updated_at: now,
        opted_out: state.opted_out,
        stats: VisibleStats {
            events: since(t.events, b.events),
            injections: since(t.injections, b.injections),
            delivered: since(t.delivered, b.delivered),
            session_hits: since(t.session_hits, b.session_hits),
        },
        last_analysis: s.last_analysis.clone(),
        last_delivery: s.last_delivery.clone(),
    })
}
```

  Em `src/hook.rs`:
  - campo novo em `SessionState`, com doc comment
    `/// What the status line shows (PRD §24.6.3); absent in states saved before it.`
    e `#[serde(default, skip_serializing_if = "Option::is_none")] pub statusline: Option<crate::statusline_state::Summary>,`
  - um helper:
    ```rust
    /// Records what the last analysis said, when the session is bound to a workspace.
    fn analysed(state: &mut SessionState, event: Event, status: AnalysisStatus, kind: Option<&str>) {
        if let Some(s) = state.statusline.as_mut() {
            s.last_analysis = Some(Analysis {
                at: now(), event: event_name(event).into(), status, error_kind: kind.map(str::to_string),
            });
        }
    }
    fn status_of(s: Status) -> AnalysisStatus {
        match s {
            Status::Ready => AnalysisStatus::Ready,
            Status::AttentionRequired => AnalysisStatus::AttentionRequired,
            Status::Unknown => AnalysisStatus::Unknown,
        }
    }
    ```
  - em `respond`, logo depois de **cada** `let env = broker.context_…(req).await?;` (três lugares:
    prompt, edição e `Stop`), a linha `analysed(state, event, status_of(env.status), None);`. Na
    edição, ela vem **antes** do `if !has_news(...)`: uma análise sem novidade ainda é uma análise.
  - em `record`, depois do `push` no log:
    ```rust
    if let Some(s) = state.statusline.as_mut() {
        s.last_delivery = Some(Delivery { at: now(), estimated_tokens: env.budget.estimated_tokens });
    }
    ```
  - em `handle`, no braço `Err(e) => Some(failure(&e))`, antes:
    `analysed(state, event, AnalysisStatus::Error, Some(e.error));`.

- [ ] **Passo 4: verde.** `cargo test --test hooks` → passa, inclusive os antigos.
  `cargo test --test cli hook_stats` → passa (o `hook-stats` não muda).

- [ ] **Passo 5: mutação.** Mover o `analysed(...)` da edição para depois do `has_news` e apagar a
  linha do `Err`: cada uma tem de derrubar um teste. Se a primeira não derrubar, acrescentar um teste
  de edição sem novidade que espera `last_analysis` atualizado. Desfazer e `touch`.

- [ ] **Passo 6: commit.**
  ```bash
  git add src/statusline_state.rs src/hook.rs tests/hooks.rs
  git commit -m "hooks: keep the last analysis and delivery for the status line"
  ```

---

### Tarefa 7: os hooks publicam a projeção

**Arquivos:**
- Modificar: `src/hook.rs` (`run`)
- Teste: `tests/cli.rs` (pelo binário: `run` já existe como helper)

**Interfaces:**
- Consome: `bind`, `project`, `publish`, `workspace_key`, `read`, `Read`, `HOST`.
- Produz: nenhuma API nova; o arquivo em `<state-dir>/statusline/`.

**Regras (spec §6.4 e D3, D4):**
- Só `Host::ClaudeCode`, só com `session_id` vindo do evento (nunca `"default"`) e só com a raiz
  canonicalizável.
- Ordem: lock → load → `bind` → … → `save` → **se o save deu certo**, `publish`. Erro de publicação
  é ignorado.
- Falha do `local::launch` (fora de opt-out): `analysed(Error, e.error)`, `save`, `publish`, e
  devolve o mesmo `failure(&e)` de hoje. Os contadores não mudam.

- [ ] **Passo 1: testes vermelhos.** Em `tests/cli.rs`:

```rust
use ripwire_broker::statusline_state::{self as projection, AnalysisStatus, HOST, Read};

fn bar_snapshot(state: &Path, session: &str, ws: &Path) -> Read {
    projection::read(state, HOST, session, &ws.canonicalize().unwrap())
}

#[test]
fn a_launch_failure_is_published_as_an_error_and_the_hook_still_answers() {
    let ws = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let args = ["hook", "claude-code", "user-prompt-submit", "--workspace", ws.path().to_str().unwrap(),
        "--state-dir", state.path().to_str().unwrap(), "--ripwire", "/nonexistent/ripwire"];
    let (code, out, _) = run(&args, &prompt_event(ws.path(), "s-1", "hello"));
    assert_eq!(code, 0);
    assert!(out.contains("no context"), "same answer as before: {out}");
    let Read::Valid(s) = bar_snapshot(state.path(), "s-1", ws.path()) else { panic!("published") };
    let a = s.last_analysis.unwrap();
    assert_eq!(a.status, AnalysisStatus::Error);
    assert_eq!(s.stats.events, 0, "D4: counters unchanged by a launch failure");
}

#[test]
fn no_session_id_or_codex_publishes_nothing() {
    let ws = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let base = |host: &'static str| vec!["hook", host, "user-prompt-submit", "--workspace",
        ws.path().to_str().unwrap(), "--state-dir", state.path().to_str().unwrap(), "--ripwire", "/nonexistent/ripwire"];
    let no_id = json!({"cwd": ws.path(), "hook_event_name": "UserPromptSubmit", "prompt": "x"}).to_string();
    run(&base("claude-code"), &no_id);
    run(&base("codex"), &prompt_event(ws.path(), "s-1", "x"));
    assert!(!state.path().join("statusline").exists()
        || std::fs::read_dir(state.path().join("statusline")).unwrap().count() == 0);
}

#[test]
fn a_failed_publication_changes_nothing_and_the_next_event_repairs_it() {
    let ws = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    std::fs::write(state.path().join("statusline"), "not a dir").unwrap();
    let args = ["hook", "claude-code", "user-prompt-submit", "--workspace", ws.path().to_str().unwrap(),
        "--state-dir", state.path().to_str().unwrap(), "--ripwire", "/nonexistent/ripwire"];
    let (code, blocked, _) = run(&args, &prompt_event(ws.path(), "s-1", "hello"));
    std::fs::remove_file(state.path().join("statusline")).unwrap();
    let (_, again, _) = run(&args, &prompt_event(ws.path(), "s-1", "hello"));
    assert_eq!(code, 0);
    assert_eq!(blocked, again, "the hook's answer does not depend on the projection");
    assert!(matches!(bar_snapshot(state.path(), "s-1", ws.path()), Read::Valid(_)), "republished");
}
```

  E um teste com o ripwire real, no estilo de
  `the_hook_command_injects_context_from_the_real_ripwire_once_per_session` (com `require_ripwire!()`):
  um prompt e depois um `#ripwire-off` na mesma sessão; o snapshot mostra `injections == 1`, depois
  `opted_out == true`, e os totais batem com `StateStore::new(dir).load("s-1").stats` menos a baseline
  (zero numa sessão nova). Ele cobre o critério 7 da spec ("totais reproduzem `SessionTally`").

- [ ] **Passo 2: vermelho.** `cargo test --test cli launch_failure` → falha (nada publicado).

- [ ] **Passo 3: implementar em `hook::run`.**

  - Depois de calcular `workspace` e antes do lock, guardar se o `session_id` veio do evento:
    ```rust
    let real_session = input.get("session_id").and_then(Value::as_str).is_some();
    let root = workspace.canonicalize().ok();
    let publishes = real_session && args.host == Host::ClaudeCode && root.is_some();
    ```
  - Depois do `store.load`: `if let Some(r) = &root { statusline_state::bind(&mut state, &statusline_state::workspace_key(r)); }`
  - Um fecho local para salvar e publicar, usado nos dois finais:
    ```rust
    let finish = |state: &SessionState| {
        // Losing the state only costs a repeated injection; never fail the host for it.
        if store.save(&session_id, state).is_ok() && publishes {
            if let (Some(r), Some(snap)) = (&root, statusline_state::project(state, now())) {
                let _ = statusline_state::publish(store.dir(), &session_id, r, &snap);
            }
        }
    };
    ```
    `StateStore` precisa de `pub fn dir(&self) -> &Path { &self.dir }`.
  - No `Err` do `launch`:
    ```rust
    Err(_) if state.opted_out => return None,
    Err(e) => {
        analysed(&mut state, args.event, AnalysisStatus::Error, Some(e.error));
        finish(&state);
        return Some(failure(&e));
    }
    ```
  - No fim, `let _ = store.save(...)` vira `finish(&state);`.

  Se o fecho esbarrar no borrow checker (ele empresta `store`), transformá-lo numa `fn finish(store:
  &StateStore, session_id: &str, root: Option<&Path>, publishes: bool, state: &SessionState)`.

- [ ] **Passo 4: verde.** `cargo test --test cli` e `cargo test --test hooks` → passam.

- [ ] **Passo 5: mutação.** Publicar **antes** do `save`, e remover o `real_session` da condição: a
  segunda tem de derrubar `no_session_id_or_codex_publishes_nothing`. A primeira não é visível nos
  testes sem injetar falha no `save`; aceitar e registrar no D-123 que a ordem é garantida por
  leitura de código, não por teste. Desfazer e `touch`.

- [ ] **Passo 6: commit.**
  ```bash
  git add src/hook.rs src/state.rs tests/cli.rs
  git commit -m "hooks: publish the status line projection after saving"
  ```

---

### Tarefa 8: `install --statusline`

**Arquivos:**
- Modificar: `src/install.rs`
- Teste: `tests/cli.rs`

**Interfaces:**
- Consome: `InstallArgs.statusline` (T1), `quote`, `change`, `merge_hooks`, `utf8` (existentes).
- Produz (privadas ao módulo, testadas pelo binário):
  ```rust
  fn statusline_command(binary: &Path, workspace: &Path) -> String;
  fn shell_words(cmd: &str) -> Vec<String>;
  enum Bar { Absent, Ours, Foreign }
  fn bar(settings: &Value) -> Bar;
  fn merge_statusline(settings: Value, command: &str) -> Value; // só Absent/Ours mudam
  fn user_settings() -> Option<PathBuf>; // $CLAUDE_CONFIG_DIR/settings.json, senão ~/.claude/settings.json
  ```

**Regras:** spec §7 e D5. Uma alteração só para `.claude/settings.json`, com hooks e barra
mesclados juntos em memória.

- [ ] **Passo 1: testes vermelhos.** Em `tests/cli.rs`. **Todo teste de instalação com
  `--statusline` define `CLAUDE_CONFIG_DIR`** para um tempdir, senão leria o `~/.claude` real de quem
  roda. Um helper:

```rust
fn run_env(args: &[&str], env: &[(&str, &Path)]) -> (i32, String, String) {
    let mut cmd = Proc::new(env!("CARGO_BIN_EXE_ripwire-broker"));
    cmd.args(args);
    for (k, v) in env { cmd.env(k, v); }
    let out = cmd.output().unwrap();
    (out.status.code().unwrap_or(-1),
     String::from_utf8_lossy(&out.stdout).into_owned(),
     String::from_utf8_lossy(&out.stderr).into_owned())
}

fn install_bar(root: &Path, user: &Path, extra: &[&str]) -> (i32, String, String) {
    let mut args = vec!["install", "claude-code", "--workspace", root.to_str().unwrap(), "--statusline"];
    args.extend_from_slice(extra);
    run_env(&args, &[("CLAUDE_CONFIG_DIR", user)])
}

#[test]
fn statusline_install_is_a_dry_run_then_one_merged_change_and_idempotent() {
    let ws = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();
    let (code, out, err) = install_bar(&root, user.path(), &["--hooks"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("dry run") && out.contains("statusLine"), "{out}");
    assert!(!root.join(".claude").exists(), "nothing written");

    let (code, first, err) = install_bar(&root, user.path(), &["--hooks", "--write"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(first.matches("settings.json").count(), 1, "one change for settings: {first}");
    let s = read_json(&root.join(".claude/settings.json"));
    let cmd = s["statusLine"]["command"].as_str().unwrap();
    assert!(cmd.ends_with(&format!(" statusline --workspace '{}' --color never", root.display())), "{cmd}");
    assert_eq!(s["statusLine"]["type"], "command");
    assert!(s.get("statusLine").unwrap().get("refreshInterval").is_none());
    assert_eq!(commands(&s, "Stop").len(), 1, "hooks are in the same file");

    let (_, second, _) = install_bar(&root, user.path(), &["--hooks", "--write"]);
    assert!(second.contains("unchanged") && second.contains("settings.json"), "{second}");
}

#[test]
fn a_foreign_bar_is_kept_with_a_note_and_ours_is_updated_keeping_options() {
    let ws = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();
    let settings = root.join(".claude/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let graft = r#"{"statusLine":{"type":"command","command":"node \"${CLAUDE_PROJECT_DIR:-.}/.claude/helpers/graft-statusline.cjs\""}}"#;
    std::fs::write(&settings, graft).unwrap();
    let (code, out, _) = install_bar(&root, user.path(), &["--write"]);
    assert_eq!(code, 0);
    assert!(out.contains("statusLine") && out.contains("keep"), "a note: {out}");
    assert!(read_json(&settings)["statusLine"]["command"].as_str().unwrap().contains("graft"));

    let old = r#"{"statusLine":{"type":"command","command":"'/old/bin/ripwire-broker' statusline --workspace '/old' --color never","padding":2,"refreshInterval":5,"x-extra":true}}"#;
    std::fs::write(&settings, old).unwrap();
    install_bar(&root, user.path(), &["--write"]);
    let s = read_json(&settings);
    assert!(s["statusLine"]["command"].as_str().unwrap().contains(&root.display().to_string()));
    assert_eq!((s["statusLine"]["padding"].clone(), s["statusLine"]["refreshInterval"].clone(), s["statusLine"]["x-extra"].clone()),
               (json!(2), json!(5), json!(true)));
}

#[test]
fn ownership_is_structural_not_a_substring() {
    let ws = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();
    let settings = root.join(".claude/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    for foreign in [
        "echo ripwire-broker statusline",
        "'/x/not-ripwire-broker' statusline",
        "'/x/ripwire-broker' hook claude-code stop",
    ] {
        std::fs::write(&settings, json!({"statusLine": {"type": "command", "command": foreign}}).to_string()).unwrap();
        install_bar(&root, user.path(), &["--write"]);
        assert_eq!(read_json(&settings)["statusLine"]["command"], foreign, "kept: {foreign}");
    }
}

#[test]
fn an_inherited_user_bar_is_not_shadowed_and_a_local_one_is_reported() {
    let ws = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();
    std::fs::write(user.path().join("settings.json"),
        r#"{"statusLine":{"type":"command","command":"my-bar"}}"#).unwrap();
    let (code, out, _) = install_bar(&root, user.path(), &["--write"]);
    assert_eq!(code, 0);
    let user_file = user.path().join("settings.json").display().to_string();
    assert!(out.contains(&user_file) && out.contains("keeping it"), "a note naming the inherited bar's file: {out}");
    assert!(!root.join(".claude/settings.json").exists()
        || read_json(&root.join(".claude/settings.json")).get("statusLine").is_none(), "not shadowed");

    std::fs::remove_file(user.path().join("settings.json")).unwrap();
    std::fs::create_dir_all(root.join(".claude")).unwrap();
    std::fs::write(root.join(".claude/settings.local.json"),
        r#"{"statusLine":{"type":"command","command":"local-bar"}}"#).unwrap();
    let (_, out, _) = install_bar(&root, user.path(), &["--write"]);
    assert!(read_json(&root.join(".claude/settings.json"))["statusLine"]["command"].as_str().unwrap().contains("statusline"));
    assert!(out.contains("settings.local.json"), "the local bar wins, and the note says so: {out}");
}

#[test]
fn statusline_alone_says_counters_need_hooks_and_paths_are_quoted() {
    let ws = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let odd = ws.path().join("a b'c$(x)");
    std::fs::create_dir(&odd).unwrap();
    let root = odd.canonicalize().unwrap();
    let (code, out, err) = install_bar(&root, user.path(), &["--write"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("--hooks"), "counters need hooks: {out}");
    let cmd = read_json(&root.join(".claude/settings.json"))["statusLine"]["command"].as_str().unwrap().to_string();
    assert!(cmd.contains("'\\''"), "single quotes escaped: {cmd}");
    let (_, re, _) = install_bar(&root, user.path(), &["--write"]);
    assert!(re.contains("unchanged"), "its own quoted command is recognized as ours: {re}");
}

#[test]
fn invalid_settings_json_blocks_the_whole_file() {
    let ws = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join(".claude")).unwrap();
    std::fs::write(root.join(".claude/settings.json"), "{broken").unwrap();
    let (code, _, err) = install_bar(&root, user.path(), &["--hooks", "--write"]);
    assert_ne!(code, 0);
    assert!(err.contains("not valid JSON"), "{err}");
    assert_eq!(std::fs::read_to_string(root.join(".claude/settings.json")).unwrap(), "{broken");
}

#[test]
fn reinstalling_without_statusline_keeps_the_bar() {
    let ws = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();
    install_bar(&root, user.path(), &["--hooks", "--write"]);
    run_env(&["install", "claude-code", "--workspace", root.to_str().unwrap(), "--hooks", "--write"],
        &[("CLAUDE_CONFIG_DIR", user.path())]);
    assert!(read_json(&root.join(".claude/settings.json")).get("statusLine").is_some());
}
```

  `read_json` e `commands` já existem em `tests/cli.rs`.

- [ ] **Passo 2: vermelho.** `cargo test --test cli statusline` → falha.

- [ ] **Passo 3: implementar em `src/install.rs`.**

```rust
fn statusline_command(binary: &Path, workspace: &Path) -> String {
    format!("{} statusline --workspace {} --color never", quote(binary), quote(workspace))
}

/// Just enough of POSIX shell words to read back a command `install` wrote: single quotes (with
/// `'\''`), double quotes, backslashes and whitespace. Used only to recognize ownership.
fn shell_words(cmd: &str) -> Vec<String> {
    let (mut words, mut cur, mut any) = (vec![], String::new(), false);
    let mut it = cmd.chars();
    while let Some(c) = it.next() {
        match c {
            '\'' => { any = true; for c in it.by_ref() { if c == '\'' { break; } cur.push(c); } }
            '"' => { any = true; while let Some(c) = it.next() {
                match c { '"' => break, '\\' => { if let Some(n) = it.next() { cur.push(n) } } _ => cur.push(c) } } }
            '\\' => { any = true; if let Some(n) = it.next() { cur.push(n) } }
            c if c.is_whitespace() => { if any { words.push(std::mem::take(&mut cur)); any = false; } }
            c => { any = true; cur.push(c) }
        }
    }
    if any { words.push(cur); }
    words
}

enum Bar { Absent, Ours, Foreign }

/// Ours: the program is a `ripwire-broker` executable and its first argument is `statusline`.
fn bar(settings: &Value) -> Bar {
    let Some(line) = settings.get("statusLine") else { return Bar::Absent };
    let words = line.get("command").and_then(Value::as_str).map(shell_words).unwrap_or_default();
    let ours = words.first().and_then(|p| Path::new(p).file_name()).is_some_and(|n| n == "ripwire-broker")
        && words.get(1).is_some_and(|w| w == "statusline");
    if ours { Bar::Ours } else { Bar::Foreign }
}

fn merge_statusline(mut settings: Value, command: &str) -> Value {
    if !settings.is_object() { settings = json!({}); }
    match bar(&settings) {
        Bar::Foreign => {}
        Bar::Absent => { settings["statusLine"] = json!({"type": "command", "command": command}); }
        Bar::Ours => {
            settings["statusLine"]["type"] = "command".into();
            settings["statusLine"]["command"] = command.into();
        }
    }
    settings
}

fn user_settings() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude")))
        .map(|d| d.join("settings.json"))
}
```

  Em `plan()`, no braço `Host::ClaudeCode`, trocar o `if args.hooks { ... }` por:

```rust
let settings_path = workspace.join(".claude/settings.json");
let command = statusline_command(binary, &workspace);
let manual = format!("statusLine for {}:\n{}", settings_path.display(),
    pretty(&json!({"statusLine": {"type": "command", "command": command}})));
let mut bar_wanted = args.statusline;
if args.statusline {
    // D5: an inherited user bar would be shadowed by ours; an unreadable one leaves the
    // effective bar unknown. Either way, nothing is written and the snippet is shown.
    match user_settings().map(|p| (parse(&read(&p)), p)) {
        Some((Err(e), p)) => { bar_wanted = false;
            plan.notes.push(format!("{}: {e}; add the bar by hand if it is free:\n{manual}", p.display())); }
        Some((Ok(v), p)) if matches!(bar(&v), Bar::Foreign) => { bar_wanted = false;
            plan.notes.push(format!("{} has its own statusLine; keeping it. To use the broker's:\n{manual}", p.display())); }
        _ => {}
    }
    let local = workspace.join(".claude/settings.local.json");
    if let Ok(v) = parse(&read(&local)) && matches!(bar(&v), Bar::Foreign) {
        plan.notes.push(format!("{} has its own statusLine, and it wins over the project's.", local.display()));
    }
    if !args.hooks {
        plan.notes.push("statusLine without --hooks: the bar shows `hooks sem dados` until hooks are installed.".into());
    }
}
if args.hooks || bar_wanted {
    let mut foreign = false;
    plan.changes.push(change(settings_path.clone(), |mut v| {
        if args.hooks { v = merge_hooks(v, Host::ClaudeCode, binary, Some(&workspace)); }
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
```

  `src/main.rs` não muda: o braço `Install` já imprime `plan.notes` depois das alterações, no dry
  run e com `--write` (`src/main.rs:203`), que é o que a spec pede ("inclusive com `--write`").

- [ ] **Passo 4: verde.** `cargo test --test cli` → passa, inclusive os testes de instalação antigos.

- [ ] **Passo 5: mutação.** Trocar `file_name() == "ripwire-broker"` por `contains("ripwire-broker")`
  e remover o `bar_wanted = false` do braço `Foreign` do usuário: cada uma tem de cair
  (`ownership_is_structural_not_a_substring`, `an_inherited_user_bar_is_not_shadowed...`). Desfazer
  e `touch`.

- [ ] **Passo 6: commit.**
  ```bash
  git add src/install.rs tests/cli.rs
  git commit -m "install: --statusline merged into the same settings change"
  ```

---

### Tarefa 9: docs, medição e validação manual

**Arquivos:**
- Modificar: `README.md`, `spec/changelog.md` (D-123), `handoff.md`, `spec/ripwire-broker-mcp.md` (estado do §24 e do §19), `integrations/` se houver instruções de instalação para Claude Code

- [ ] **Passo 1: README.** Seção `## Status line` depois da de hooks: o que cada segmento significa
  (com os quatro exemplos da spec), `--detail`, `--width`, `--color`, `install … --statusline`, a
  relação com os hooks (`hooks sem dados` não prova desinstalação), privacidade (só contagens; o
  stdin do host não é guardado), e a **migração**: os contadores da barra começam na primeira
  projeção vinculada ao workspace, então uma sessão anterior à barra mostra zero, enquanto o
  `hook-stats` continua contando tudo. Acrescentar `statusline` à tabela de comandos do README.

- [ ] **Passo 2: medição release** (spec §9). Com o build release e uma projeção publicada:
  ```bash
  cargo build --release
  python3 - <<'EOF'
  import subprocess, time, json, statistics, tempfile, os
  ws = tempfile.mkdtemp(); sd = tempfile.mkdtemp()
  inp = json.dumps({"session_id":"m","workspace":{"project_dir":ws},"model":{"display_name":"Opus","id":"claude-opus-5-5"},
                    "effort":{"level":"high"},"context_window":{"used_percentage":42}}).encode()
  cmd = ["target/release/ripwire-broker","statusline","--workspace",ws,"--state-dir",sd,"--detail"]
  t = []
  for _ in range(300):
      s = time.perf_counter(); subprocess.run(cmd, input=inp, capture_output=True); t.append((time.perf_counter()-s)*1000)
  t.sort(); print(f"p50 {t[149]:.1f} ms  p95 {t[284]:.1f} ms  max {t[-1]:.1f} ms")
  EOF
  ```
  Repetir com uma projeção de 16 KiB no lugar (gerar com `publish` num teste `#[ignore]`, ou
  escrevê-la à mão com `stats` grandes e `event` longo). Meta: p95 < 100 ms ponta a ponta. Registrar
  os números, a máquina e o sistema no D-123. Se o p95 passar da meta, parar e reportar antes de
  otimizar.

- [ ] **Passo 3: validação manual numa sessão real** (spec §10, últimos parágrafos). Numa pasta de
  teste: `ripwire-broker install claude-code --workspace DIR --hooks --statusline --write`; abrir o
  Claude Code ali; mandar um prompt, editar um arquivo, encerrar o turno, digitar `#ripwire-off`
  e depois `#ripwire-on`. A cada passo, comparar a barra com
  `ripwire-broker hook-log --session ID` e com o snapshot. Registrar a versão do Claude Code. **É o
  mantenedor quem roda este passo**; o agente prepara a pasta e o roteiro.

  Aproveitar e **gravar um payload real** de `statusLine` como fixture
  (`tests/fixtures/statusline/claude_code.json`, com o placeholder `__WORKSPACE__` no lugar do caminho,
  como as fixtures de hooks), trocando os JSONs sintéticos dos testes por ele onde couber. Conferir
  nele os nomes `effort.level`, `workspace.project_dir` e `agent`; se algum divergir da spec, parar e
  reportar.

- [ ] **Passo 4: D-123 em `spec/changelog.md`.** Linha no índice (bloco recente em ordem
  decrescente) e seção no fim:
  - o que entrou, comando por comando;
  - as decisões D1 a D6 deste plano, cada uma com o motivo;
  - o defeito existente do `#ripwire-on` sob falha de launch, registrado e não corrigido;
  - a ordem save → publish garantida por leitura, não por teste (Tarefa 7, passo 5);
  - as mutações de cada tarefa;
  - a medição e a validação manual;
  - a contagem de testes antes e depois, nas duas features.

- [ ] **Passo 5: handoff e PRD.** Em `handoff.md`, acrescentar a barra em "Onde está o quê"
  (`src/statusline.rs`, `src/statusline_state.rs`) e o defeito em "Pendências". No PRD, o
  `**Estado:**` do §24 e o da entrada "Barra de status" do §19 passam a `implementada (D-123)`, e o
  §24.13 registra a decisão tomada em cada D1 a D6.

- [ ] **Passo 6: verificação completa** (sem pipe):
  ```bash
  if cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings \
     && cargo clippy --all-targets --locked --features online -- -D warnings \
     && cargo test --all-targets --locked && cargo test --all-targets --locked --features online
  then echo OK; else echo FALHOU; fi
  ```
  E o gate do CA-10: `cargo tree --locked -e normal | grep -Ei 'reqwest|secrecy|rustls|hyper'` tem
  de sair vazio.

- [ ] **Passo 7: commit.**
  ```bash
  git add README.md spec/changelog.md handoff.md spec/ripwire-broker-mcp.md spec/plan/status-bar-plan.md integrations
  git commit -m "Status line: docs, measurement and D-123"
  ```

---

## Cobertura da spec

| spec | tarefa |
| --- | --- |
| §2 contrato, modos separados | 5 (`statusline` não toca MCP), 1 |
| §3.1 lacunas: `load` que esconde ausência | 4 (`Read` distingue os quatro casos) |
| §3.1 identidade de workspace | 4, 6 (`workspace_key`, `bind`) |
| §3.1 `Stop` silencioso e `log.last()` | 6 (`last_analysis` separado do log) |
| §3.1 falha de `local::launch` | 7 |
| §3.1 hooks offline, MCP separado | nada é lido do MCP (5); README (9) |
| §5.1 conteúdo, modelo, effort | 2 |
| §5.2 detalhado | 2 |
| §5.3 largura e cores | 3 |
| §6.1 CLI, ordem do workspace, sem `default` | 1, 5 |
| §6.2 módulos | mapa de arquivos |
| §6.3 snapshot, baseline, chave | 4, 6 |
| §6.4 escrita, leitura, limites, permissões | 4, 5, 7 |
| §7 instalação e coexistência | 8 |
| §8 agente | 2, 5 |
| §9 desempenho, privacidade, legado | 9 (medição), 4 e 6 (privacidade, legado) |
| §10.1 critérios 1 a 12 | 1: T5 · 2: T2, T4, T5 · 3: T2 · 4: T4, T5 · 5: T6 · 6: T6, T7 · 7: T4, T7 · 8: T3 · 9: T8 · 10: T8 · 11: T9 · 12: T2 |
