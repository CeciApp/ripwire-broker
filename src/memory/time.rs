//! Time of a memory (PRD jev-mem §5.4). The wall clock is recorded as evidence of when
//! something was observed, nothing more; order comes from a sequence the store persists, which
//! never goes back when the clock does.

use super::admission::Stamp;
use serde::{Deserialize, Serialize};

/// The wall clock, injectable so a test can turn it back.
pub trait Clock {
    fn now_ms(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
    }
}

/// `ingest_seq` of a workspace: receipt order in its store, not causality.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sequence {
    last: u64,
}

impl Sequence {
    /// Continues after `last`, the highest sequence the store holds.
    pub fn resume(&mut self, last: u64) {
        self.last = last;
    }

    /// The next sequence number; `None` once the sequence is exhausted.
    pub fn advance(&mut self) -> Option<u64> {
        self.last = self.last.checked_add(1)?;
        Some(self.last)
    }

    /// The next stamp; `None` once the sequence is exhausted, rather than wrapping around.
    pub fn stamp(
        &mut self,
        clock: &dyn Clock,
        generation: u64,
        retention_ms: u64,
    ) -> Option<Stamp> {
        let next = self.advance()?;
        Some(Stamp {
            observed_at_ms: clock.now_ms(),
            ingest_seq: next,
            generation,
            retention_ms,
        })
    }
}

/// How long ago `then` was; `None` when the clock went back, never a negative duration.
pub fn elapsed_ms(now: u64, then: u64) -> Option<u64> {
    now.checked_sub(then)
}

/// `YYYY-MM-DD HH:MM:SS UTC`, without a date crate (days to civil date: H. Hinnant).
pub fn utc(secs: u64) -> String {
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn dates_are_civil_utc() {
        assert_eq!(super::utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(super::utc(1791312721), "2026-10-06 18:52:01 UTC");
        assert_eq!(super::utc(1709164800), "2024-02-29 00:00:00 UTC");
    }
}
