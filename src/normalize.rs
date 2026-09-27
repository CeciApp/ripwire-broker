//! Converts ripwire payloads into broker entries. Each entry keeps the verb it came from.

use crate::markup::{self, Node};
use crate::model::{Item, Limitation, Risk, Role, Source, TestItem, Untrusted};

/// One candidate for the final answer, with its selection priority (PRD 10.2, lower first).
#[derive(Debug, Clone)]
pub enum Entry {
    Item(u8, Item),
    Test(u8, TestItem),
    Risk(u8, Risk),
    Limitation(Limitation),
}

pub mod priority {
    pub const CENTRAL: u8 = 2;
    pub const BODY: u8 = 3;
    pub const NEIGHBOUR: u8 = 4;
    pub const CONTRACT: u8 = 5;
    pub const TEST: u8 = 6;
    pub const COCHANGE: u8 = 7;
    pub const DOC: u8 = 8;
    pub const PERIPHERAL: u8 = 9;
}

fn is_doc(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [".md", ".rst", ".txt", ".adoc"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

fn split_loc(p: &str) -> (String, Option<u64>) {
    match p.rsplit_once(':') {
        Some((path, line)) if line.chars().all(|c| c.is_ascii_digit()) && !line.is_empty() => {
            (path.to_string(), line.parse().ok())
        }
        _ => (p.to_string(), None),
    }
}

fn symbol_item(
    verb: &'static str,
    role: Role,
    path: String,
    line: Option<u64>,
    name: &str,
    why: String,
) -> Item {
    Item {
        kind: "symbol",
        role,
        path,
        line,
        symbol: Some(name.to_string()),
        signature: None,
        why_included: why,
        source: Source::fact(verb),
        content: None,
    }
}

fn limitation(verb: &'static str, kind: &'static str, detail: impl Into<String>) -> Entry {
    Entry::Limitation(Limitation {
        kind,
        detail: detail.into(),
        source: Source::fact(verb),
    })
}

/// `<ctx>` bundles from `explore` and `from_trace`.
pub fn ctx(verb: &'static str, payload: &str) -> Vec<Entry> {
    let Some(root) = markup::parse(payload) else {
        return vec![limitation(
            verb,
            "unparsed_upstream",
            "ripwire answer could not be read; nothing was inferred from it",
        )];
    };
    let mut out = Vec::new();
    let frames: Vec<&Node> = root
        .child("trace")
        .map(|t| t.all("frame").collect())
        .unwrap_or_default();
    let bodies: Vec<&Node> = root
        .child("bodies")
        .map(|b| b.all("b").collect())
        .unwrap_or_default();

    if let Some(sigs) = root.child("sigs") {
        for d in sigs.all("d") {
            let (Some(name), Some(path)) = (d.attr("n"), d.attr("p")) else {
                continue;
            };
            let rank = d.attr_u64("r").unwrap_or(0);
            let doc = is_doc(path);
            let frame = frames.iter().find(|f| f.attr("n") == Some(name));
            let (role, why) = if let Some(f) = frame {
                let innermost = if f.flag("innermost") {
                    " (innermost in-repo frame)"
                } else {
                    ""
                };
                (
                    Role::Primary,
                    format!(
                        "stack frame {}{innermost} at {}",
                        f.attr("rank").unwrap_or("?"),
                        f.attr("p").unwrap_or(path)
                    ),
                )
            } else if doc {
                (
                    Role::Doc,
                    format!("documentation ranked {rank} by {verb} for the task"),
                )
            } else {
                (
                    Role::Primary,
                    format!("rank {rank} of the {verb} ranking for the task"),
                )
            };
            let mut item = symbol_item(verb, role, path.to_string(), d.attr_u64("l"), name, why);
            item.kind = if doc { "doc" } else { "symbol" };
            item.signature = Some(d.text.trim().to_string()).filter(|s| !s.is_empty());
            let body = bodies
                .iter()
                .find(|b| b.attr("n") == Some(name) && b.attr("p") == Some(path));
            let prio = if doc {
                priority::DOC
            } else if rank <= 1 {
                priority::CENTRAL
            } else {
                priority::BODY
            };
            if let Some(b) = body {
                item.content = Some(Untrusted {
                    untrusted_repository_data: b.text.clone(),
                });
                if b.flag("truncated") {
                    out.push(limitation(
                        verb,
                        "body_truncated",
                        format!(
                            "{verb} cut the body of {name} ({})",
                            b.attr("lines").unwrap_or("partial")
                        ),
                    ));
                }
            }
            out.push(Entry::Item(prio, item));
        }
        if let Some(far) = sigs.child("far") {
            for s in far.all("s") {
                let (Some(name), Some(p)) = (s.attr("n"), s.attr("p")) else {
                    continue;
                };
                let (path, line) = split_loc(p);
                let why = format!("ranked by {verb} but more than one call hop from the top hits");
                out.push(Entry::Item(
                    priority::PERIPHERAL,
                    symbol_item(verb, Role::Primary, path, line, name, why),
                ));
            }
        }
    }

    if let Some(callers) = root.child("callers") {
        for s in callers.all("s") {
            let (Some(name), Some(p)) = (s.attr("n"), s.attr("p")) else {
                continue;
            };
            let (path, line) = split_loc(p);
            let (role, why) = match s.attr("rel") {
                Some("callee") => (
                    Role::Callee,
                    format!(
                        "called by {} of the top hits",
                        s.attr("shared").unwrap_or("1")
                    ),
                ),
                _ => (
                    Role::Caller,
                    format!("calls {} of the top hits", s.attr("shared").unwrap_or("1")),
                ),
            };
            let mut item = symbol_item(verb, role, path, line, name, why);
            item.signature = Some(s.text.trim().to_string()).filter(|t| !t.is_empty());
            out.push(Entry::Item(priority::NEIGHBOUR, item));
        }
    }

    if let Some(tests) = root.child("tests") {
        out.extend(test_rows(verb, tests));
    }
    if let Some(t) = root.child("trace") {
        let unresolved = t.attr_u64("unresolved").unwrap_or(0) + t.attr_u64("skipped").unwrap_or(0);
        if unresolved > 0 {
            out.push(limitation(
                verb,
                "trace_frames_unresolved",
                format!("{unresolved} trace frames could not be mapped to indexed code"),
            ));
        }
    }
    out.extend(ctx_limitations(verb, &root));
    out
}

/// `<test p= run=>` rows and `<g n= p=a,b>` groups, as used by explore and affected.
fn test_rows(verb: &'static str, parent: &Node) -> Vec<Entry> {
    let mut out = Vec::new();
    for row in &parent.children {
        let paths: Vec<&str> = match row.name.as_str() {
            "test" => row.attr("p").into_iter().collect(),
            "g" => row
                .attr("p")
                .map(|p| p.split(',').collect())
                .unwrap_or_default(),
            _ => continue,
        };
        for p in paths {
            out.push(Entry::Test(
                priority::TEST,
                TestItem {
                    path: p.to_string(),
                    run: row.attr("run").map(str::to_string),
                    why_included: format!("{verb}: test reaches the code in scope"),
                    source: Source::fact(verb),
                },
            ));
        }
        if row.flag("run_unknown") {
            out.push(limitation(
                verb,
                "test_runner_unknown",
                "ripwire could not derive a run command for some tests",
            ));
        }
    }
    out
}

fn ctx_limitations(verb: &'static str, root: &Node) -> Vec<Entry> {
    let mut out = Vec::new();
    if let Some(n) = root.attr_u64("dropped_positive").filter(|n| *n > 0) {
        out.push(limitation(
            verb,
            "upstream_truncated",
            format!("{verb} dropped {n} ranked candidates to fit its budget"),
        ));
    }
    for section in ["sigs", "bodies", "tests", "callers"] {
        if let Some(s) = root.child(section).filter(|s| s.flag("capped")) {
            out.push(limitation(
                verb,
                "upstream_truncated",
                format!(
                    "{verb} {section}: showed {} of {}",
                    s.attr("shown").unwrap_or("?"),
                    s.attr("total").unwrap_or("?")
                ),
            ));
        }
    }
    if root.flag("over_ceiling") {
        out.push(limitation(
            verb,
            "upstream_truncated",
            format!("{verb}: requested budget is below its minimum answer size"),
        ));
    }
    out
}

fn graph_limitations(verb: &'static str, v: &serde_json::Value) -> Vec<Entry> {
    let mut out = Vec::new();
    let truthy =
        |k: &str| v[k].as_bool().unwrap_or(false) || v[k].as_u64() == Some(1) || v[k] == "1";
    if truthy("counts_floor") {
        out.push(limitation(verb, "counts_floor", "caller/reach counts are lower bounds, not totals; zero means none found, not none exists"));
    }
    let ambiguous =
        v["graph_ambiguous"].as_u64().unwrap_or(0) + v["graph_unresolved"].as_u64().unwrap_or(0);
    if ambiguous > 0 {
        out.push(limitation(
            verb,
            "graph_ambiguous",
            format!("{ambiguous} call edges are ambiguous or unresolved in the index"),
        ));
    }
    if let Some(n) = v["graph_unindexed"].as_u64().filter(|n| *n > 0) {
        out.push(limitation(
            verb,
            "unindexed_files",
            format!("{n} files could not be indexed"),
        ));
    }
    out
}

fn json_symbol(verb: &'static str, role: Role, s: &serde_json::Value, why: String) -> Option<Item> {
    Some(symbol_item(
        verb,
        role,
        s["file"].as_str()?.to_string(),
        s["line"].as_u64(),
        s["name"].as_str()?,
        why,
    ))
}

/// `find_symbol` JSON: the symbol, its direct callers and callees. Returns entries and the body handle.
pub fn find_symbol(payload: &str) -> (Vec<Entry>, Option<String>) {
    let verb = "find_symbol";
    let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) else {
        return (
            vec![limitation(
                verb,
                "unparsed_upstream",
                "ripwire answer could not be read; nothing was inferred from it",
            )],
            None,
        );
    };
    let mut out = Vec::new();
    if let Some(item) = json_symbol(
        verb,
        Role::Primary,
        &v["symbol"],
        "the symbol named in the task".into(),
    ) {
        out.push(Entry::Item(priority::CENTRAL, item));
    }
    for (key, role, why) in [
        ("calledBy", Role::Caller, "direct caller (1 hop)"),
        ("calls", Role::Callee, "direct callee (1 hop)"),
    ] {
        for s in v[key].as_array().into_iter().flatten() {
            if let Some(item) = json_symbol(verb, role, s, why.into()) {
                out.push(Entry::Item(priority::NEIGHBOUR, item));
            }
        }
    }
    if v["has_more"].as_bool() == Some(true) {
        out.push(limitation(
            verb,
            "upstream_truncated",
            "find_symbol paged its caller/callee lists; more rows exist",
        ));
    }
    out.extend(graph_limitations(verb, &v));
    (out, v["symbol"]["handle"].as_str().map(str::to_string))
}

/// Attaches a `fetch_body` answer to the primary symbol it was fetched for.
pub fn attach_body(entries: &mut [Entry], payload: &str) -> Vec<Entry> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) else {
        return vec![];
    };
    let Some(body) = v["body"].as_str() else {
        return vec![];
    };
    for e in entries.iter_mut() {
        if let Entry::Item(prio, item) = e
            && item.role == Role::Primary
            && item.symbol.as_deref() == v["name"].as_str()
            && Some(item.path.as_str()) == v["file"].as_str()
        {
            item.content = Some(Untrusted {
                untrusted_repository_data: body.to_string(),
            });
            *prio = priority::CENTRAL;
        }
    }
    if v["partial"].as_bool() == Some(true) {
        return vec![limitation(
            "fetch_body",
            "body_truncated",
            "fetch_body served a partial body",
        )];
    }
    vec![]
}

/// `<impact>`: the transitive reach of a symbol and the files importing it.
pub fn impact(payload: &str) -> Vec<Entry> {
    let verb = "impact";
    let Some(root) = markup::parse(payload) else {
        return vec![limitation(
            verb,
            "unparsed_upstream",
            "ripwire answer could not be read; nothing was inferred from it",
        )];
    };
    let of = root.attr("of").unwrap_or("the symbol");
    let mut out = Vec::new();
    for s in root.all("s") {
        let (Some(name), Some(p)) = (s.attr("n"), s.attr("p")) else {
            continue;
        };
        let (path, line) = split_loc(p);
        let why = format!("transitively reaches {of}; may break if its contract changes");
        out.push(Entry::Item(
            priority::CONTRACT,
            symbol_item(verb, Role::Caller, path, line, name, why),
        ));
    }
    for f in root.all("f") {
        let Some(path) = f.attr("p") else { continue };
        out.push(Entry::Item(
            priority::CONTRACT,
            Item {
                kind: "file",
                role: Role::Caller,
                path: path.to_string(),
                line: None,
                symbol: None,
                signature: None,
                why_included: format!("imports {of} ({})", f.attr("via").unwrap_or("import")),
                source: Source::fact(verb),
                content: None,
            },
        ));
    }
    if root.flag("capped") {
        out.push(limitation(
            verb,
            "upstream_truncated",
            format!(
                "impact listed {} of {} reached symbols",
                root.attr("shown").unwrap_or("?"),
                root.attr("reaches").unwrap_or("?")
            ),
        ));
    }
    out.extend(markup_graph_limitations(verb, &root));
    out
}

fn markup_graph_limitations(verb: &'static str, root: &Node) -> Vec<Entry> {
    let as_json = serde_json::json!({
        "counts_floor": root.flag("counts_floor"),
        "graph_ambiguous": root.attr_u64("graph_ambiguous").unwrap_or(0),
        "graph_unresolved": root.attr_u64("graph_unresolved").unwrap_or(0),
        "graph_unindexed": root.attr_u64("graph_unindexed").unwrap_or(0),
    });
    graph_limitations(verb, &as_json)
}

/// `memory_recall` plain text: a header, then `━━ path (relevance r) ━━ [...lines="a-b"]` blocks.
pub fn recall(payload: &str) -> Vec<Entry> {
    let verb = "memory_recall";
    let mut out = Vec::new();
    let mut blocks = payload.split("━━ ");
    let header = blocks.next().unwrap_or("");
    let mut rest: Vec<&str> = blocks.collect();
    // Blocks come as pairs: "path  (relevance r) " then " [meta]\ncontent".
    while rest.len() >= 2 {
        let head = rest.remove(0);
        let body = rest.remove(0);
        let path = head.split_whitespace().next().unwrap_or("").to_string();
        let (meta, content) = body.split_once('\n').unwrap_or((body, ""));
        let line = meta
            .split("lines=\"")
            .nth(1)
            .and_then(|l| l.split(['-', '"']).next())
            .and_then(|l| l.parse().ok());
        out.push(Entry::Item(
            priority::CENTRAL,
            Item {
                kind: "doc",
                role: Role::Doc,
                path,
                line,
                symbol: None,
                signature: None,
                why_included: format!(
                    "memory_recall: {}",
                    head.trim()
                        .trim_start_matches(|c: char| !c.is_whitespace())
                        .trim()
                ),
                source: Source::fact(verb),
                content: Some(Untrusted {
                    untrusted_repository_data: content.trim_end().to_string(),
                }),
            },
        ));
    }
    if header.contains("capped=1") {
        out.push(limitation(
            verb,
            "upstream_truncated",
            "memory_recall omitted some relevant documents to fit its budget",
        ));
    }
    out
}

fn risk(
    verb: &'static str,
    prio: u8,
    kind: &'static str,
    message: String,
    path: Option<&str>,
) -> Entry {
    Entry::Risk(
        prio,
        Risk {
            kind,
            message,
            path: path.map(str::to_string),
            symbol: None,
            source: Source::fact(verb),
        },
    )
}

/// `tests_to_run` rows: `test` (or `p`) is a path or an array of paths; `run` or `run_unknown`.
fn json_tests(verb: &'static str, rows: &serde_json::Value) -> Vec<Entry> {
    let mut out = Vec::new();
    for row in rows.as_array().into_iter().flatten() {
        let key = if row.get("test").is_some() {
            "test"
        } else {
            "p"
        };
        let paths: Vec<&str> = match &row[key] {
            serde_json::Value::String(p) => vec![p.as_str()],
            serde_json::Value::Array(ps) => ps.iter().filter_map(|p| p.as_str()).collect(),
            _ => vec![],
        };
        for p in paths {
            out.push(Entry::Test(
                priority::TEST,
                TestItem {
                    path: p.to_string(),
                    run: row["run"].as_str().map(str::to_string),
                    why_included: format!("{verb}: test reaches the changed code"),
                    source: Source::fact(verb),
                },
            ));
        }
        if row["run_unknown"].as_bool() == Some(true) {
            out.push(limitation(
                verb,
                "test_runner_unknown",
                "ripwire could not derive a run command for some tests",
            ));
        }
    }
    out
}

/// `situational_awareness` JSON: blast radius, tests, forgotten co-change partners, hotspots.
pub fn situation(payload: &str) -> Vec<Entry> {
    let verb = "situational_awareness";
    let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) else {
        return vec![limitation(
            verb,
            "unparsed_upstream",
            "ripwire answer could not be read; nothing was inferred from it",
        )];
    };
    let mut out = Vec::new();
    for row in v["blast_radius"].as_array().into_iter().flatten() {
        let Some(file) = row["file"].as_str() else {
            continue;
        };
        out.push(Entry::Item(
            priority::CONTRACT,
            Item {
                kind: "file",
                role: Role::Caller,
                path: file.to_string(),
                line: None,
                symbol: None,
                signature: None,
                why_included: format!(
                    "{} symbols here depend on the changed code",
                    row["dependent_symbols"].as_u64().unwrap_or(0)
                ),
                source: Source::fact(verb),
                content: None,
            },
        ));
    }
    out.extend(json_tests(verb, &v["tests_to_run"]));
    for row in v["forgotten"].as_array().into_iter().flatten() {
        let Some(file) = row["file"].as_str() else {
            continue;
        };
        let degree = row["cochange_degree"].as_f64().unwrap_or(0.0);
        out.push(risk(verb, priority::CONTRACT, "cochange_missing", format!("{file} usually changes together with this diff (co-change degree {degree:.2}) but is not in it"), Some(file)));
    }
    for row in v["hotspot_alert"].as_array().into_iter().flatten() {
        let Some(file) = row["file"].as_str() else {
            continue;
        };
        out.push(risk(
            verb,
            priority::COCHANGE,
            "hotspot",
            format!("{file} is a change hotspot (score {})", row["score"]),
            Some(file),
        ));
    }
    for (flag, what) in [
        ("blast_radius_capped", "blast radius"),
        ("forgotten_capped", "co-change partners"),
    ] {
        if v[flag].as_bool() == Some(true) {
            out.push(limitation(
                verb,
                "upstream_truncated",
                format!("situational_awareness capped the {what} list"),
            ));
        }
    }
    if let Some(n) = v["script_gates_unmodelled"].as_u64().filter(|n| *n > 0) {
        out.push(limitation(
            verb,
            "script_gates_unmodelled",
            format!("{n} script test runners are not modelled by the call graph"),
        ));
    }
    out.extend(graph_limitations(verb, &v));
    out
}

/// `<edit-check>`: whether a symbol's contract changed and which callers now disagree with it.
/// Returns the entries and whether the contract changed (so the caller may ask for `impact`).
pub fn edit_check(payload: &str) -> (Vec<Entry>, bool) {
    let verb = "edit_check";
    let Some(root) = markup::parse(payload) else {
        return (
            vec![limitation(
                verb,
                "unparsed_upstream",
                "ripwire answer could not be read; nothing was inferred from it",
            )],
            false,
        );
    };
    let sym = root.attr("sym").unwrap_or("?").to_string();
    let changed = root.attr("status") == Some("contract-change");
    let mut out = Vec::new();
    if changed {
        let mut parts = Vec::new();
        if root.attr("params_was") != root.attr("params_now") {
            parts.push(format!(
                "parameter count {} -> {}",
                root.attr("params_was").unwrap_or("?"),
                root.attr("params_now").unwrap_or("?")
            ));
        }
        if root.attr("public_was") != root.attr("public_now") {
            parts.push(format!(
                "visibility {} -> {}",
                root.attr("public_was").unwrap_or("?"),
                root.attr("public_now").unwrap_or("?")
            ));
        }
        let (path, line) = split_loc(root.attr("p").unwrap_or(""));
        out.push(Entry::Risk(
            priority::CENTRAL,
            Risk {
                kind: "contract_change",
                message: format!(
                    "{sym} ({path}:{}): {}; {} of {} known callers are incompatible",
                    line.unwrap_or(0),
                    parts.join(", "),
                    root.attr("incompatible").unwrap_or("?"),
                    root.attr("callers").unwrap_or("?")
                ),
                path: Some(path),
                symbol: Some(sym.clone()),
                source: Source::fact(verb),
            },
        ));
    }
    for c in root.all("c") {
        let (Some(name), Some(p)) = (c.attr("n"), c.attr("p")) else {
            continue;
        };
        let (path, line) = split_loc(p);
        let (prio, why) = if c.flag("incompatible") {
            (
                priority::NEIGHBOUR,
                format!(
                    "caller incompatible with the new contract of {sym} (call at line {})",
                    c.attr("sites_l").unwrap_or("?")
                ),
            )
        } else {
            (
                priority::CONTRACT,
                format!("direct caller of {sym}; contract still compatible"),
            )
        };
        out.push(Entry::Item(
            prio,
            symbol_item(verb, Role::Caller, path, line, name, why),
        ));
    }
    out.extend(markup_graph_limitations(verb, &root));
    (out, changed)
}

/// Files the working tree changed, as reported by `situational_awareness`.
pub fn changed_files(payload: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|v| v["changed_files"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|f| f["file"].as_str().map(str::to_string))
        .collect()
}

/// Outcome of `quality_delta`: evidence entries plus the counts the gate decides on.
pub struct QualityDelta {
    pub entries: Vec<Entry>,
    pub regressions: usize,
    pub minor: usize,
}

/// `quality_delta` JSON: only what the working tree made worse (`r` rows).
pub fn quality_delta(payload: &str) -> Option<QualityDelta> {
    let verb = "quality_delta";
    let v = serde_json::from_str::<serde_json::Value>(payload).ok()?;
    v.get("regressions")?;
    let mut entries = Vec::new();
    let (mut regressions, mut minor) = (0, 0);
    for row in v["r"].as_array().into_iter().flatten() {
        let kind = row["kind"].as_str().unwrap_or("regression");
        let sym = row["sym"].as_str().unwrap_or("?");
        let is_minor = row["sev"] == "minor";
        if is_minor {
            minor += 1
        } else {
            regressions += 1
        }
        let delta = match (row.get("was"), row.get("now")) {
            (Some(was), Some(now)) => format!("{kind} {was} -> {now}"),
            _ => kind.to_string(),
        };
        let origin = row["origin"]
            .as_str()
            .map(|o| format!(" ({o})"))
            .unwrap_or_default();
        let (path, _) = split_loc(row["p"].as_str().unwrap_or(""));
        entries.push(Entry::Risk(
            priority::CENTRAL,
            Risk {
                kind: if is_minor {
                    "quality_minor"
                } else {
                    "quality_regression"
                },
                message: format!(
                    "{sym}: {delta}{origin} at {}",
                    row["p"].as_str().unwrap_or("?")
                ),
                path: Some(path).filter(|p| !p.is_empty()),
                symbol: Some(sym.to_string()),
                source: Source::fact(verb),
            },
        ));
    }
    if v["stale"].as_u64().unwrap_or(0) > 0 {
        entries.push(limitation(
            verb,
            "baseline_stale",
            "quality_delta compared against a stale baseline",
        ));
    }
    Some(QualityDelta {
        entries,
        regressions,
        minor,
    })
}

/// `<affected>`: test files that transitively reach the changed files.
pub fn affected(payload: &str) -> Vec<Entry> {
    let verb = "affected";
    let Some(root) = markup::parse(payload) else {
        return vec![limitation(
            verb,
            "unparsed_upstream",
            "ripwire answer could not be read; nothing was inferred from it",
        )];
    };
    let mut out = test_rows(verb, &root);
    if let Some(n) = root.attr_u64("script_gates_unmodelled").filter(|n| *n > 0) {
        out.push(limitation(
            verb,
            "script_gates_unmodelled",
            format!("{n} script test runners are not modelled by the call graph"),
        ));
    }
    out.extend(markup_graph_limitations(verb, &root));
    out
}

/// The router found no signal and explored with part of the budget (PRD 9.1, "caso incerto").
pub fn route_uncertain(verb: &'static str, asked: u32, budget: u32) -> Entry {
    Entry::Limitation(Limitation {
        kind: "route_uncertain",
        detail: format!(
            "no trace, symbol, change or docs signal: {verb} was asked for {asked} of {budget} tokens; \
             pass mode (orient, change, debug, review) or name a symbol for a fuller answer"
        ),
        source: Source {
            verb,
            basis: crate::model::Basis::BrokerInference,
        },
    })
}

/// The task named `symbol`, but ripwire has no such symbol; the answer is an exploration.
pub fn symbol_not_found(symbol: &str) -> Entry {
    Entry::Limitation(Limitation {
        kind: "symbol_not_found",
        detail: format!(
            "'{symbol}' was read as the task's symbol, but the repository has no such symbol; \
             this answer explores the task instead. Name an exact symbol in backticks for a symbol answer"
        ),
        source: Source {
            verb: "find_symbol",
            basis: crate::model::Basis::BrokerInference,
        },
    })
}

pub fn missing_evidence(verb: &'static str, why: &str) -> Entry {
    limitation(
        verb,
        "evidence_missing",
        format!("{verb} gave no usable evidence ({why}); the gate cannot conclude"),
    )
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Deterministic one-line digest (broker inference). Uses only names, paths and counts,
/// never repository text.
pub fn summary(lead: &str, entries: &[Entry]) -> String {
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by_key(|e| match e {
        Entry::Item(p, _) | Entry::Test(p, _) | Entry::Risk(p, _) => *p,
        Entry::Limitation(_) => 0,
    });
    let focus = sorted.iter().find_map(|e| match e {
        Entry::Item(_, i) if i.role == Role::Primary => Some(match (&i.symbol, i.line) {
            (Some(s), Some(l)) => format!("focus {s} ({}:{l})", i.path),
            (Some(s), None) => format!("focus {s} ({})", i.path),
            _ => format!("focus {}", i.path),
        }),
        _ => None,
    });
    let count_risk = |kind: &str| {
        entries
            .iter()
            .filter(|e| matches!(e, Entry::Risk(_, r) if r.kind == kind))
            .count()
    };
    let count = |f: fn(&Entry) -> bool| entries.iter().filter(|e| f(e)).count();
    let mut parts: Vec<String> = Vec::new();
    for (kind, one, many) in [
        (
            "quality_regression",
            "quality regression",
            "quality regressions",
        ),
        ("contract_change", "contract change", "contract changes"),
        (
            "cochange_missing",
            "missing co-change partner",
            "missing co-change partners",
        ),
    ] {
        let n = count_risk(kind);
        if n > 0 {
            parts.push(plural(n, one, many));
        }
    }
    parts.push(plural(
        count(|e| matches!(e, Entry::Item(..))),
        "item",
        "items",
    ));
    parts.push(plural(
        count(|e| matches!(e, Entry::Test(..))),
        "test",
        "tests",
    ));
    parts.push(plural(
        count(|e| matches!(e, Entry::Limitation(..))),
        "limitation",
        "limitations",
    ));
    match focus {
        Some(f) => format!("{lead}: {f}; {}", parts.join(", ")),
        None => format!("{lead}: {}", parts.join(", ")),
    }
}

/// Cuts any item content above `max_bytes` (at a char boundary) and declares the cut.
pub fn cap_items(entries: Vec<Entry>, max_bytes: usize) -> Vec<Entry> {
    let mut out = Vec::with_capacity(entries.len());
    let mut cut = Vec::new();
    for mut e in entries {
        if let Entry::Item(_, item) = &mut e
            && let Some(c) = item
                .content
                .as_mut()
                .filter(|c| c.untrusted_repository_data.len() > max_bytes)
        {
            let text = &c.untrusted_repository_data;
            let mut end = max_bytes;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            c.untrusted_repository_data = text[..end].to_string();
            let what = item.symbol.clone().unwrap_or_else(|| item.path.clone());
            cut.push(limitation(
                item.source.verb,
                "item_truncated",
                format!("content of {what} cut to {max_bytes} bytes; fetch the rest on demand"),
            ));
        }
        out.push(e);
    }
    out.extend(cut);
    out
}
