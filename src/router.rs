//! Deterministic intent routing for `context_for_task` (RF-04).

use crate::model::Intent;

/// Frame shapes of common runtimes: Python, JS/Java/Go/Rust `path:line`, Rust panics.
fn looks_like_trace(task: &str) -> bool {
    let lower = task.to_ascii_lowercase();
    if lower.contains("traceback (most recent call last)") || lower.contains("panicked at") {
        return true;
    }
    let frame_lines = task
        .lines()
        .filter(|l| {
            let t = l.trim();
            (t.starts_with("File \"") && t.contains(", line "))
                || (t.starts_with("at ") && has_path_line(t))
                || (t.starts_with('#') && has_path_line(t))
                || has_path_line(t) && (t.contains("error") || t.contains("Error"))
        })
        .count();
    frame_lines >= 1 && task.lines().count() >= 2
}

/// `something.ext:123` anywhere in the line.
fn has_path_line(line: &str) -> bool {
    line.split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .any(|tok| {
            let mut parts = tok.split(':');
            let path = parts.next().unwrap_or("");
            let line_no = parts.next().unwrap_or("");
            path.contains('.')
                && !path.ends_with('.')
                && !line_no.is_empty()
                && line_no.chars().all(|c| c.is_ascii_digit())
        })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub intent: Intent,
    pub symbol: Option<String>,
}

/// A code-shaped name: `backticked`, snake_case, camelCase, a::path or call().
fn named_symbol(task: &str) -> Option<String> {
    if let Some(start) = task.find('`') {
        let rest = &task[start + 1..];
        if let Some(end) = rest.find('`') {
            let name = rest[..end].trim().trim_end_matches("()");
            if !name.is_empty() && !name.contains(char::is_whitespace) {
                return Some(name.to_string());
            }
        }
    }
    task.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '?' | '!' | '"' | '\''))
        .map(|t| t.trim_end_matches(['.', ':']).trim_end_matches("()"))
        .find(|t| is_code_identifier(t))
        .map(str::to_string)
}

fn is_code_identifier(t: &str) -> bool {
    let ident = |s: &str| {
        !s.is_empty()
            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !s.starts_with(|c: char| c.is_ascii_digit())
    };
    if let Some((scope, name)) = t.rsplit_once("::") {
        return ident(scope.rsplit("::").next().unwrap_or("")) && ident(name);
    }
    if !ident(t) {
        return false;
    }
    let snake = t.trim_matches('_').contains('_');
    let camel = t.chars().skip(1).any(|c| c.is_ascii_uppercase())
        && t.chars().any(|c| c.is_ascii_lowercase());
    snake || camel
}

/// Requests to change code. Each entry is the start of a word ("refator" in "refatorar"); one
/// ending in `$` is a whole word only, where a stem would match inside others ("fix" in
/// "prefix", "add" in "address", "alter" in "alternative").
const CHANGE_WORDS: &[&str] = &[
    "change",
    "modif",
    "renam",
    "refactor",
    "updat",
    "remov",
    "delet",
    "replac",
    "add$",
    "adds$",
    "added$",
    "adding$",
    "signature",
    "migrat",
    "implement",
    "fix$",
    "fixes$",
    "fixed$",
    "fixing$",
    "alter$",
    "alters$",
    "altered$",
    "altering$",
    "mude",
    "mudar",
    "altera",
    "altere",
    "renome",
    "refator",
    "atualiz",
    "apag",
    "substitu",
    "assinatura",
    "adicion",
    "corrij",
    "corrig",
];

/// Requests about documentation, design or past decisions, by the same rule; several words are
/// a phrase, word after word.
const DOC_WORDS: &[&str] = &[
    "documenta",
    "docs$",
    "readme",
    "adr$",
    "adrs$",
    "decision$",
    "decisions$",
    "design doc",
    "rationale",
    "why was",
    "decisão$",
    "decisões$",
    "decisao$",
    "decisoes$",
    "arquitetura",
    "por que foi",
    "motivo$",
];

/// Whole words (English and Portuguese) asking to review existing work; "preview" is not one.
const REVIEW_WORDS: &[&str] = &[
    "review",
    "reviewing",
    "revise",
    "revisar",
    "revisa",
    "revisão",
    "revisao",
];

/// Whether some entry of `words` names a word of `task`, by the rule of [`CHANGE_WORDS`]:
/// never a match inside another word.
fn has_any(task: &str, words: &[&str]) -> bool {
    let lower = task.to_lowercase();
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    let part = |entry: &str, token: &str| match entry.strip_suffix('$') {
        Some(word) => token == word,
        None => token.starts_with(entry),
    };
    words.iter().any(|w| {
        let parts: Vec<&str> = w.split_whitespace().collect();
        tokens
            .windows(parts.len())
            .any(|window| parts.iter().zip(window).all(|(p, t)| part(p, t)))
    })
}

fn has_word(task: &str, words: &[&str]) -> bool {
    task.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .any(|t| words.contains(&t))
}

pub fn classify(task: &str) -> Route {
    if looks_like_trace(task) {
        return Route {
            intent: Intent::Debug,
            symbol: None,
        };
    }
    if has_word(task, REVIEW_WORDS) {
        return Route {
            intent: Intent::Review,
            symbol: None,
        };
    }
    if has_any(task, DOC_WORDS) && !has_any(task, CHANGE_WORDS) {
        return Route {
            intent: Intent::Docs,
            symbol: None,
        };
    }
    let change = has_any(task, CHANGE_WORDS);
    let symbol = named_symbol(task);
    let intent = match (change, symbol.is_some()) {
        (true, _) => Intent::Change,
        (false, true) => Intent::Symbol,
        (false, false) => Intent::Orient,
    };
    Route { intent, symbol }
}

/// Applies an explicit mode; `auto` classifies. Change keeps the named symbol, if any.
pub fn route(task: &str, mode: crate::broker::Mode) -> Route {
    use crate::broker::Mode;
    let auto = classify(task);
    match mode {
        Mode::Auto => auto,
        Mode::Orient => Route {
            intent: Intent::Orient,
            symbol: None,
        },
        Mode::Debug => Route {
            intent: Intent::Debug,
            symbol: None,
        },
        Mode::Review => Route {
            intent: Intent::Review,
            symbol: None,
        },
        Mode::Change => Route {
            intent: Intent::Change,
            symbol: auto.symbol,
        },
    }
}
