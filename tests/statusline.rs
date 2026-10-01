//! The status line (PRD §24): rendering, the `statusline` command and its projection.
use ripwire_broker::statusline::{
    HostInput, Options, model_label, parse_input, render, resolve_root, sanitize, width,
};
use ripwire_broker::statusline_state::*;
use std::io::Write;
use std::path::Path;
use std::process::{Command as Proc, Stdio};

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

fn valid(root: &Path) -> Snapshot {
    Snapshot {
        workspace_key: workspace_key(root),
        ..snap(false, 2, 3, Some(AnalysisStatus::Ready))
    }
}

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
fn an_agent_payload_shows_the_hook_data_and_the_agent_name_after_the_model() {
    let agent =
        r#"{"session_id":"s","agent":{"name":"reviewer"},"model":{"display_name":"Opus 5.5"}}"#;
    let s = snap(false, 7, 18, Some(AnalysisStatus::AttentionRequired));
    assert_eq!(
        render(&host(agent), Some(&s), &WIDE, 1_000),
        "rw-brkr · Opus 5.5 · agente: reviewer · hooks on · última: atenção · inj 7 · não reenviados 18"
    );
    let nameless = r#"{"session_id":"s","agent":{},"model":{"display_name":"Opus 5.5"}}"#;
    assert_eq!(
        render(&host(nameless), None, &WIDE, 1_000),
        "rw-brkr · Opus 5.5 · agente · hooks sem dados"
    );
}

#[test]
fn the_agent_name_is_sanitized_and_cut_to_24_columns() {
    let hostile = r#"{"agent":{"name":"re\u001b[31mviewer"}}"#;
    let line = render(&host(hostile), None, &WIDE, 0);
    assert!(!line.chars().any(|c| c.is_control()), "{line:?}");
    assert!(line.contains("agente: re[31mviewer"), "{line:?}");
    let long = format!(r#"{{"agent":{{"name":"{}"}}}}"#, "x".repeat(60));
    let line = render(&host(&long), None, &WIDE, 0);
    assert!(
        line.contains(&format!("agente: {}", "x".repeat(24))) && !line.contains(&"x".repeat(25)),
        "{line:?}"
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
fn drop_order_detail_before_counter() {
    // Detail dropped, counters kept: Detail segments omitted, others present.
    // Input: snap(false, 7, 18, Ready), SONNET, detail: true, width 90.
    // Without the details the line is 7 + 14 + 7 + 8 + 14 + 5 + 17 = 72 columns of text plus six
    // separators of 3 = 90, so it fits from 90 up and the counters go below that.
    let s = snap(false, 7, 18, Some(AnalysisStatus::Ready));
    let line = render(&host(SONNET), Some(&s), &opts(90), 1_020);
    assert_eq!(
        line,
        "rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks on · última: pronta · inj 7 · não reenviados 18"
    );
}

#[test]
fn drop_order_counter_before_model() {
    // Counter dropped, model kept: Model visible, counters omitted.
    // Input: snap(false, 7, 18, Ready), SONNET, detail: false, width 70.
    // Without the two counters (5 + 17 columns and their separators) the line is 90 - 28 = 62
    // columns: width 70 is inside the range 62..=89 where the counters drop and the model stays.
    let s = snap(false, 7, 18, Some(AnalysisStatus::Ready));
    let no_counter_opts = Options {
        detail: false,
        width: 70,
        color: false,
    };
    let line = render(&host(SONNET), Some(&s), &no_counter_opts, 1_020);
    assert_eq!(
        line,
        "rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks on · última: pronta"
    );
}

#[test]
fn drop_order_model_before_soft_and_d2_rules() {
    // Model dropped, soft kept: Model omitted, soft segments (hooks on, última: pronta) visible.
    // Without the model (14 columns and a separator) the line is 62 - 17 = 45 columns: width 60 is
    // inside the range 45..=61 where the model drops and the soft segments stay.
    let s = snap(false, 7, 18, Some(AnalysisStatus::Ready));
    let line = render(
        &host(SONNET),
        Some(&s),
        &Options {
            detail: false,
            width: 60,
            color: false,
        },
        1_020,
    );
    assert_eq!(line, "rw-brkr · ctx 32% · hooks on · última: pronta");

    // D2 rule: soft drops but essential alerts survive (não é pausa nem alerta, saem).
    // With Error status, última: erro is an alert, not soft.
    // With `hooks on` and no model the line is 7 + 7 + 8 + 12 + 3 separators of 3 = 43 columns; at
    // width 40 `hooks on` drops and the alerts stay: 7 + 7 + 12 + 2 separators = 32 columns, which
    // is what fits in 32..=42.
    let alert_snap = snap(false, 7, 18, Some(AnalysisStatus::Error));
    let alert_line = render(
        &host(SONNET),
        Some(&alert_snap),
        &Options {
            detail: true,
            width: 40,
            color: false,
        },
        1_020,
    );
    assert_eq!(alert_line, "rw-brkr · ctx 32% · última: erro");

    // At width 30, soft also drops with Ready (hooks on and última: pronta are soft): only the
    // prefix and ctx remain, 7 + 3 + 7 = 17 columns, for 17..=44.
    let wide_ready = snap(false, 7, 18, Some(AnalysisStatus::Ready));
    let ready_line = render(
        &host(SONNET),
        Some(&wide_ready),
        &Options {
            detail: true,
            width: 30,
            color: false,
        },
        1_020,
    );
    assert_eq!(ready_line, "rw-brkr · ctx 32%");
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
    // 7 + 3 + 7 + 3 + 9 + 3 + 12 = 44 columns: the essentials exactly.
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
fn tiny_widths_cut_the_prefix_on_a_column() {
    let s = snap(true, 7, 18, Some(AnalysisStatus::Error));
    for (w, expected) in [
        (0, ""),
        (1, "r"),
        (4, "rw-b"),
        (5, "rw-br"),
        (6, "rw-brk"),
        (7, "rw-brkr"),
    ] {
        assert_eq!(
            render(&host(SONNET), Some(&s), &opts(w), 1_020),
            expected,
            "width {w}"
        );
    }
}

#[test]
fn model_labels_are_trimmed_after_the_control_characters_go() {
    assert_eq!(
        model_label(Some("Son \u{7}"), None, None).as_deref(),
        Some("Son")
    );
    assert_eq!(
        model_label(Some("\u{1b} Son"), None, None).as_deref(),
        Some("Son")
    );
    assert_eq!(model_label(Some(" \u{7} "), None, None), None);
}

#[test]
fn invisible_format_characters_are_dropped_like_controls() {
    let hidden = [
        '\u{061C}', '\u{200B}', '\u{200C}', '\u{200D}', '\u{200E}', '\u{200F}', '\u{202A}',
        '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2060}', '\u{2061}', '\u{2062}',
        '\u{2063}', '\u{2064}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}', '\u{FEFF}',
    ];
    for c in hidden {
        assert_eq!(sanitize(&format!("A{c}B")), "AB", "U+{:04X}", c as u32);
    }
    // Neighbours of the ranges, and ordinary non-ASCII text, stay.
    for kept in [
        '\u{200A}', '\u{2010}', '\u{202F}', '\u{205F}', '\u{2065}', 'é', '日',
    ] {
        assert_eq!(
            sanitize(&format!("A{kept}B")),
            format!("A{kept}B"),
            "{kept:?}"
        );
    }
    let line = render(
        &host(r#"{"model":{"display_name":"Op\u202Eus\u200B"}}"#),
        None,
        &WIDE,
        0,
    );
    assert_eq!(line, "rw-brkr · Opus · hooks sem dados");
}

fn detail_line(s: &Snapshot, now: u64) -> String {
    render(&host(SONNET), Some(s), &opts(1_000), now)
}

#[test]
fn token_counts_show_whole_below_a_thousand_and_drop_a_zero_decimal() {
    let at = |tokens: u32| {
        let mut s = snap(false, 1, 1, None);
        s.last_delivery = Some(Delivery {
            at: 1,
            estimated_tokens: tokens,
        });
        detail_line(&s, 1_000)
    };
    for (tokens, shown) in [
        (0, "~0 tok"),
        (999, "~999 tok"),
        (1_000, "~1k tok"),
        (1_240, "~1,2k tok"),
        (10_000, "~10k tok"),
        (10_500, "~10,5k tok"),
    ] {
        assert!(
            at(tokens).contains(&format!("último contexto {shown} ·")),
            "{tokens}: {}",
            at(tokens)
        );
    }
}

#[test]
fn age_counts_seconds_minutes_and_hours_and_a_future_stamp_is_zero() {
    let s = snap(false, 1, 1, None);
    for (now, shown) in [
        (1_000, "há 0s"),
        (1_059, "há 59s"),
        (1_060, "há 1min"),
        (1_000 + 3_599, "há 59min"),
        (1_000 + 3_600, "há 1h"),
        (1_000 + 7_300, "há 2h"),
    ] {
        let line = detail_line(&s, now);
        assert!(line.contains(&format!(" · {shown}")), "{now}: {line}");
    }
    // `updated_at` ahead of the clock (a skew, or a hand-written file): no panic, no "old" mark.
    let future = detail_line(&s, 500);
    assert!(
        future.ends_with("· há 0s") && !future.contains("dados antigos"),
        "{future}"
    );
}

#[test]
fn counters_near_the_limit_do_not_overflow_the_reuse_rate() {
    let mut s = snap(false, 1, u64::MAX, None);
    s.stats.delivered = u64::MAX;
    let line = detail_line(&s, 1_000);
    assert!(line.contains("reuso 100%"), "{line}");
    s.stats.delivered = 0;
    assert!(detail_line(&s, 1_000).contains("reuso 100%"));
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
        .replace("\x1b[38;5;117m", "")
        .replace("\x1b[31m", "")
        .replace("\x1b[0m", "");
    assert_eq!(stripped, plain);
}

#[test]
fn hooks_off_is_red_and_hooks_on_light_blue_when_colored() {
    let colored = Options {
        detail: false,
        width: 200,
        color: true,
    };
    let off = render(&host(SONNET), Some(&snap(true, 1, 1, None)), &colored, 0);
    assert!(off.contains("\x1b[31mhooks off\x1b[0m"), "{off:?}");
    let on = render(&host(SONNET), Some(&snap(false, 1, 1, None)), &colored, 0);
    assert!(on.contains("\x1b[38;5;117mhooks on\x1b[0m"), "{on:?}");
    // `hooks sem dados` is not a hook state the session chose: it stays plain.
    let none = render(&host(SONNET), None, &colored, 0);
    assert!(none.contains(" · hooks sem dados"), "{none:?}");
    assert!(!none.contains("\x1b[38;5;117m"), "{none:?}");
}

#[test]
fn a_published_snapshot_reads_back_and_files_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/repo/a");
    publish(state.path(), "s-1", root, &valid(root)).unwrap();
    assert_eq!(
        read(state.path(), HOST, "s-1", root),
        Read::Valid(valid(root))
    );
    let file = path(state.path(), HOST, "s-1", root);
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(file.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        !text.contains("s-1") && !text.contains("/repo/a"),
        "no plain id or path: {text}"
    );
}

#[test]
fn sessions_hosts_and_workspaces_do_not_share_a_projection() {
    let state = tempfile::tempdir().unwrap();
    let (a, b) = (Path::new("/repo/a"), Path::new("/repo/a-worktree"));
    publish(state.path(), "s-1", a, &valid(a)).unwrap();
    assert_eq!(
        read(state.path(), HOST, "s-2", a),
        Read::Missing,
        "another window"
    );
    assert_eq!(
        read(state.path(), HOST, "s-1", b),
        Read::Missing,
        "another worktree"
    );
    assert_eq!(
        read(state.path(), "codex", "s-1", a),
        Read::Missing,
        "another host"
    );
    assert_ne!(
        path(state.path(), "a", "bc", a),
        path(state.path(), "ab", "c", a)
    );
}

#[test]
fn hostile_session_ids_stay_inside_the_directory() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    for id in ["../../etc/passwd", "a/b", &"x".repeat(10_000)] {
        let p = path(state.path(), HOST, id, root);
        assert_eq!(p.parent().unwrap(), state.path().join("statusline"), "{id}");
        publish(state.path(), id, root, &valid(root)).unwrap();
    }
}

#[test]
fn absence_corruption_and_size_are_told_apart() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    assert_eq!(read(state.path(), HOST, "s", root), Read::Missing);
    publish(state.path(), "s", root, &valid(root)).unwrap();
    let file = path(state.path(), HOST, "s", root);
    std::fs::write(&file, "{not json").unwrap();
    assert_eq!(read(state.path(), HOST, "s", root), Read::Corrupt);
    std::fs::write(&file, " ".repeat(MAX_SNAPSHOT_BYTES as usize + 1)).unwrap();
    assert_eq!(
        read(state.path(), HOST, "s", root),
        Read::Corrupt,
        "over the limit"
    );
}

#[test]
fn a_newer_schema_reads_as_incompatible() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    publish(state.path(), "s", root, &valid(root)).unwrap();
    let file = path(state.path(), HOST, "s", root);
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    v["schema_version"] = 2.into();
    v["stats"] = "reshaped".into();
    std::fs::write(&file, v.to_string()).unwrap();
    assert_eq!(read(state.path(), HOST, "s", root), Read::Incompatible);
}

#[test]
fn a_snapshot_for_another_workspace_key_is_incompatible() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    let wrong = Snapshot {
        workspace_key: workspace_key(Path::new("/other")),
        ..valid(root)
    };
    publish(state.path(), "s", root, &wrong).unwrap();
    assert_eq!(read(state.path(), HOST, "s", root), Read::Incompatible);
}

#[test]
fn symlinks_and_directories_read_as_missing() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    let file = path(state.path(), HOST, "s", root);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let target = state.path().join("elsewhere.json");
    std::fs::write(&target, serde_json::to_string(&valid(root)).unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &file).unwrap();
    assert_eq!(read(state.path(), HOST, "s", root), Read::Missing);
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert_eq!(read(state.path(), HOST, "s", root), Read::Missing);
}

#[test]
fn reading_creates_nothing() {
    let state = tempfile::tempdir().unwrap();
    let missing = state.path().join("never");
    assert_eq!(read(&missing, HOST, "s", Path::new("/r")), Read::Missing);
    assert!(!missing.exists());
    // A state dir that exists but has no `statusline/` yet stays as it is.
    assert_eq!(
        read(state.path(), HOST, "s", Path::new("/r")),
        Read::Missing
    );
    assert_eq!(std::fs::read_dir(state.path()).unwrap().count(), 0);
}

#[test]
fn a_fifo_reads_as_missing_without_blocking() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    let file = path(state.path(), HOST, "s", root);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    assert!(Proc::new("mkfifo").arg(&file).status().unwrap().success());
    let (tx, rx) = std::sync::mpsc::channel();
    let dir = state.path().to_path_buf();
    // Detached on purpose: a read that blocks on the FIFO must fail the test, not hang it.
    std::thread::spawn(move || {
        let _ = tx.send(read(&dir, HOST, "s", Path::new("/r")));
    });
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(10))
            .expect("read blocked on a FIFO"),
        Read::Missing
    );
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn writers_of_one_file_do_not_share_a_temporary() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    let dir = state.path().to_path_buf();
    let writers: Vec<_> = (0..8)
        .map(|t| {
            let dir = dir.clone();
            std::thread::spawn(move || {
                for i in 0..50 {
                    let mut s = valid(Path::new("/r"));
                    s.stats.injections = t * 100 + i;
                    publish(&dir, "s", Path::new("/r"), &s).expect("a concurrent write failed");
                }
            })
        })
        .collect();
    for w in writers {
        w.join().unwrap();
    }
    assert!(matches!(
        read(state.path(), HOST, "s", root),
        Read::Valid(_)
    ));
    assert_eq!(
        entries(&state.path().join("statusline")).len(),
        1,
        "no temporary is left behind"
    );
}

#[test]
fn a_failed_write_removes_its_temporary() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    let file = path(state.path(), HOST, "s", root);
    // A non-empty directory where the file goes: the rename cannot succeed.
    std::fs::create_dir_all(file.join("inside")).unwrap();
    assert!(publish(state.path(), "s", root, &valid(root)).is_err());
    assert_eq!(
        entries(file.parent().unwrap()),
        vec![file.file_name().unwrap().to_string_lossy().into_owned()],
        "only the directory that was already there"
    );
}

#[test]
fn a_symlink_planted_at_a_temporary_name_is_never_written_through() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    let file = path(state.path(), HOST, "s", root);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let victim = state.path().join("victim");
    std::fs::write(&victim, "precious").unwrap();
    // The temporary is `<file>.tmp<pid>-<n>` with a process-wide counter: plant every name this
    // process can reach in this test binary.
    for n in 0..5_000 {
        let name = file.with_extension(format!("tmp{}-{n}", std::process::id()));
        std::os::unix::fs::symlink(&victim, name).unwrap();
    }
    let _ = publish(state.path(), "s", root, &valid(root));
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "precious");
}

#[test]
fn a_looser_existing_directory_is_tightened_and_a_shared_one_is_left_alone() {
    use std::os::unix::fs::PermissionsExt;
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o7777;
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    let sub = state.path().join("statusline");
    std::fs::create_dir(&sub).unwrap();
    for d in [state.path(), sub.as_path()] {
        std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    publish(state.path(), "s", root, &valid(root)).unwrap();
    assert_eq!(mode(state.path()), 0o700);
    assert_eq!(mode(&sub), 0o700);
    // A sticky, world-writable directory (a shared `/tmp`) is not ours to restrict.
    let shared = tempfile::tempdir().unwrap();
    std::fs::set_permissions(shared.path(), std::fs::Permissions::from_mode(0o1777)).unwrap();
    publish(shared.path(), "s", root, &valid(root)).unwrap();
    assert_eq!(mode(shared.path()), 0o1777);
    assert_eq!(mode(&shared.path().join("statusline")), 0o700);
}

#[test]
fn the_session_store_tightens_its_directory_too() {
    use ripwire_broker::hook::SessionState;
    use ripwire_broker::state::StateStore;
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let store = StateStore::new(dir.path().to_path_buf());
    store.save("s", &SessionState::default()).unwrap();
    assert_eq!(
        std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(store.load("s"), SessionState::default());
}

#[test]
fn concurrent_readers_see_whole_versions_only() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/r");
    publish(state.path(), "s", root, &valid(root)).unwrap();
    let dir = state.path().to_path_buf();
    let writer = std::thread::spawn(move || {
        for i in 0..300 {
            let mut s = valid(Path::new("/r"));
            s.stats.injections = i;
            publish(&dir, "s", Path::new("/r"), &s).unwrap();
        }
    });
    for _ in 0..300 {
        assert!(matches!(
            read(state.path(), HOST, "s", root),
            Read::Valid(_)
        ));
    }
    writer.join().unwrap();
}

// Binary-level tests: Command::Statusline dispatch (Task 5).
fn run_bar(args: &[&str], stdin: &[u8], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut cmd = Proc::new(env!("CARGO_BIN_EXE_ripwire-broker"));
    cmd.arg("statusline")
        .args(args)
        .env_remove("COLUMNS")
        .env_remove("NO_COLOR")
        .env_remove("XDG_STATE_HOME");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child.stdin.take().unwrap().write_all(stdin);
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn the_manual_example_of_the_spec() {
    let ws = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let input = format!(
        r#"{{"session_id":"demo","workspace":{{"project_dir":"{}"}},"model":{{"display_name":"Sonnet","id":"claude-sonnet-4-6"}},"effort":{{"level":"high"}},"context_window":{{"used_percentage":32}}}}"#,
        ws.path().display()
    );
    let (code, out, err) = run_bar(
        &[
            "--workspace",
            ws.path().to_str().unwrap(),
            "--state-dir",
            state.path().to_str().unwrap(),
        ],
        input.as_bytes(),
        &[],
    );
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        "rw-brkr · Sonnet 4.6 hig · ctx 32% · hooks sem dados\n"
    );
    assert!(err.is_empty(), "{err}");
    let (_, colored, _) = run_bar(
        &[
            "--workspace",
            ws.path().to_str().unwrap(),
            "--state-dir",
            state.path().to_str().unwrap(),
            "--color",
            "always",
        ],
        input.as_bytes(),
        &[("NO_COLOR", "1")],
    );
    assert!(
        colored.contains("\x1b[38;5;250mctx 32%\x1b[0m"),
        "{colored:?}"
    );
}

#[test]
fn it_reads_the_projection_of_this_session_and_workspace() {
    let ws = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();
    publish(
        state.path(),
        "s-1",
        &root,
        &Snapshot {
            workspace_key: workspace_key(&root),
            ..snap(false, 7, 18, Some(AnalysisStatus::AttentionRequired))
        },
    )
    .unwrap();
    let input = r#"{"session_id":"s-1","model":{"display_name":"Opus 5.5"}}"#;
    let (_, out, _) = run_bar(
        &[
            "--workspace",
            ws.path().to_str().unwrap(),
            "--state-dir",
            state.path().to_str().unwrap(),
        ],
        input.as_bytes(),
        &[],
    );
    assert_eq!(
        out.trim_end(),
        "rw-brkr · Opus 5.5 · hooks on · última: atenção · inj 7 · não reenviados 18"
    );
    // An agent payload is the main session run with `--agent` (D6 revised, D-123): same data, one
    // more segment.
    let agent_input = r#"{"session_id":"s-1","agent":{"name":"re\u001b[31mviewer"},"model":{"display_name":"Opus 5.5"}}"#;
    let (_, agent_out, _) = run_bar(
        &[
            "--workspace",
            ws.path().to_str().unwrap(),
            "--state-dir",
            state.path().to_str().unwrap(),
        ],
        agent_input.as_bytes(),
        &[],
    );
    assert_eq!(
        agent_out.trim_end(),
        "rw-brkr · Opus 5.5 · agente: re[31mviewer · hooks on · última: atenção · inj 7 · não reenviados 18"
    );
}

#[test]
fn without_a_session_id_nothing_is_read() {
    let ws = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();
    publish(
        state.path(),
        "default",
        &root,
        &Snapshot {
            workspace_key: workspace_key(&root),
            ..snap(false, 7, 18, None)
        },
    )
    .unwrap();
    let (_, out, _) = run_bar(
        &[
            "--workspace",
            ws.path().to_str().unwrap(),
            "--state-dir",
            state.path().to_str().unwrap(),
        ],
        br#"{"model":{"display_name":"Opus"}}"#,
        &[],
    );
    assert!(out.contains("hooks sem dados"), "{out}");
}

#[test]
fn bad_stdin_degrades_and_still_exits_zero() {
    let state = tempfile::tempdir().unwrap();
    let sd = state.path().to_str().unwrap();
    let huge = vec![b' '; 256 * 1024 + 1];
    for stdin in [&b""[..], b"{", b"\xff\xfe", &huge[..]] {
        let (code, out, err) = run_bar(&["--state-dir", sd], stdin, &[]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, "rw-brkr · hooks sem dados\n");
        assert!(err.len() < 200, "no long logs: {err}");
    }

    // Stdin size limit is enforced: valid JSON padded past MAX_STDIN_BYTES is rejected.
    let base = br#"{"model":{"display_name":"X"}}"#;
    let padding_needed = ripwire_broker::statusline::MAX_STDIN_BYTES as usize + 1 - base.len();
    let mut oversized = base.to_vec();
    oversized.extend(vec![b' '; padding_needed]);
    let (code, out, err) = run_bar(&["--state-dir", sd], &oversized, &[]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out, "rw-brkr · hooks sem dados\n",
        "model must not appear when over limit"
    );
    assert!(err.len() < 200, "no long logs: {err}");

    // At exactly the limit, the input is accepted.
    let mut at_limit = base.to_vec();
    at_limit.extend(vec![b' '; padding_needed - 1]);
    let (code, out, err) = run_bar(&["--state-dir", sd], &at_limit, &[]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out, "rw-brkr · X · hooks sem dados\n",
        "model appears at limit"
    );
    assert!(err.len() < 200, "no long logs: {err}");
}

#[test]
fn width_comes_from_the_flag_then_columns_then_100() {
    let input = br#"{"model":{"display_name":"Sonnet"},"context_window":{"used_percentage":32}}"#;
    let (_, by_flag, _) = run_bar(&["--width", "20"], input, &[("COLUMNS", "200")]);
    assert_eq!(by_flag.trim_end(), "rw-brkr · ctx 32%");
    let (_, by_env, _) = run_bar(&[], input, &[("COLUMNS", "20")]);
    assert_eq!(by_env.trim_end(), "rw-brkr · ctx 32%");
}

#[test]
fn the_workspace_order_is_flag_project_dir_current_dir_cwd() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let input = parse_input(&format!(
        r#"{{"workspace":{{"project_dir":"{}","current_dir":"{}"}},"cwd":"{}"}}"#,
        a.path().display(),
        b.path().display(),
        b.path().display()
    ));
    assert_eq!(
        resolve_root(Some(b.path()), &input),
        Some(b.path().canonicalize().unwrap())
    );
    assert_eq!(
        resolve_root(None, &input),
        Some(a.path().canonicalize().unwrap())
    );
    assert_eq!(
        resolve_root(Some(Path::new("/does/not/exist")), &input),
        None,
        "no silent fallback"
    );
}

#[test]
fn the_status_line_never_starts_ripwire() {
    let bin = tempfile::tempdir().unwrap();
    let marker = bin.path().join("ran");
    let fake = bin.path().join("ripwire");
    std::fs::write(&fake, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let ws = tempfile::tempdir().unwrap();
    let path = format!("{}:/usr/bin:/bin", bin.path().display());
    let (code, _, _) = run_bar(
        &["--workspace", ws.path().to_str().unwrap()],
        br#"{"session_id":"s"}"#,
        &[("PATH", &path), ("HOME", ws.path().to_str().unwrap())],
    );
    assert_eq!(code, 0);
    assert!(!marker.exists(), "statusline must not start ripwire");
    assert_eq!(
        std::fs::read_dir(ws.path()).unwrap().count(),
        0,
        "and must not create files"
    );
}

#[test]
fn the_status_line_creates_and_changes_nothing_with_existing_state_either() {
    let bin = tempfile::tempdir().unwrap();
    let marker = bin.path().join("ran");
    let fake = bin.path().join("ripwire");
    std::fs::write(&fake, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let ws = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();
    publish(state.path(), "s", &root, &valid(&root)).unwrap();
    // The state as the hooks left it: a projection, a lock and a session file beside it.
    std::fs::write(state.path().join("abc.json"), "{}").unwrap();
    std::fs::write(state.path().join("abc.lock"), "").unwrap();
    let tree = |dir: &Path| {
        let mut out = vec![];
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                let m = std::fs::symlink_metadata(&p).unwrap();
                if m.is_dir() {
                    stack.push(p.clone());
                }
                out.push((p, m.len(), m.modified().unwrap()));
            }
        }
        out.sort();
        out
    };
    let before = (tree(state.path()), tree(ws.path()));
    let path = format!("{}:/usr/bin:/bin", bin.path().display());
    let input = format!(r#"{{"session_id":"s","cwd":"{}"}}"#, root.display());
    // Both a session that has a projection and one that has none.
    for session in ["s", "other"] {
        let input = input.replace(r#""s""#, &format!("\"{session}\""));
        let (code, _, _) = run_bar(
            &[
                "--workspace",
                ws.path().to_str().unwrap(),
                "--state-dir",
                state.path().to_str().unwrap(),
            ],
            input.as_bytes(),
            &[("PATH", &path), ("HOME", ws.path().to_str().unwrap())],
        );
        assert_eq!(code, 0);
    }
    assert!(!marker.exists(), "statusline must not start ripwire");
    assert_eq!(
        before,
        (tree(state.path()), tree(ws.path())),
        "no file created, removed, resized or touched"
    );
}

#[test]
fn a_closed_stdout_never_makes_the_bar_fail() {
    let mut child = Proc::new(env!("CARGO_BIN_EXE_ripwire-broker"))
        .arg("statusline")
        .env_remove("XDG_STATE_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The reader goes away before the bar has anything to write: its write gets EPIPE.
    drop(child.stdout.take());
    let mut stdin = child.stdin.take().unwrap();
    let _ = stdin.write_all(b"{}");
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_zero_width_heavy_agent_name_is_bounded_and_fast() {
    let name = format!("a{}", "\u{301}".repeat(120_000));
    let json = format!(r#"{{"agent":{{"name":"{name}"}}}}"#);
    let line = render(&host(&json), None, &WIDE, 0);
    assert!(line.len() < 300, "{} bytes", line.len());
    assert!(line.contains("agente: a"), "{line:?}");
}

#[test]
fn a_cut_after_a_space_leaves_no_trailing_space() {
    // 23 letters, a space, then more: the 24-column cut lands just after the space.
    let json = format!(r#"{{"agent":{{"name":"{} tail"}}}}"#, "x".repeat(23));
    let line = render(&host(&json), None, &WIDE, 0);
    assert_eq!(
        line,
        format!("rw-brkr · agente: {} · hooks sem dados", "x".repeat(23))
    );
}

#[test]
fn the_agent_segment_is_soft_and_goes_before_the_alert_and_ctx() {
    let json = r#"{"session_id":"s","agent":{"name":"reviewer"},"model":{"display_name":"Opus 5.5"},"context_window":{"used_percentage":12}}"#;
    let s = snap(false, 7, 18, Some(AnalysisStatus::AttentionRequired));
    let at = |w| render(&host(json), Some(&s), &opts(w), 1_000);
    assert_eq!(at(40), "rw-brkr · ctx 12% · última: atenção");
    assert_eq!(
        at(80),
        "rw-brkr · Opus 5.5 · agente: reviewer · ctx 12% · hooks on · última: atenção"
    );
}
