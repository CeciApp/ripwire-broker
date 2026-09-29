//! Property tests (D-109). Run this target alone with
//! `cargo nextest run -E 'binary(props)'`, and raise the case count with
//! `PROPTEST_CASES=4096 cargo test --test props`.
//!
//! Only pure functions live here. Everything is reachable from `tests/` in the **default**
//! build — no feature flag, and no property touches the network, spawns a process or writes
//! to disk. The properties that need a workspace on disk are a separate target, so that this
//! one stays fast enough to run with a high case count.

use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, TestCaseError};
use ripwire_broker::notes;
use ripwire_broker::online::cache::{self, KeyParts};
use ripwire_broker::online::decision::{self, FileDecision, SourceDecision};
use ripwire_broker::online::redact::remote_text;
use ripwire_broker::online::response::{InvalidResponse, parse_answers};
use ripwire_broker::online::{SemanticStage, prompt};
use serde_json::json;

/// Seeds for past failures go next to this file, named explicitly. Left to guess, proptest looks
/// for a `lib.rs`/`main.rs` above the test and warns in the log when it cannot find one, which is
/// always the case for an integration test target.
fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/props.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    }
}

/// What `remote_text` promises to keep: printable ASCII and spaces, nothing else.
fn is_printable_ascii(s: &str) -> bool {
    s.chars().all(|c| c == ' ' || c.is_ascii_graphic())
}

/// The same reduction `remote_text` applies to the text. A secret only ever reaches the output
/// in this form, so this is the form that must not survive.
fn printable(s: &str) -> String {
    s.chars()
        .filter(|c| *c == ' ' || c.is_ascii_graphic())
        .collect()
}

// ---------------------------------------------------------------- P0.7 — confidentiality

proptest! {
    #![proptest_config(config())]

    /// The shape of the output, for any input at all.
    #[test]
    fn remote_text_is_printable_ascii_and_within_the_cap(
        text in ".{0,400}",
        secret in prop::option::of("\\PC{1,40}"),
        max in 0usize..300,
    ) {
        let out = remote_text(&text, secret.as_deref(), max);
        prop_assert!(is_printable_ascii(&out), "non-printable byte in {:?}", out);
        prop_assert!(out.len() <= max, "{} bytes over a cap of {}", out.len(), max);
    }

    /// The control itself: the credential never reaches the output. Stated over the *printable
    /// form* of the secret, because that is the only form that can appear — the filter runs
    /// before the replacement, so a secret carrying a non-ASCII character used to leave its
    /// ASCII skeleton behind (D-109).
    #[test]
    fn remote_text_never_carries_the_secret(
        // A printable ASCII core, optionally carrying one character the filter removes. That is
        // the class that leaked: the filter erased the character, so `replace` searched for a
        // form that was no longer there. Generating it directly beats generating arbitrary
        // strings and hoping to land on one.
        core in "[!-~]{4,20}",
        noise in prop::option::of(prop::sample::select(vec![
            '\u{a9}', '\u{ae}', '\u{2014}', '\u{7}', '\u{1b}', '\u{fc}',
        ])),
        at in 0usize..24,
        before in ".{0,60}",
        after in ".{0,60}",
        max in 0usize..400,
    ) {
        let mut secret = core.clone();
        if let Some(c) = noise {
            // `core` is ASCII, so any index up to its length is a character boundary.
            secret.insert(at.min(core.len()), c);
        }
        let skeleton = printable(&secret);
        let text = format!("{before}{secret}{after}");
        let out = remote_text(&text, Some(&secret), max);
        prop_assert!(
            !out.contains(&skeleton),
            "the secret survived redaction: secret={:?} skeleton={:?} out={:?}",
            secret, skeleton, out
        );
    }

    /// Truncation is the last step, so a cap can never re-expose what redaction removed.
    #[test]
    fn a_tighter_cap_only_ever_removes(text in ".{0,200}", max in 0usize..200) {
        let wide = remote_text(&text, None, 500);
        let tight = remote_text(&text, None, max);
        prop_assert!(wide.starts_with(&tight), "{:?} is not a prefix of {:?}", tight, wide);
    }
}

// ---------------------------------------------------------------- P0.5 — response validation

/// An `ids` vector of distinct, question-shaped ids.
fn ids(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("q{i}")).collect()
}

proptest! {
    #![proptest_config(config())]

    /// Arbitrary bytes in, no panic, and the contract on the output length.
    #[test]
    fn parse_answers_is_total(body in ".{0,300}", n in 0usize..8) {
        let ids = ids(n);
        match parse_answers("m", &ids, &body) {
            Ok(v) => prop_assert_eq!(v.len(), ids.len()),
            Err(e) => prop_assert!(matches!(
                e,
                InvalidResponse::Malformed | InvalidResponse::WrongModel | InvalidResponse::UnknownQuestion
            )),
        }
    }

    /// A probability outside `[0, 1]`, or not finite, is **unknown** — never clamped into the
    /// range, never read as `false`.
    #[test]
    fn an_impossible_probability_is_unknown_not_clamped(
        // Built out of range rather than filtered into it: an `ANY` strategy plus a
        // `prop_assume!` throws away almost every case and proptest aborts on the rejections,
        // however many successes it already had.
        p in prop_oneof![
            Just(f64::NAN),
            Just(f64::INFINITY),
            Just(f64::NEG_INFINITY),
            1.0000001f64..1e300,
            -1e300..=-1e-300,
        ],
    ) {
        let body = json!({"model": "m", "answers": {"q0": {"type": "noul", "noul": p}}}).to_string();
        let got = parse_answers("m", &ids(1), &body).unwrap();
        prop_assert_eq!(got[0], None, "probability {} was not rejected", p);
    }

    /// And a probability inside the range comes back known, in range, and within one ULP.
    ///
    /// **Not bit-exact, and that is a finding rather than a slack assertion.** `serde_json`
    /// decodes floats approximately unless its `float_roundtrip` feature is on, so
    /// `0.45623431892261956` comes back as `0.4562343189226195` — one ULP down. The drift is in
    /// the JSON decoder, not in the validator, and it only changes a decision for a probability
    /// within one ULP of `ADMISSION` or `SELECTION`; the strict comparisons tolerate that and no
    /// classifier answers at that resolution. Enabling the feature to buy exactness in a path
    /// where one ULP cannot matter was refused. A tolerance of one ULP still catches real
    /// regressions — truncation, rounding to two decimals, reading the wrong field.
    #[test]
    fn a_valid_probability_comes_back_within_one_ulp(p in 0.0f64..=1.0) {
        let body = json!({"model": "m", "answers": {"q0": {"type": "noul", "noul": p}}}).to_string();
        let got = parse_answers("m", &ids(1), &body).unwrap()[0];
        let Some(q) = got else {
            return Err(TestCaseError::fail(format!("a valid probability {p} was read as unknown")));
        };
        prop_assert!((0.0..=1.0).contains(&q), "{} left the range", q);
        let ulps = (q.to_bits() as i64 - p.to_bits() as i64).abs();
        prop_assert!(ulps <= 1, "{} came back as {} ({} ULPs away)", p, q, ulps);
    }

    /// One id that was never asked invalidates the response **as a whole**, not just its own
    /// answer.
    #[test]
    fn one_unasked_id_invalidates_the_whole_response(n in 1usize..6, extra in "[a-z]{1,6}") {
        let ids = ids(n);
        prop_assume!(!ids.contains(&extra));
        let mut answers = serde_json::Map::new();
        for id in &ids {
            answers.insert(id.clone(), json!({"type": "noul", "noul": 0.9}));
        }
        answers.insert(extra, json!({"type": "noul", "noul": 0.9}));
        let body = json!({"model": "m", "answers": answers}).to_string();
        prop_assert_eq!(
            parse_answers("m", &ids, &body),
            Err(InvalidResponse::UnknownQuestion)
        );
    }

    /// Any model other than the pinned one is refused before a single probability is read.
    #[test]
    fn another_model_is_refused(got in "[a-z]{1,8}", want in "[a-z]{1,8}") {
        prop_assume!(got != want);
        let body = json!({"model": got, "answers": {}}).to_string();
        prop_assert_eq!(parse_answers(&want, &ids(0), &body), Err(InvalidResponse::WrongModel));
    }

    /// The answers come back in the order of `ids`, whatever order the response used.
    #[test]
    fn the_order_of_ids_is_the_order_of_answers(n in 1usize..8) {
        let ids = ids(n);
        let mut answers = serde_json::Map::new();
        // Inserted back to front, so a parser that trusted the response's order would show it.
        for (i, id) in ids.iter().enumerate().rev() {
            let p = i as f64 / n as f64;
            answers.insert(id.clone(), json!({"type": "noul", "noul": p}));
        }
        let body = json!({"model": "m", "answers": answers}).to_string();
        let got = parse_answers("m", &ids, &body).unwrap();
        for (i, p) in got.iter().enumerate() {
            prop_assert_eq!(*p, Some(i as f64 / n as f64));
        }
    }
}

// ---------------------------------------------------------------- P0.6 — strict thresholds

/// `Excluded < ReadingLead < Selected`, so monotonicity can be stated as an ordering.
fn rank(d: SourceDecision) -> u8 {
    match d {
        SourceDecision::Excluded => 0,
        SourceDecision::ReadingLead => 1,
        SourceDecision::Selected => 2,
        SourceDecision::Unknown => 0,
    }
}

proptest! {
    #![proptest_config(config())]

    /// The thresholds are strict, and `None` is `Unknown` — never `Rejected`, never zero.
    #[test]
    fn the_thresholds_are_strict_and_unknown_stays_unknown(p in prop::num::f64::ANY) {
        prop_assume!(p.is_finite());
        let want = if p > decision::ADMISSION { FileDecision::Admitted } else { FileDecision::Rejected };
        prop_assert_eq!(decision::admit(Some(p)), want);
        let want = if p > decision::SELECTION {
            SourceDecision::Selected
        } else if p > decision::ADMISSION {
            SourceDecision::ReadingLead
        } else {
            SourceDecision::Excluded
        };
        prop_assert_eq!(decision::select(Some(p)), want);
        prop_assert_eq!(decision::admit(None), FileDecision::Unknown);
        prop_assert_eq!(decision::select(None), SourceDecision::Unknown);
    }

    /// Monotone in `p`: raising a probability never lowers the decision.
    #[test]
    fn raising_the_probability_never_lowers_the_decision(a in 0.0f64..1.0, b in 0.0f64..1.0) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(rank(decision::select(Some(lo))) <= rank(decision::select(Some(hi))));
        prop_assert!(
            decision::admit(Some(lo)) != FileDecision::Admitted
                || decision::admit(Some(hi)) == FileDecision::Admitted
        );
    }

    /// A file keeps its highest known score, and is `Rejected` only when **every** fragment was
    /// evaluated: an unevaluated one may hold the evidence.
    #[test]
    fn a_gap_in_the_fragments_is_unknown_not_rejected(
        fragments in prop::collection::vec(prop::option::of(0.0f64..1.0), 0..6),
    ) {
        let (got, best) = decision::file_decision(&fragments);
        let want_best = fragments.iter().flatten().copied().reduce(f64::max);
        prop_assert_eq!(best, want_best);
        let has_gap = fragments.iter().any(Option::is_none);
        match want_best {
            Some(b) if b > decision::ADMISSION => prop_assert_eq!(got, FileDecision::Admitted),
            Some(_) if has_gap => prop_assert_eq!(got, FileDecision::Unknown),
            Some(_) => prop_assert_eq!(got, FileDecision::Rejected),
            None => prop_assert_eq!(got, FileDecision::Unknown),
        }
    }
}

/// Equal to the threshold does not pass. A random `f64` will not land on `0.25` or `0.50`, so
/// this is the one place a directed case says more than a property.
#[test]
fn a_probability_equal_to_the_threshold_does_not_pass() {
    assert_eq!(
        decision::admit(Some(decision::ADMISSION)),
        FileDecision::Rejected
    );
    assert_eq!(
        decision::select(Some(decision::SELECTION)),
        SourceDecision::ReadingLead
    );
    assert_eq!(
        decision::select(Some(decision::ADMISSION)),
        SourceDecision::Excluded
    );
}

// ---------------------------------------------------------------- P0.8 — cache keys

fn parts<'a>(provider: &'a str, model: &'a str, query: &'a str, hash: &'a str) -> KeyParts<'a> {
    KeyParts {
        provider,
        endpoint: "https://e",
        model,
        stage: SemanticStage::FileAdmission,
        query,
        content_hash: hash,
        range: 0..10,
    }
}

proptest! {
    #![proptest_config(config())]

    /// Changing any one part changes the key.
    #[test]
    fn any_changed_part_changes_the_cache_key(
        a in "\\PC{0,12}", b in "\\PC{0,12}", which in 0usize..4,
    ) {
        prop_assume!(a != b);
        let base = ["p", "m", "q", "h"];
        let k1 = cache::key(&parts(base[0], base[1], base[2], base[3]));
        let mut fields = base;
        fields[which] = &a;
        let k2 = cache::key(&parts(fields[0], fields[1], fields[2], fields[3]));
        fields[which] = &b;
        let k3 = cache::key(&parts(fields[0], fields[1], fields[2], fields[3]));
        prop_assert_ne!(k2, k3, "two values of part {} gave one key", which);
        prop_assert!(k1 != k2 || base[which] == a);
    }

    /// The length prefix exists so that concatenation cannot collide: `("ab", "c")` and
    /// `("a", "bc")` are different questions and must have different keys.
    #[test]
    fn concatenation_cannot_collide_in_the_cache_key(
        // The cut is drawn from the string's own length, so every case is a valid pair of
        // different splits and nothing is rejected.
        (joined, cut) in "[a-z]{3,10}".prop_flat_map(|s| {
            let n = s.len();
            (Just(s), 2usize..n)
        }),
    ) {
        let (a1, b1) = joined.split_at(1);
        let (a2, b2) = joined.split_at(cut);
        prop_assert_ne!(
            cache::key(&parts("p", "m", a1, b1)),
            cache::key(&parts("p", "m", a2, b2)),
            "{:?}+{:?} collided with {:?}+{:?}", a1, b1, a2, b2
        );
    }

    /// The same for the note key, which has its own three parts.
    #[test]
    fn concatenation_cannot_collide_in_the_note_key(
        (joined, cut) in "[a-z]{3,10}".prop_flat_map(|s| {
            let n = s.len();
            (Just(s), 2usize..n)
        }),
    ) {
        let (a1, b1) = joined.split_at(1);
        let (a2, b2) = joined.split_at(cut);
        prop_assert_ne!(notes::key("m", a1, b1), notes::key("m", a2, b2));
    }

    /// Every part is actually in the digest, including the two version constants.
    #[test]
    fn a_changed_note_part_changes_the_note_key(m in "\\PC{0,8}", s in "\\PC{0,8}", e in "\\PC{0,8}") {
        prop_assert_ne!(notes::key(&m, &s, &e), notes::key(&format!("{m}x"), &s, &e));
        prop_assert_ne!(notes::key(&m, &s, &e), notes::key(&m, &format!("{s}x"), &e));
        prop_assert_ne!(notes::key(&m, &s, &e), notes::key(&m, &s, &format!("{e}x")));
    }
}

/// The keys are stable across runs: a digest is a cache key and a recorded fixture, so drifting
/// silently would invalidate every stored answer without anybody noticing.
#[test]
fn the_keys_are_stable_across_runs() {
    assert_eq!(
        notes::key("model-x", "src/a", "evidence"),
        "50b064d8b77cef7ebbb69c44a1813e386aafa5238dce05548a9368f609b8763c"
    );
    assert_eq!(prompt::VERSION, "v1");
    assert_eq!(cache::POLICY_VERSION, "policy/v1");
}

// ---------------------------------------------------------------- P0.9 — note sanitizing

proptest! {
    #![proptest_config(config())]

    /// No control character survives except `\n`, and no escape sequence at all.
    #[test]
    fn a_sanitized_note_has_no_control_or_escape(text in ".{0,800}") {
        let out = notes::sanitize(&text);
        for c in out.chars() {
            prop_assert!(!c.is_control() || c == '\n', "control {:?} survived", c);
        }
        prop_assert!(!out.contains('\u{1b}'), "an escape survived: {:?}", out);
    }

    /// The cap counts **characters**, not bytes.
    #[test]
    fn a_sanitized_note_respects_the_character_cap(text in ".{0,2000}") {
        let out = notes::sanitize(&text);
        prop_assert!(
            out.chars().count() <= notes::MAX_NOTE_CHARS,
            "{} characters over a cap of {}", out.chars().count(), notes::MAX_NOTE_CHARS
        );
    }

    /// Sanitizing twice is sanitizing once: the output is already a fixed point.
    #[test]
    fn sanitizing_is_idempotent(text in ".{0,2000}") {
        let once = notes::sanitize(&text);
        prop_assert_eq!(notes::sanitize(&once), once);
    }

    /// A CSI or OSC sequence goes out whole, taking its parameters with it, rather than leaving
    /// the payload behind as text.
    #[test]
    fn an_escape_sequence_leaves_nothing_behind(
        before in "[a-z]{0,10}", params in "[0-9;]{0,6}", after in "[a-z]{0,10}",
    ) {
        let csi = notes::sanitize(&format!("{before}\u{1b}[{params}m{after}"));
        prop_assert_eq!(csi, format!("{before}{after}").trim().to_string());
        let osc = notes::sanitize(&format!("{before}\u{1b}]0;title\u{7}{after}"));
        prop_assert_eq!(osc, format!("{before}{after}").trim().to_string());
    }
}
