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
use ripwire_broker::cli::{self, Command};
use ripwire_broker::local;
use ripwire_broker::markup;
use ripwire_broker::memory::identity as memory_identity;
use ripwire_broker::memory::time as memory_time;
use ripwire_broker::model::{Budget, Envelope, Item, Provenance, Role, Source, Status, Untrusted};
use ripwire_broker::notes;
use ripwire_broker::online::cache::{self, KeyParts};
use ripwire_broker::online::decision::{self, FileDecision, SourceDecision};
use ripwire_broker::online::redact::remote_text;
use ripwire_broker::online::response::{InvalidResponse, parse_answers};
use ripwire_broker::online::{
    self,
    request::{self, StateItem},
    retry_after,
};
use ripwire_broker::online::{SemanticStage, prompt};
use serde_json::json;
use std::time::{Duration, SystemTime};

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

// ---------------------------------------------------------------- P0.13 — prompt injection

/// A minimal envelope whose repository-text fields carry `text`. Every field is `pub`, so this
/// needs no constructor in `src/`.
fn envelope_carrying(text: &str) -> Envelope {
    Envelope {
        schema_version: "ripwire-broker.context/v1",
        tool: "context_for_task",
        status: Status::Ready,
        intent: None,
        summary: text.to_string(),
        items: vec![Item {
            kind: "file",
            role: Role::Primary,
            path: text.to_string(),
            line: None,
            symbol: Some(text.to_string()),
            signature: None,
            why_included: text.to_string(),
            source: Source::fact("route"),
            content: Some(Untrusted {
                untrusted_repository_data: text.to_string(),
            }),
            semantic: None,
        }],
        tests: vec![],
        risks: vec![],
        limitations: vec![],
        notes: vec![],
        provenance: Provenance {
            request_id: 1,
            upstream_tools: vec!["route"],
            workspace: text.to_string(),
            ripwire_version: text.to_string(),
            broker_version: "0.1.0",
            online: None,
        },
        budget: Budget {
            requested_tokens: 2500,
            estimated_tokens: 10,
            truncated: false,
            shown: 1,
            omitted: 0,
            next_step: None,
            already_delivered: 0,
        },
    }
}

proptest! {
    #![proptest_config(config())]

    /// Repository text can never close the block that wraps it, whatever it contains — including
    /// the closing tag itself. The payload region carries no `<` and no `>` at all, so there is
    /// nothing to reason about case by case.
    #[test]
    fn repository_text_can_never_close_its_own_block(
        text in prop_oneof![
            Just(local::CONTEXT_CLOSE.to_string()),
            Just(local::CONTEXT_OPEN.to_string()),
            Just("</ripwire-broker-context>".to_string()),
            ".{0,120}",
            "[<>\"\\\\/ a-z-]{0,60}",
        ],
    ) {
        let out = local::wrap("a task", &envelope_carrying(&text));
        prop_assert_eq!(out.matches(local::CONTEXT_OPEN).count(), 1, "{:?}", out);
        prop_assert_eq!(out.matches(local::CONTEXT_CLOSE).count(), 1, "{:?}", out);

        let start = out.find(local::CONTEXT_OPEN).unwrap() + local::CONTEXT_OPEN.len();
        let end = out.rfind(local::CONTEXT_CLOSE).unwrap();
        let payload = &out[start..end];
        prop_assert!(!payload.contains('<'), "a raw < reached the payload: {:?}", payload);
        prop_assert!(!payload.contains('>'), "a raw > reached the payload: {:?}", payload);
    }
}

// ---------------------------------------------------------------- P0.4 — request batching

fn state_item(id: usize, text_len: usize) -> StateItem {
    StateItem {
        id: format!("i{id}"),
        path: format!("src/f{id}.rs"),
        text: "x".repeat(text_len),
    }
}

proptest! {
    #![proptest_config(config())]

    /// Every batch is under every limit, and nothing is lost or invented.
    ///
    /// The order claim needs care: an item that cannot be sent even alone goes to `too_large`
    /// **where it occurs**, so concatenating the batches and then `too_large` does not reproduce
    /// the input — the two lists interleave. What holds, and what is asserted, is that each list
    /// is a subsequence of the input and together they are exactly the input's multiset.
    #[test]
    fn every_batch_is_within_every_limit_and_nothing_is_lost(
        lens in prop::collection::vec(prop_oneof![0usize..200, 3_000usize..6_000], 0..40),
        source_selection in any::<bool>(),
    ) {
        let stage = if source_selection {
            SemanticStage::SourceSelection
        } else {
            SemanticStage::FileAdmission
        };
        let items: Vec<StateItem> =
            lens.iter().enumerate().map(|(i, n)| state_item(i, *n)).collect();
        let (sent, too_large) = request::batches("m", "q", stage, items.clone());

        for batch in &sent {
            let n = batch.questions.0.len();
            prop_assert!((1..=request::MAX_QUESTIONS).contains(&n), "{n} questions");
            let bytes = serde_json::to_string(batch).unwrap().len();
            prop_assert!(bytes <= request::MAX_REQUEST_BYTES, "{bytes} bytes in one request");
            if source_selection {
                prop_assert!(n <= request::MAX_EVIDENCE_UNITS, "{n} evidence units");
                let text: usize = batch.state.items.iter().map(|i| i.text.len()).sum();
                // One item alone is allowed past the text budget: refusing it would drop
                // evidence the request can still carry.
                prop_assert!(
                    n == 1 || text <= request::EVIDENCE_BATCH_BYTES,
                    "{text} bytes of evidence across {n} units"
                );
            }
        }

        let ids = |v: &[StateItem]| v.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
        let batched: Vec<String> =
            sent.iter().flat_map(|b| ids(&b.state.items)).collect();
        let apart = ids(&too_large);
        let mut all = [batched.clone(), apart.clone()].concat();
        all.sort();
        let mut want = ids(&items);
        want.sort();
        prop_assert_eq!(all, want, "an item was lost or invented");

        let input = ids(&items);
        prop_assert!(is_subsequence(&batched, &input), "the batches reordered the input");
        prop_assert!(is_subsequence(&apart, &input), "too_large reordered the input");
    }

    /// `request_bytes` is the length of the JSON `build` produces, not an estimate of it.
    #[test]
    fn request_bytes_is_exactly_the_json_length(
        lens in prop::collection::vec(0usize..300, 0..12),
        source_selection in any::<bool>(),
    ) {
        let stage = if source_selection {
            SemanticStage::SourceSelection
        } else {
            SemanticStage::FileAdmission
        };
        let items: Vec<StateItem> =
            lens.iter().enumerate().map(|(i, n)| state_item(i, *n)).collect();
        let built = serde_json::to_string(&request::build("m", "q", stage, items.clone()))
            .unwrap()
            .len();
        prop_assert_eq!(request::request_bytes("m", "q", stage, &items), built);
    }
}

fn is_subsequence(part: &[String], whole: &[String]) -> bool {
    let mut it = whole.iter();
    part.iter().all(|x| it.any(|y| y == x))
}

// ---------------------------------------------------------------- P0.10 — the command line

proptest! {
    #![proptest_config(config())]

    /// Arbitrary `argv` never panics, and a refusal always tells the caller how to invoke it.
    #[test]
    fn parse_is_total_and_every_refusal_carries_the_usage(
        args in prop::collection::vec(
            prop_oneof![
                "[a-z-]{0,12}",
                "--[a-z-]{0,12}",
                "--[a-z-]{1,12}=[a-z0-9/.]{0,8}",
                Just("serve".to_string()),
                Just("hook".to_string()),
                Just("prompt".to_string()),
                Just("doctor".to_string()),
                Just("--online".to_string()),
                Just("--workspace".to_string()),
                Just("--".to_string()),
                ".{0,6}",
            ],
            0..8,
        ),
    ) {
        match cli::parse(args) {
            Ok(_) => {}
            Err(e) => prop_assert!(
                e.contains(cli::USAGE),
                "a refusal without the usage text: {:?}", e
            ),
        }
    }

    /// The flags of `serve` survive the round trip. `budget_tokens` is deliberately absent:
    /// it is a per-request field on the tool call, not a process flag.
    #[test]
    fn the_serve_flags_survive_a_round_trip(
        rss in 1u64..64_000,
        incremental in any::<bool>(),
        redact in any::<bool>(),
    ) {
        let mut argv = vec![
            "serve".to_string(),
            "--workspace".to_string(),
            ".".to_string(),
            "--ripwire-max-rss-mb".to_string(),
            rss.to_string(),
        ];
        if incremental {
            argv.push("--incremental".to_string());
        }
        if redact {
            argv.push("--redact-workspace".to_string());
        }
        match cli::parse(argv) {
            Ok(Command::Serve(a)) => {
                prop_assert_eq!(a.ripwire_max_rss_mb, Some(rss));
                prop_assert_eq!(a.incremental, incremental);
                prop_assert_eq!(a.redact_workspace, redact);
                prop_assert_eq!(a.workspace, std::path::PathBuf::from("."));
            }
            Ok(_) => prop_assert!(false, "a serve argv parsed as another command"),
            Err(e) => prop_assert!(false, "a valid serve argv was refused: {:?}", e),
        }
    }

    /// `--online` belongs to `serve` alone: a one-shot command that accepted it silently would
    /// promise a remote classifier it never starts.
    #[test]
    fn online_outside_serve_is_refused(sub in prop::sample::select(vec!["doctor", "hook", "prompt"])) {
        let argv: Vec<String> = [sub, "--workspace", ".", "--online"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        prop_assert!(cli::parse(argv).is_err(), "{} accepted --online", sub);
    }

    /// A `--jev-*` flag without `--online` is a refusal, not a silent no-op.
    #[test]
    fn a_jev_flag_without_online_is_refused(value in "[a-z0-9]{1,8}") {
        let argv = vec![
            "serve".to_string(),
            "--workspace".to_string(),
            ".".to_string(),
            "--jev-model".to_string(),
            value,
        ];
        prop_assert!(cli::parse(argv).is_err(), "a --jev flag passed without --online");
    }
}

// ---------------------------------------------------------------- P1.1 — Retry-After

proptest! {
    #![proptest_config(config())]

    /// Delay seconds are read as seconds, whatever the surrounding whitespace.
    #[test]
    fn a_delay_in_seconds_is_read_as_seconds(secs in 0u64..100_000, pad in "[ \\t]{0,3}") {
        let got = retry_after::parse(&format!("{pad}{secs}{pad}"), SystemTime::UNIX_EPOCH);
        prop_assert_eq!(got, Some(Duration::from_secs(secs)));
    }

    /// Anything that is neither digits nor an IMF-fixdate is `None`, and nothing panics.
    #[test]
    fn a_malformed_retry_after_is_none(value in ".{0,40}") {
        let digits = !value.trim().is_empty() && value.trim().bytes().all(|b| b.is_ascii_digit());
        let got = retry_after::parse(&value, SystemTime::UNIX_EPOCH);
        if !digits && got.is_some() {
            // Then it parsed as a date, which must mean it had the fixdate shape.
            prop_assert!(value.contains("GMT"), "{:?} parsed as a date", value);
        }
    }

    /// A date in the past is no wait at all, never a wait into the past.
    #[test]
    fn a_date_in_the_past_is_no_wait(day in 1u32..=28, hour in 0u32..=23) {
        let value = format!("Mon, {day:02} Jan 1994 {hour:02}:00:00 GMT");
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(60 * 365 * 86_400);
        prop_assert_eq!(retry_after::parse(&value, now), Some(Duration::ZERO));
    }

    /// An impossible date is refused, not clamped into a possible one.
    #[test]
    fn an_impossible_date_is_refused(hour in 24u32..99, day in 32u32..99) {
        let bad_hour = format!("Mon, 06 Nov 1994 {hour:02}:00:00 GMT");
        prop_assert_eq!(retry_after::parse(&bad_hour, SystemTime::UNIX_EPOCH), None);
        let bad_day = format!("Mon, {day:02} Nov 1994 08:00:00 GMT");
        prop_assert_eq!(retry_after::parse(&bad_day, SystemTime::UNIX_EPOCH), None);
        prop_assert_eq!(
            retry_after::parse("Mon, 31 Feb 1994 08:00:00 GMT", SystemTime::UNIX_EPOCH),
            None
        );
    }
}

// ---------------------------------------------------------------- P1.2 — candidate ordering

fn item_at(path: &str, role: Role, line: Option<u64>) -> Item {
    Item {
        kind: "file",
        role,
        path: path.to_string(),
        line,
        symbol: None,
        signature: None,
        why_included: String::new(),
        source: Source::fact("route"),
        content: None,
        semantic: None,
    }
}

proptest! {
    #![proptest_config(config())]

    /// Docs are out; paths are distinct; the order is `(best priority, ripwire's order)`; and
    /// each path's lines are sorted and without repetition.
    #[test]
    fn the_candidates_are_ordered_distinct_and_free_of_docs(
        raw in prop::collection::vec(
            (0u8..4, 0usize..5, prop::option::of(1u64..6), any::<bool>()),
            0..25,
        ),
    ) {
        let items: Vec<Item> = raw
            .iter()
            .map(|(_, p, line, doc)| {
                let role = if *doc { Role::Doc } else { Role::Primary };
                item_at(&format!("src/f{p}.rs"), role, *line)
            })
            .collect();
        let pairs: Vec<(u8, &Item)> =
            raw.iter().map(|(pri, ..)| *pri).zip(items.iter()).collect();
        let ranked = online::ranked_paths(pairs);

        let docs: Vec<&str> = items
            .iter()
            .filter(|i| i.role == Role::Doc)
            .map(|i| i.path.as_str())
            .collect();
        for p in &ranked {
            // A path is excluded only when *every* item naming it is a doc.
            let also_source = items
                .iter()
                .any(|i| i.path == p.path && i.role != Role::Doc);
            prop_assert!(also_source || !docs.contains(&p.path.as_str()));
            let mut sorted = p.lines.clone();
            sorted.sort_unstable();
            sorted.dedup();
            prop_assert_eq!(&sorted, &p.lines, "lines out of order or repeated");
        }

        let paths: Vec<&String> = ranked.iter().map(|p| &p.path).collect();
        let mut distinct = paths.clone();
        distinct.sort();
        distinct.dedup();
        prop_assert_eq!(distinct.len(), paths.len(), "a path appears twice");

        for w in ranked.windows(2) {
            prop_assert!(
                (w[0].priority, w[0].rank) <= (w[1].priority, w[1].rank),
                "{:?} before {:?}",
                (w[0].priority, w[0].rank),
                (w[1].priority, w[1].rank)
            );
        }
    }
}

// ---------------------------------------------------------------- P0.12 — the lenient reader

/// How deep the parsed tree goes. Safe to recurse over: `MAX_DEPTH` bounds it.
fn tree_depth(n: &markup::Node) -> usize {
    1 + n.children.iter().map(tree_depth).max().unwrap_or(0)
}

proptest! {
    #![proptest_config(config())]

    /// For **any** input: no panic, no hang, and the tree it returns is bounded in depth.
    /// Historically false — two panics by out-of-bounds and mid-character slicing (D-091), and
    /// stack exhaustion by nesting (D-111).
    #[test]
    fn markup_parse_is_total_and_bounded(
        // Drawn from the alphabet that matters — tag punctuation, quotes, entities, CDATA,
        // multi-byte characters — instead of arbitrary text that would almost never form a tag.
        input in prop::collection::vec(
            prop::sample::select(vec![
                "<", ">", "/", "=", "\"", "'", " ", "a", "ctx", "&lt;", "&amp;", "&", ";",
                "<!--", "-->", "<![CDATA[", "]]>", "\u{e9}", "\u{1f600}", "\n", "\u{0}",
            ]),
            0..60,
        ).prop_map(|v| v.concat()),
    ) {
        if let Some(node) = markup::parse(&input) {
            prop_assert!(
                tree_depth(&node) <= markup::MAX_DEPTH + 1,
                "a tree {} deep came out of a cap of {}",
                tree_depth(&node),
                markup::MAX_DEPTH
            );
        }
    }

    /// And it still reads what it is for: a name, quoted attributes and text come back whole.
    #[test]
    fn a_well_formed_element_round_trips(
        name in "[a-z][a-z_]{0,8}",
        key in "[a-z][a-z_]{0,8}",
        value in "[a-zA-Z0-9 ./_-]{0,20}",
        text in "[a-zA-Z0-9 ./_-]{0,20}",
    ) {
        let doc = format!("<{name} {key}=\"{value}\">{text}</{name}>");
        let node = markup::parse(&doc).expect("a well-formed element must parse");
        prop_assert_eq!(&node.name, &name);
        prop_assert_eq!(node.attr(&key), Some(value.as_str()));
        prop_assert_eq!(&node.text, &text);
    }
}

/// D-111: nesting used to be stack recursion without a bound. 1 000 levels parsed; **10 000
/// aborted the process with SIGABRT**, which `catch_unwind` cannot save — for a long-lived
/// `serve` reading another process's stdout, that is a denial of service.
///
/// Well past the old breaking point, so this test would abort rather than fail if the cap went
/// away. It is the reason the cap is a constant and not a comment.
#[test]
fn nesting_past_the_cap_is_read_as_text_instead_of_exhausting_the_stack() {
    let hostile = "<a>".repeat(50_000);

    let node = markup::parse(&hostile).expect("the outermost element still parses");

    assert!(
        tree_depth(&node) <= markup::MAX_DEPTH + 1,
        "{} levels deep",
        tree_depth(&node)
    );
    // The same for a balanced document, which also has to come back bounded.
    let balanced = format!("{}{}", "<a>".repeat(5_000), "</a>".repeat(5_000));
    let node = markup::parse(&balanced).expect("a balanced document still parses");
    assert!(tree_depth(&node) <= markup::MAX_DEPTH + 1);
}

// ---------------------------------------------------------------- memory identity (PRD jev-mem §5.3)

proptest! {
    #![proptest_config(config())]

    /// Each component is hashed with its length, so moving a boundary is another identity:
    /// `("ab", "c")` and `("a", "bc")` never share a hash.
    #[test]
    fn ids_never_collide_for_ambiguous_tuples(
        a in ".{0,12}", b in ".{0,12}", c in ".{0,12}", d in ".{0,12}",
    ) {
        prop_assume!((a.as_str(), b.as_str()) != (c.as_str(), d.as_str()));
        prop_assert_ne!(
            memory_identity::hash(&[&a, &b]),
            memory_identity::hash(&[&c, &d])
        );
        let joined = format!("{a}{b}");
        prop_assert_ne!(
            memory_identity::hash(&[&joined]),
            memory_identity::hash(&[&joined, ""]),
            "an empty component still counts"
        );
    }
}

proptest! {
    #![proptest_config(config())]

    /// Whatever the wall clock says, each stamp's sequence is above the previous one.
    #[test]
    fn ingest_sequence_is_strictly_monotonic(
        start in 0u64..u64::MAX / 2,
        readings in prop::collection::vec(any::<u64>(), 1..40),
    ) {
        struct Fixed(u64);
        impl memory_time::Clock for Fixed {
            fn now_ms(&self) -> u64 {
                self.0
            }
        }
        let mut seq = memory_time::Sequence::default();
        seq.resume(start);
        let mut last = start;
        for r in readings {
            let s = seq.stamp(&Fixed(r), 1, 0).unwrap();
            prop_assert!(s.ingest_seq > last, "{} after {}", s.ingest_seq, last);
            prop_assert_eq!(s.observed_at_ms, r, "the clock is recorded as read");
            last = s.ingest_seq;
        }
        seq.resume(u64::MAX);
        prop_assert!(seq.stamp(&Fixed(0), 1, 0).is_none(), "never wraps around");
    }
}

proptest! {
    #![proptest_config(config())]

    /// Whatever numbers come back, a decision's probability is unknown or inside `[0, 1]`.
    #[test]
    fn no_parsed_probability_is_ever_outside_the_unit_interval(
        noul in any::<f64>(), a in any::<f64>(), b in any::<f64>(), pick in any::<bool>(),
    ) {
        use ripwire_broker::online::request::{JevQuestion, StateRequest};
        use ripwire_broker::online::response::parse_decisions;
        let req = StateRequest::new(
            "m",
            serde_json::json!({}),
            vec![JevQuestion::noul("n"), JevQuestion::choice("c", &[("x", "x"), ("y", "y")])],
        );
        let body = serde_json::json!({"model": "m", "answers": {
            "q0": {"type": "noul", "noul": noul},
            "q1": {"type": "choice", "choice": if pick { "x" } else { "y" },
                   "probabilities": {"x": a, "y": b}}
        }})
        .to_string();
        if let Ok(decisions) = parse_decisions(&req, &body) {
            for d in decisions {
                if let Some(p) = d.probability() {
                    prop_assert!((0.0..=1.0).contains(&p), "{}", p);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(config())]

    /// An inferred causal edge always points the way its own question asked.
    #[test]
    fn the_two_causal_directions_are_never_confused(caused_by in 0.0f64..=1.0, causes in 0.0f64..=1.0) {
        use ripwire_broker::memory::controller::{self, Config};
        use ripwire_broker::memory::model::{Graph, Record};
        use ripwire_broker::online::response::Decision;
        let rec = |n: u64| -> Record {
            serde_json::from_value(serde_json::json!({
                "schema_version": 1, "policy_version": "memory-policy/v1",
                "node_id": format!("{n:064}"), "content_hash": format!("{n:064}"),
                "workspace_id": "w", "event_key": "e", "kind": "edit_observation", "content": "c",
                "observed_at_ms": n, "ingest_seq": n, "timestamp_role": "observation",
                "expires_at_ms": 9, "generation": 0
            })).unwrap()
        };
        let (new, cand) = (rec(2), rec(1));
        let answers = [
            ("caused_by", Decision::Noul { probability: caused_by }),
            ("causes", Decision::Noul { probability: causes }),
        ].into();
        let cfg = Config { model: "m".into(), candidates: 4 };
        for e in controller::pair_edges(&new, &cand, &answers, &cfg).iter().filter(|e| e.graph == Graph::Causal) {
            match e.source == new.node_id {
                true => prop_assert!(causes >= 0.6 && e.target == cand.node_id),
                false => prop_assert!(caused_by >= 0.6 && e.source == cand.node_id && e.target == new.node_id),
            }
        }
    }
}
