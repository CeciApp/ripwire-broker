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
        label = label
            .chars()
            .take(MAX_MODEL_CHARS - 1)
            .chain(['…'])
            .collect();
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
}

fn seg(text: impl Into<String>, keep: Keep, style: Style) -> Segment {
    Segment {
        text: text.into(),
        keep,
        style,
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
    if let Some(p) = input.ctx_percent.filter(|p| (0.0..=100.0).contains(p)) {
        let shown = p.round() as u32;
        out.push(seg(
            format!("ctx {shown}%"),
            Keep::Essential,
            ctx_style(shown),
        ));
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
        let whole = s.stats.session_hits + s.stats.delivered;
        if whole > 0 {
            let rate = (s.stats.session_hits as f64 * 100.0 / whole as f64).round() as u64;
            out.push(seg(format!("reuso {rate}%"), Keep::Detail, Style::Plain));
        }
        if let Some(d) = &s.last_delivery {
            out.push(seg(
                format!("último contexto {}", tokens(d.estimated_tokens)),
                Keep::Detail,
                Style::Plain,
            ));
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
pub struct Options {
    pub detail: bool,
    pub width: usize,
    pub color: bool,
}

/// This task: join everything. Task 3 replaces the body with fit-then-color.
pub fn render(
    input: &HostInput,
    snapshot: Option<&Snapshot>,
    options: &Options,
    now: u64,
) -> String {
    segments(input, snapshot, options.detail, now)
        .into_iter()
        .map(|s| s.text)
        .collect::<Vec<_>>()
        .join(SEPARATOR)
}
