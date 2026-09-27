//! `EvidenceMerger` (PRD §23.4, D-061, D-063): additive. A structural item keeps its
//! provenance and gains a `semantic` annotation; a selected block of a ripwire symbol promotes
//! it to the central band; evidence no ripwire symbol matches becomes a `semantic_location`.
//! Nothing structural is removed or demoted, and no caller, test, risk or contract is created.

use super::SemanticStage;
use super::coordinator::{Discovery, FileEvidence, Scored};
use super::decision::{self, FileDecision, SourceDecision, select};
use crate::model::{
    Basis, Item, Limitation, OnlineProvenance, Role, SemanticEvidence, Source, Untrusted,
};
use crate::normalize::{Entry, priority};

const JEV: Source = Source {
    verb: "jev",
    basis: Basis::RemoteClassifier,
};

const INFERENCE: Source = Source {
    verb: "jev",
    basis: Basis::BrokerInference,
};

fn evidence(
    stage: SemanticStage,
    state: &'static str,
    scored: &Scored,
    file: &FileEvidence,
    lines: Option<[u64; 2]>,
    model: &str,
) -> Option<Box<SemanticEvidence>> {
    Some(Box::new(SemanticEvidence {
        stage,
        state,
        probability: scored.probability?,
        threshold: match stage {
            SemanticStage::FileAdmission => decision::ADMISSION,
            SemanticStage::SourceSelection => decision::SELECTION,
        },
        model: model.into(),
        request_digest: scored.request_digest.clone(),
        content_hash: file.content_hash.clone(),
        lines,
        cache_hit: scored.cache_hit,
    }))
}

fn source_state(d: SourceDecision) -> &'static str {
    match d {
        SourceDecision::Selected => "selected_source",
        SourceDecision::ReadingLead => "reading_lead",
        SourceDecision::Excluded | SourceDecision::Unknown => "excluded",
    }
}

/// `entries` with the discovery's evidence merged in.
pub(crate) fn merge(
    entries: Vec<Entry>,
    disc: &Discovery,
    model: &str,
    max_source_bytes: Option<usize>,
) -> Vec<Entry> {
    let mut rendered = 0usize;
    let mut capped = 0usize;
    let mut out: Vec<Entry> = entries
        .into_iter()
        .map(|e| match e {
            Entry::Item(p, mut item) if item.source.basis == Basis::Ripwire => {
                let Some(file) = disc.files.iter().find(|f| f.path == item.path) else {
                    return Entry::Item(p, item);
                };
                let unit = file.units.iter().find(|u| {
                    item.line.is_some()
                        && u.unit.symbol_line == item.line
                        && u.scored.probability.is_some()
                });
                let mut p = p;
                item.semantic = match unit {
                    Some(u) => {
                        let d = select(u.scored.probability);
                        if d == SourceDecision::Selected {
                            p = p.min(priority::CENTRAL);
                        }
                        let lines = Some([u.unit.start_line, u.unit.end_line]);
                        evidence(
                            SemanticStage::SourceSelection,
                            source_state(d),
                            &u.scored,
                            file,
                            lines,
                            model,
                        )
                    }
                    None => {
                        let state = match file.decision {
                            FileDecision::Admitted => "admitted",
                            _ => "rejected",
                        };
                        evidence(
                            SemanticStage::FileAdmission,
                            state,
                            &file.admission,
                            file,
                            None,
                            model,
                        )
                    }
                };
                Entry::Item(p, item)
            }
            other => other,
        })
        .collect();

    // Semantic-only items, ordered among themselves by probability before they join the
    // entries; ripwire's items keep their order, and the scores never mix (D-081, §23.4).
    let mut found: Vec<Entry> = vec![];
    for file in disc
        .files
        .iter()
        .filter(|f| f.decision == FileDecision::Admitted)
    {
        let structural_lines: Vec<u64> = out
            .iter()
            .filter_map(|e| match e {
                Entry::Item(_, i) if i.path == file.path && i.source.basis == Basis::Ripwire => {
                    i.line
                }
                _ => None,
            })
            .collect();
        let has_structural = out.iter().any(|e| {
            matches!(e, Entry::Item(_, i) if i.path == file.path && i.source.basis == Basis::Ripwire)
        });
        let mut added = false;
        for u in &file.units {
            let linked = u
                .unit
                .symbol_line
                .is_some_and(|l| structural_lines.contains(&l));
            let d = select(u.scored.probability);
            if linked || !matches!(d, SourceDecision::Selected | SourceDecision::ReadingLead) {
                continue;
            }
            let p = u.scored.probability.unwrap_or_default();
            let (start, end) = (u.unit.start_line, u.unit.end_line);
            let beside = beside(file);
            let (prio, why, content) = match d {
                SourceDecision::Selected
                    if max_source_bytes.is_some_and(|cap| rendered + u.text.len() > cap) =>
                {
                    capped += 1;
                    (
                        priority::BODY,
                        format!(
                            "classifier: lines {start}-{end} are evidence for the task (p={p:.2} > 0.50); source omitted by --jev-max-source-bytes"
                        ),
                        None,
                    )
                }
                SourceDecision::Selected => {
                    rendered += u.text.len();
                    (
                        priority::BODY,
                        format!(
                            "classifier: lines {start}-{end} are evidence for the task (p={p:.2} > 0.50)"
                        ),
                        Some(Untrusted {
                            untrusted_repository_data: u.text.clone(),
                        }),
                    )
                }
                _ => (
                    priority::READING_LEAD,
                    format!(
                        "classifier: lines {start}-{end} may be relevant (p={p:.2}); a reading lead, no source"
                    ),
                    None,
                ),
            };
            found.push(Entry::Item(
                prio,
                Item {
                    kind: "semantic_location",
                    role: Role::Semantic,
                    path: file.path.clone(),
                    line: Some(start),
                    symbol: None,
                    signature: None,
                    why_included: format!("{why}{beside}"),
                    source: JEV,
                    content,
                    semantic: evidence(
                        SemanticStage::SourceSelection,
                        source_state(d),
                        &u.scored,
                        file,
                        Some([start, end]),
                        model,
                    ),
                },
            ));
            added = true;
        }
        if !added && !has_structural {
            let p = file.admission.probability.unwrap_or_default();
            found.push(Entry::Item(
                priority::SEMANTIC_LOCATION,
                Item {
                    kind: "semantic_location",
                    role: Role::Semantic,
                    path: file.path.clone(),
                    line: None,
                    symbol: None,
                    signature: None,
                    why_included: format!(
                        "classifier admitted the file (p={p:.2} > 0.25); no block selected{}",
                        beside(file)
                    ),
                    source: JEV,
                    content: None,
                    semantic: evidence(
                        SemanticStage::FileAdmission,
                        "admitted",
                        &file.admission,
                        file,
                        None,
                        model,
                    ),
                },
            ));
        }
    }
    let p = |e: &Entry| match e {
        Entry::Item(_, i) => i.semantic.as_ref().map_or(0.0, |s| s.probability),
        _ => 0.0,
    };
    found.sort_by(|a, b| p(b).total_cmp(&p(a)));
    out.extend(found);
    out.extend(limitations(disc).into_iter().map(Entry::Limitation));
    if capped > 0 {
        out.push(Entry::Limitation(limitation(
            "semantic_source_capped",
            format!(
                "{capped} selected block(s) shown as locations: --jev-max-source-bytes reached"
            ),
        )));
    }
    out
}

/// Says where a lookahead file came from; ripwire never ranked it.
fn beside(file: &FileEvidence) -> &'static str {
    match file.lookahead {
        true => "; found beside a ripwire candidate",
        false => "",
    }
}

fn limitation(kind: &'static str, detail: String) -> Limitation {
    Limitation {
        kind,
        detail,
        source: INFERENCE,
    }
}

/// What the discovery could not do, as limitations (no paths, no remote text).
fn limitations(disc: &Discovery) -> Vec<Limitation> {
    let mut out = vec![];
    if !disc.not_sent.is_empty() {
        let reasons: Vec<String> = disc
            .not_sent
            .iter()
            .map(|(why, n)| format!("{why} {n}"))
            .collect();
        out.push(limitation(
            "semantic_not_sent",
            format!(
                "candidate files not sent to the classifier by policy: {}",
                reasons.join(", ")
            ),
        ));
    }
    if disc.too_large > 0 {
        out.push(limitation(
            "request_too_large",
            format!(
                "{} question(s) too large to send even alone",
                disc.too_large
            ),
        ));
    }
    if disc.incomplete() {
        let mut why: Vec<String> = disc
            .failures
            .iter()
            .map(|(c, n)| format!("{n} request(s) failed: {c}"))
            .collect();
        if disc.limit_reached {
            why.push("request limit reached".into());
        }
        if disc.stale_batches > 0 {
            why.push(format!(
                "{} batch(es) not sent: their source changed after it was read",
                disc.stale_batches
            ));
        }
        if disc.changed_files > 0 {
            why.push(format!(
                "{} file(s) changed during discovery; their evidence was dropped",
                disc.changed_files
            ));
        }
        if disc.unknown_answers > 0 {
            why.push(format!(
                "{} answer(s) unknown: no valid probability",
                disc.unknown_answers
            ));
        }
        if let Some(ms) = disc.interrupted {
            why.push(format!("discovery deadline of {ms} ms reached"));
        }
        if disc.unfinished > 0 {
            why.push(format!("{} request(s) not answered", disc.unfinished));
        }
        if !disc.not_sent.is_empty() || disc.too_large > 0 {
            why.push("some candidates were not sent".into());
        }
        out.push(limitation(
            "semantic_incomplete",
            format!(
                "semantic discovery incomplete ({}); an absence is not evidence of irrelevance",
                why.join("; ")
            ),
        ));
    }
    out
}

/// The limitation of a route the classifier does not run on (D-060).
pub(crate) fn skipped(route: &str) -> Entry {
    Entry::Limitation(limitation(
        "semantic_skipped",
        format!(
            "the classifier runs only on explore routes; the {route} route has more precise structural evidence"
        ),
    ))
}

pub(crate) fn provenance(
    provider: &str,
    model: &str,
    disc: Option<&Discovery>,
) -> OnlineProvenance {
    OnlineProvenance {
        enabled: true,
        provider: provider.into(),
        model: model.into(),
        requests: disc.map_or(0, |d| d.requests),
        cache_hits: disc.map_or(0, |d| d.cache_hits),
        incomplete: disc.is_some_and(Discovery::incomplete),
        discovery: match disc {
            None => "skipped",
            Some(d) if d.interrupted.is_some() => "interrupted",
            Some(d) if d.incomplete() => "incomplete",
            Some(_) => "complete",
        },
    }
}
