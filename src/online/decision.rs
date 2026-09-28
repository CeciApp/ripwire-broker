//! Strict thresholds (PRD §23.3–23.4, CA-ONLINE-06): a value equal to the threshold does not
//! pass, and an unknown probability is never read as zero.

/// `file_admission`: `p > 0.25` admits the file.
pub const ADMISSION: f64 = 0.25;
/// `source_selection`: `p > 0.50` selects the block.
pub const SELECTION: f64 = 0.50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDecision {
    Admitted,
    Rejected,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceDecision {
    /// `p > 0.50`: literal source may be rendered.
    Selected,
    /// `0.25 < p ≤ 0.50`: a location worth reading, without source.
    ReadingLead,
    Excluded,
    Unknown,
}

pub fn admit(p: Option<f64>) -> FileDecision {
    match p {
        Some(p) if p > ADMISSION => FileDecision::Admitted,
        Some(_) => FileDecision::Rejected,
        None => FileDecision::Unknown,
    }
}

pub fn select(p: Option<f64>) -> SourceDecision {
    match p {
        Some(p) if p > SELECTION => SourceDecision::Selected,
        Some(p) if p > ADMISSION => SourceDecision::ReadingLead,
        Some(_) => SourceDecision::Excluded,
        None => SourceDecision::Unknown,
    }
}

/// A file evaluated in several fragments keeps its highest known score. It is rejected only
/// when every fragment was evaluated: an unevaluated one may hold the evidence.
pub fn file_decision(fragments: &[Option<f64>]) -> (FileDecision, Option<f64>) {
    let best = fragments.iter().flatten().copied().reduce(f64::max);
    let decision = match admit(best) {
        FileDecision::Rejected if fragments.iter().any(Option::is_none) => FileDecision::Unknown,
        d => d,
    };
    (decision, best)
}
