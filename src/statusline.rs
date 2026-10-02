//! The `statusline` renderer (PRD §24.5): the host's JSON and the hooks' projection
//! become one line. Pure: no clock, no disk, no environment; the caller passes all of them.

use crate::statusline_state::{AnalysisStatus, Snapshot};
use serde_json::Value;
use std::path::PathBuf;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const PREFIX: &str = "rw-brkr";
pub const SEPARATOR: &str = " · ";
/// After this many seconds, `--detail` says the data is old (§5.2).
pub const STALE_SECS: u64 = 300;
const MAX_MODEL_CHARS: usize = 32;
const MAX_AGENT_COLS: usize = 24;
/// Bounds the work on a hostile name before it is measured.
const MAX_AGENT_CHARS: usize = 64;
pub const MAX_STDIN_BYTES: u64 = 256 * 1024;

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
    /// The main session runs with `--agent` or agent settings (D6 revised, D-123).
    pub agent: bool,
    pub agent_name: Option<String>,
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
        agent_name: text(&v, &["agent", "name"]),
    }
}

/// `--workspace`, else the host's project dir, current dir, cwd (spec §6.1). Canonical; a root that
/// does not resolve gives `None` and never falls through to the next candidate.
pub fn resolve_root(flag: Option<&std::path::Path>, input: &HostInput) -> Option<PathBuf> {
    let candidate = flag
        .map(PathBuf::from)
        .or_else(|| input.project_dir.clone())
        .or_else(|| input.current_dir.clone())
        .or_else(|| input.cwd.clone())?;
    candidate.canonicalize().ok()
}

/// Format characters that draw nothing or reorder what follows: the soft hyphen, the combining
/// grapheme joiner, the Arabic letter mark, the Hangul and Mongolian fillers, the zero-width and
/// directional marks, the line and paragraph separators, the bidi embeddings and isolates, the
/// invisible operators, the variation selectors, the interlinear annotation marks, the BOM and the
/// tag characters. Not controls to Rust, but just as able to disguise what the bar shows.
fn is_invisible_format(c: char) -> bool {
    matches!(c,
        '\u{00AD}'
        | '\u{034F}'
        | '\u{061C}'
        | '\u{115F}'..='\u{1160}'
        | '\u{180E}'
        | '\u{200B}'..='\u{200F}'
        | '\u{2028}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{2069}'
        | '\u{3164}'
        | '\u{FE00}'..='\u{FE0F}'
        | '\u{FEFF}'
        | '\u{FFA0}'
        | '\u{FFF9}'..='\u{FFFB}'
        | '\u{E0000}'..='\u{E007F}')
}

/// Drops control characters (ESC, newline, BEL...) and invisible format characters, so external
/// text cannot steer the terminal or hide in the bar.
pub fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() && !is_invisible_format(*c))
        .collect()
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
    let rest = id
        .strip_prefix("claude-")?
        .strip_prefix(family.as_str())?
        .strip_prefix('-')?;
    let parts: Vec<&str> = rest
        .split('-')
        .take_while(|p| (1..=2).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
        .collect();
    (!parts.is_empty()).then(|| parts.join("."))
}

pub fn model_label(name: Option<&str>, id: Option<&str>, effort: Option<&str>) -> Option<String> {
    // Trimmed last: dropping a character can leave a space at the edge.
    let name = sanitize(name?).trim().to_string();
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
        let cut: String = label.chars().take(MAX_MODEL_CHARS - 1).collect();
        label = format!("{}…", cut.trim_end());
    }
    if let Some(e) = effort.and_then(effort_label) {
        label = format!("{label} {e}");
    }
    Some(label)
}

/// How a segment is drawn, and how early it goes when the line is too wide (Task 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Plain,
    Gray,
    White,
    Yellow,
    Red,
    LightBlue,
}

/// Dropped first to last: Detail, Counter, Model, Soft. Essential is never dropped (D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Keep {
    Detail,
    Counter,
    Model,
    Soft,
    Essential,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub text: String,
    pub keep: Keep,
    pub style: Style,
    /// The context-window reading: the last thing `fit` gives up before the prefix.
    pub ctx: bool,
}

fn seg(text: impl Into<String>, keep: Keep, style: Style) -> Segment {
    Segment {
        text: text.into(),
        keep,
        style,
        ctx: false,
    }
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
pub fn segments(
    input: &HostInput,
    snapshot: Option<&Snapshot>,
    detail: bool,
    now: u64,
) -> Vec<Segment> {
    let mut out = vec![seg(PREFIX, Keep::Essential, Style::Plain)];
    if let Some(m) = model_label(
        input.model_name.as_deref(),
        input.model_id.as_deref(),
        input.effort.as_deref(),
    ) {
        out.push(seg(m, Keep::Model, Style::Plain));
    }
    if input.agent {
        let name = input
            .agent_name
            .as_deref()
            .map(|n| {
                let bounded: String = sanitize(n.trim()).chars().take(MAX_AGENT_CHARS).collect();
                truncate(bounded.trim(), MAX_AGENT_COLS)
                    .trim_end()
                    .to_string()
            })
            .filter(|n| !n.is_empty());
        out.push(seg(
            name.map_or("agente".into(), |n| format!("agente: {n}")),
            Keep::Soft,
            Style::Plain,
        ));
    }
    if let Some(p) = input.ctx_percent.filter(|p| (0.0..=100.0).contains(p)) {
        let shown = p.round() as u32;
        out.push(Segment {
            ctx: true,
            ..seg(format!("ctx {shown}%"), Keep::Essential, ctx_style(shown))
        });
    }
    let Some(s) = snapshot else {
        out.push(seg("hooks sem dados", Keep::Soft, Style::Plain));
        return out;
    };
    out.push(match s.opted_out {
        true => seg("hooks off", Keep::Essential, Style::Red),
        false => seg("hooks on", Keep::Soft, Style::LightBlue),
    });
    if let Some(a) = &s.last_analysis {
        out.push(match a.status {
            AnalysisStatus::Ready => seg("última: pronta", Keep::Soft, Style::Plain),
            AnalysisStatus::Unknown => seg("última: incerta", Keep::Soft, Style::Plain),
            AnalysisStatus::AttentionRequired => {
                seg("última: atenção", Keep::Essential, Style::Yellow)
            }
            AnalysisStatus::Error => seg("última: erro", Keep::Essential, Style::Red),
        });
    }
    out.push(seg(
        format!("inj {}", s.stats.injections),
        Keep::Counter,
        Style::Plain,
    ));
    out.push(seg(
        format!("não reenviados {}", s.stats.session_hits),
        Keep::Counter,
        Style::Plain,
    ));
    if detail {
        out.push(seg(
            format!("entregues {}", s.stats.delivered),
            Keep::Detail,
            Style::Plain,
        ));
        // The snapshot comes from a file: its counters are not trusted to stay in range.
        let whole = s.stats.session_hits.saturating_add(s.stats.delivered);
        if whole > 0 {
            let rate = (s.stats.session_hits as f64 * 100.0 / whole as f64).round() as u64;
            out.push(seg(format!("reuso {rate}%"), Keep::Detail, Style::Plain));
        }
        // Two ages, each named: when the last context reached the model, and when a hook last
        // wrote this snapshot (D-130). One bare `há` next to the context read as the first.
        if let Some(d) = &s.last_delivery {
            out.push(seg(
                format!(
                    "último contexto {} {}",
                    tokens(d.estimated_tokens),
                    age(now.saturating_sub(d.at))
                ),
                Keep::Detail,
                Style::Plain,
            ));
        }
        let elapsed = now.saturating_sub(s.updated_at);
        out.push(seg(
            format!("visto {}", age(elapsed)),
            Keep::Detail,
            Style::Plain,
        ));
        if elapsed > STALE_SECS {
            out.push(seg("dados antigos", Keep::Detail, Style::Plain));
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub detail: bool,
    pub width: usize,
    pub color: bool,
}

pub fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn joined(segs: &[Segment]) -> String {
    segs.iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(SEPARATOR)
}

/// The first `cols` columns of `text`, cut on a character boundary.
fn truncate(text: &str, cols: usize) -> String {
    let (mut out, mut used) = (String::new(), 0);
    for c in text.chars() {
        used += UnicodeWidthChar::width(c).unwrap_or(0);
        if used > cols {
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
    segs.retain(|s| !s.ctx);
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
        Style::LightBlue => "\x1b[38;5;117m",
    };
    format!("{code}{}\x1b[0m", s.text)
}

pub fn render(
    input: &HostInput,
    snapshot: Option<&Snapshot>,
    options: &Options,
    now: u64,
) -> String {
    let segs = fit(
        segments(input, snapshot, options.detail, now),
        options.width,
    );
    segs.iter()
        .map(|s| {
            if options.color {
                paint(s)
            } else {
                s.text.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(SEPARATOR)
}
