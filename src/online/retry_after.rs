//! `Retry-After` (v0.1 §11.8, RFC 9110 §10.2.3): delay seconds or an HTTP date. Only the
//! preferred IMF-fixdate form is read (`Sun, 06 Nov 1994 08:49:37 GMT`); anything else is
//! `None`, and the scheduler falls back to its default wait.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// How long to wait from `now`; a date in the past means no wait.
pub fn parse(value: &str, now: SystemTime) -> Option<Duration> {
    let value = value.trim();
    if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
        return value.parse().ok().map(Duration::from_secs);
    }
    let at = UNIX_EPOCH + Duration::from_secs(imf_fixdate(value)?);
    Some(at.duration_since(now).unwrap_or(Duration::ZERO))
}

/// Seconds since the epoch of an IMF-fixdate.
fn imf_fixdate(s: &str) -> Option<u64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    let [weekday, day, month, year, time, "GMT"] = parts[..] else {
        return None;
    };
    if !(weekday.len() == 4 && weekday.ends_with(',')) {
        return None;
    }
    let day: u64 = number(day, 2)?;
    let month = MONTHS.iter().position(|m| *m == month)? as u64 + 1;
    let year: u64 = number(year, 4)?;
    let hms: Vec<&str> = time.split(':').collect();
    let [h, m, sec] = hms[..] else { return None };
    let (h, m, sec): (u64, u64, u64) = (number(h, 2)?, number(m, 2)?, number(sec, 2)?);
    if !(1..=days_in_month(year, month)).contains(&day) || h > 23 || m > 59 || sec > 59 {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    Some(days * 86_400 + h * 3_600 + m * 60 + sec)
}

fn number(s: &str, digits: usize) -> Option<u64> {
    (s.len() == digits && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

fn days_in_month(year: u64, month: u64) -> u64 {
    let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to a proleptic Gregorian date (H. Hinnant's `days_from_civil`).
fn days_from_civil(year: u64, month: u64, day: u64) -> Option<u64> {
    let y = year as i64 - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    u64::try_from(era * 146_097 + doe - 719_468).ok()
}
