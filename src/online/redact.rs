//! Remote text that survives into an error (PRD §23.9, v0.1 §13.3, §17): sanitized to
//! printable ASCII, capped, and with every occurrence of the credential redacted before it
//! can reach a status, a log or the agent. Also what `--log` hides: the credential and, for
//! Cloudflare, the account in the URL, written `accounts/[redacted]/` (D-166).

/// What stands for a secret wherever one would be written.
pub const REDACTED: &str = "[redacted]";

/// `text` with every occurrence of each of `secrets` replaced by [`REDACTED`]. An empty secret
/// hides nothing.
pub fn hide(mut text: String, secrets: &[&str]) -> String {
    for secret in secrets.iter().filter(|s| !s.is_empty()) {
        text = text.replace(secret, REDACTED);
    }
    text
}

/// `text` reduced to printable ASCII and spaces, trimmed, with `secret` replaced by
/// `[redacted]`, at most `max` bytes.
pub fn remote_text(text: &str, secret: Option<&str>, max: usize) -> String {
    let printable = |s: &str| -> String {
        s.chars()
            .filter(|c| *c == ' ' || c.is_ascii_graphic())
            .collect()
    };
    let mut clean = printable(text);
    // The filter runs before the replacement, so the only form of the credential that can
    // appear in `clean` is its printable form. Replacing the raw secret missed a credential
    // carrying any non-ASCII character: `ab\u{a9}cd` left `abcd` behind, four of its five
    // characters, in a status, a log or the agent's context (D-109).
    //
    // Redacting the printable form can over-redact — a secret of `a\u{a9}b` also removes a
    // plain `ab` from the text. That is the safe direction for a confidentiality control, and
    // the text here is a remote error message, not something anybody parses.
    if let Some(secret) = secret.filter(|s| !s.is_empty()) {
        let form = printable(secret);
        if !form.is_empty() {
            clean = clean.replace(&form, REDACTED);
        }
    }
    let clean = clean.trim();
    clean[..clean.len().min(max)].to_string()
}
