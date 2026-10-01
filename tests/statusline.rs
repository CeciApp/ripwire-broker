//! The status line (PRD §24): rendering, the `statusline` command and its projection.
use ripwire_broker::statusline::{HostInput, Options, model_label, parse_input, render, width};
use ripwire_broker::statusline_state::*;

const WIDE: Options = Options {
    detail: false,
    width: 1000,
    color: false,
};

fn host(json: &str) -> HostInput {
    parse_input(json)
}

fn snap(opted_out: bool, injections: u64, hits: u64, last: Option<AnalysisStatus>) -> Snapshot {
    Snapshot {
        schema_version: SCHEMA_VERSION,
        host: "claude-code".into(),
        workspace_key: "k".into(),
        updated_at: 1_000,
        opted_out,
        stats: VisibleStats {
            events: 3,
            injections,
            delivered: 25,
            session_hits: hits,
        },
        last_analysis: last.map(|status| Analysis {
            at: 990,
            event: "Stop".into(),
            status,
            error_kind: (status == AnalysisStatus::Error).then(|| "upstream_unavailable".into()),
        }),
        last_delivery: Some(Delivery {
            at: 970,
            estimated_tokens: 1240,
        }),
    }
}

const SONNET: &str = r#"{"session_id":"s","model":{"display_name":"Sonnet","id":"claude-sonnet-4-6"},
  "effort":{"level":"high"},"context_window":{"used_percentage":32}}"#;

#[test]
fn the_spec_examples_render_as_written() {
    let s = snap(false, 7, 18, Some(AnalysisStatus::AttentionRequired));
    assert_eq!(
        render(&host(SONNET), Some(&s), &WIDE, 1_000),
        "rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks on · última: atenção · inj 7 · não reenviados 18"
    );
    let off = snap(true, 7, 18, None);
    assert_eq!(
        render(&host(SONNET), Some(&off), &WIDE, 1_000),
        "rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks off · inj 7 · não reenviados 18"
    );
    assert_eq!(
        render(&host(SONNET), None, &WIDE, 1_000),
        "rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks sem dados"
    );
    let err = snap(false, 0, 0, Some(AnalysisStatus::Error));
    assert!(render(&host(SONNET), Some(&err), &WIDE, 1_000).contains("última: erro"));
}

#[test]
fn status_words_map_one_to_one() {
    for (status, word) in [
        (AnalysisStatus::Ready, "última: pronta"),
        (AnalysisStatus::AttentionRequired, "última: atenção"),
        (AnalysisStatus::Unknown, "última: incerta"),
        (AnalysisStatus::Error, "última: erro"),
    ] {
        let line = render(
            &host(SONNET),
            Some(&snap(false, 1, 1, Some(status))),
            &WIDE,
            1_000,
        );
        assert!(line.contains(word), "{line}");
    }
}

#[test]
fn model_versions_come_from_the_name_or_an_unambiguous_id() {
    assert_eq!(
        model_label(Some("Sonnet"), Some("claude-sonnet-4-6"), None).as_deref(),
        Some("Sonnet 4.6")
    );
    assert_eq!(
        model_label(Some("Opus 4.6"), Some("claude-opus-4-6"), None).as_deref(),
        Some("Opus 4.6")
    );
    assert_eq!(
        model_label(Some("Haiku"), Some("claude-haiku-4-5-20251001"), None).as_deref(),
        Some("Haiku 4.5")
    );
    // A date is not a version; an alias or another family gives no version.
    assert_eq!(
        model_label(Some("Haiku"), Some("claude-haiku-20251001"), None).as_deref(),
        Some("Haiku")
    );
    assert_eq!(
        model_label(Some("Sonnet"), Some("sonnet"), None).as_deref(),
        Some("Sonnet")
    );
    assert_eq!(
        model_label(Some("Sonnet"), Some("claude-opus-4-6"), None).as_deref(),
        Some("Sonnet")
    );
    assert_eq!(
        model_label(Some("Sonnet"), None, None).as_deref(),
        Some("Sonnet")
    );
    assert_eq!(
        model_label(None, Some("claude-sonnet-4-6"), Some("high")),
        None
    );
}

#[test]
fn effort_has_five_labels_and_nothing_else() {
    for (level, label) in [
        ("low", "low"),
        ("medium", "mid"),
        ("high", "hig"),
        ("xhigh", "xtr"),
        ("max", "max"),
    ] {
        assert_eq!(
            model_label(Some("Opus 5.5"), None, Some(level)).unwrap(),
            format!("Opus 5.5 {label}")
        );
    }
    for odd in ["", "HIGH", "ultra"] {
        assert_eq!(
            model_label(Some("Opus 5.5"), None, Some(odd)).unwrap(),
            "Opus 5.5"
        );
    }
}

#[test]
fn zero_context_shows_and_missing_or_out_of_range_does_not() {
    let line = |pct: &str| {
        render(
            &host(&format!(
                r#"{{"model":{{"display_name":"Opus"}},"context_window":{{"used_percentage":{pct}}}}}"#
            )),
            None,
            &WIDE,
            0,
        )
    };
    assert!(line("0").contains("ctx 0%"));
    assert!(
        line("32.5").contains("ctx 33%"),
        "rounded: {}",
        line("32.5")
    );
    for absent in ["null", "-1", "100.6", "\"32\""] {
        assert!(!line(absent).contains("ctx"), "{absent}: {}", line(absent));
    }
}

#[test]
fn host_fields_with_the_wrong_type_are_omitted() {
    let line = render(
        &host(r#"{"model":"Sonnet","effort":3,"context_window":[]}"#),
        None,
        &WIDE,
        0,
    );
    assert_eq!(line, "rw-brkr · hooks sem dados");
}

#[test]
fn invalid_or_empty_input_degrades_to_the_prefix() {
    for text in ["", "{", "[]", "null", "\u{0}"] {
        assert_eq!(
            render(&host(text), None, &WIDE, 0),
            "rw-brkr · hooks sem dados",
            "{text:?}"
        );
    }
}

#[test]
fn external_text_cannot_reach_the_terminal_as_control() {
    let line = render(
        &host(r#"{"model":{"display_name":"Son\u001b[31mnet\nX\u0007"}}"#),
        None,
        &WIDE,
        0,
    );
    assert!(!line.chars().any(|c| c.is_control()), "{line:?}");
    assert!(line.contains("Son[31mnetX"), "{line:?}");
}

#[test]
fn an_agent_payload_shows_host_segments_only() {
    let agent =
        r#"{"session_id":"s","agent":{"name":"reviewer"},"model":{"display_name":"Opus 5.5"}}"#;
    let s = snap(false, 7, 18, Some(AnalysisStatus::AttentionRequired));
    assert_eq!(
        render(&host(agent), Some(&s), &WIDE, 1_000),
        "rw-brkr · Opus 5.5 · agente"
    );
}

#[test]
fn detail_adds_delivered_reuse_last_context_and_age() {
    let opts = Options {
        detail: true,
        ..WIDE
    };
    let s = snap(false, 7, 18, Some(AnalysisStatus::Ready));
    let line = render(&host(SONNET), Some(&s), &opts, 1_020);
    assert!(
        line.ends_with("· entregues 25 · reuso 42% · último contexto ~1,2k tok · há 20s"),
        "{line}"
    );
    let old = render(&host(SONNET), Some(&s), &opts, 1_000 + 301);
    assert!(old.contains("dados antigos"), "{old}");
    let mut none = s.clone();
    none.stats.delivered = 0;
    none.stats.session_hits = 0;
    none.last_delivery = None;
    let line = render(&host(SONNET), Some(&none), &opts, 1_020);
    assert!(
        !line.contains("reuso") && !line.contains("contexto ~"),
        "no fictitious rate: {line}"
    );
}

fn opts(w: usize) -> Options {
    Options {
        detail: true,
        width: w,
        color: false,
    }
}

#[test]
fn lines_fit_40_80_and_120_columns_and_shed_in_order() {
    let s = snap(true, 7, 18, Some(AnalysisStatus::AttentionRequired));
    for w in [40, 80, 120] {
        let line = render(&host(SONNET), Some(&s), &opts(w), 1_020);
        assert!(width(&line) <= w, "{w}: {line}");
        assert!(line.starts_with("rw-brkr"), "{line}");
        assert!(
            line.contains("hooks off") && line.contains("última: atenção"),
            "alerts stay: {line}"
        );
    }
    let line40 = render(&host(SONNET), Some(&s), &opts(40), 1_020);
    assert!(
        !line40.contains("entregues") && !line40.contains("inj "),
        "details and counters go first: {line40}"
    );
}

#[test]
fn extreme_widths_keep_alerts_then_the_prefix() {
    let s = snap(true, 7, 18, Some(AnalysisStatus::Error));
    // 7 + 3 + 7 + 3 + 7 + 3 + 9 + 3 + 12 = 44 columns: the essentials exactly.
    let line = render(&host(SONNET), Some(&s), &opts(44), 1_020);
    assert_eq!(line, "rw-brkr · ctx 32% · hooks off · última: erro");
    assert_eq!(
        render(&host(SONNET), Some(&s), &opts(43), 1_020),
        "rw-brkr · hooks off · última: erro"
    );
    assert_eq!(
        render(&host(SONNET), Some(&s), &opts(34), 1_020),
        "rw-brkr · hooks off · última: erro"
    );
    assert_eq!(render(&host(SONNET), Some(&s), &opts(10), 1_020), "rw-brkr");
    assert_eq!(render(&host(SONNET), Some(&s), &opts(3), 1_020), "rw-");
}

#[test]
fn width_is_visual_not_bytes() {
    assert_eq!(width("não"), 3);
    assert_eq!(width("日本"), 4);
    let wide =
        r#"{"model":{"display_name":"モデル名前テスト"},"context_window":{"used_percentage":5}}"#;
    let line = render(
        &host(wide),
        None,
        &Options {
            detail: false,
            width: 30,
            color: false,
        },
        0,
    );
    assert!(width(&line) <= 30, "{line}");
}

fn ctx_line(pct: f64) -> String {
    render(
        &host(&format!(
            r#"{{"context_window":{{"used_percentage":{pct}}}}}"#
        )),
        None,
        &Options {
            detail: false,
            width: 100,
            color: true,
        },
        0,
    )
}

#[test]
fn ctx_colors_follow_the_rounded_value_and_reset_after_the_segment() {
    for (pct, code) in [
        (0.0, "\x1b[38;5;250m"),
        (39.0, "\x1b[38;5;250m"),
        (39.4, "\x1b[38;5;250m"),
        (39.5, "\x1b[97m"),
        (40.0, "\x1b[97m"),
        (59.0, "\x1b[97m"),
        (60.0, "\x1b[33m"),
        (80.0, "\x1b[33m"),
        (80.4, "\x1b[33m"),
        (80.5, "\x1b[31m"),
        (81.0, "\x1b[31m"),
        (100.0, "\x1b[31m"),
    ] {
        let line = ctx_line(pct);
        let shown = pct.round() as u32;
        assert!(
            line.contains(&format!("{code}ctx {shown}%\x1b[0m")),
            "{pct}: {line:?}"
        );
    }
}

#[test]
fn color_never_has_no_escape_and_alerts_are_colored_when_asked() {
    let s = snap(false, 1, 1, Some(AnalysisStatus::Error));
    let plain = render(
        &host(SONNET),
        Some(&s),
        &Options {
            detail: false,
            width: 200,
            color: false,
        },
        0,
    );
    assert!(!plain.contains('\x1b'), "{plain:?}");
    let colored = render(
        &host(SONNET),
        Some(&s),
        &Options {
            detail: false,
            width: 200,
            color: true,
        },
        0,
    );
    assert!(
        colored.contains("\x1b[31múltima: erro\x1b[0m"),
        "{colored:?}"
    );
    // The fit is computed without escapes: same visible text either way.
    let stripped = colored
        .replace("\x1b[38;5;250m", "")
        .replace("\x1b[31m", "")
        .replace("\x1b[0m", "");
    assert_eq!(stripped, plain);
}
