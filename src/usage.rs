//! `hook-stats` (§21.3): the saved hook sessions reduced to the measurement that decides
//! whether a persistent cache is worth building. Counts only; never a path, symbol, prompt,
//! fingerprint or session id (PRD 16.1).

use crate::hook::SessionState;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct UsageReport {
    pub sessions: usize,
    pub events: u64,
    pub injections: u64,
    /// Items, tests, risks and notes delivered whole.
    pub delivered: u64,
    /// Left out or reduced to a reference because the session already had them.
    pub session_hits: u64,
    /// `session_hits / (session_hits + delivered)`; `None` before anything was delivered.
    pub hit_rate: Option<f64>,
    pub cross_session: CrossSession,
}

/// What a cache shared between sessions would have saved on top of the per-session one.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CrossSession {
    /// Fingerprints held by every session but the earliest.
    pub fingerprints: usize,
    /// Of those, how many an earlier session had already been delivered.
    pub repeated: usize,
    /// `repeated / fingerprints`; `None` with fewer than two sessions.
    pub rate: Option<f64>,
}

fn ratio(part: u64, whole: u64) -> Option<f64> {
    (whole > 0).then(|| part as f64 / whole as f64)
}

pub fn report(sessions: &[SessionState]) -> UsageReport {
    let mut r = UsageReport {
        sessions: sessions.len(),
        ..UsageReport::default()
    };
    for s in sessions {
        r.events += s.stats.events;
        r.injections += s.stats.injections;
        r.delivered += s.stats.delivered;
        r.session_hits += s.stats.session_hits;
    }
    r.hit_rate = ratio(r.session_hits, r.session_hits + r.delivered);

    // In the order the sessions started, never the order of the files.
    let mut ordered: Vec<&SessionState> = sessions.iter().collect();
    ordered.sort_by_key(|s| s.stats.started_at);
    let mut earlier: HashSet<&str> = HashSet::new();
    for (n, s) in ordered.iter().enumerate() {
        for f in s.memory.fingerprints() {
            if n > 0 {
                r.cross_session.fingerprints += 1;
                r.cross_session.repeated += usize::from(earlier.contains(f));
            }
        }
        earlier.extend(s.memory.fingerprints());
    }
    if sessions.len() > 1 {
        r.cross_session.rate = ratio(
            r.cross_session.repeated as u64,
            r.cross_session.fingerprints as u64,
        );
    }
    r
}

fn percent(rate: Option<f64>) -> String {
    rate.map_or_else(|| "n/a".into(), |r| format!("{:.1}%", r * 100.0))
}

pub fn render(r: &UsageReport) -> String {
    if r.sessions == 0 {
        return "no sessions recorded\n".into();
    }
    format!(
        "{} sessions · {} hook events · {} injections\n\
         within a session: {} not resent, {} delivered whole (hit rate {})\n\
         across sessions: {} of {} fingerprints in later sessions had been delivered before ({}): \
         what a persistent cache could add (PRD §21.3)\n",
        r.sessions,
        r.events,
        r.injections,
        r.session_hits,
        r.delivered,
        percent(r.hit_rate),
        r.cross_session.repeated,
        r.cross_session.fingerprints,
        percent(r.cross_session.rate),
    )
}
