use super::*;

fn ui(ms: f64) -> UiTime {
    UiTime {
        edit_ms: ms / 2.0,
        ms,
        speed: Some(1.0),
    }
}

pub(super) fn round(ui_ms: [f64; 2], enter: [f64; 2]) -> EditorRound {
    let mut meter = Meter::new();
    for run in 0..2 {
        let frame = RenderedFrame {
            cpu_ms: 0.5,
            stages: vec![("handles", 0.2)],
            ..RenderedFrame::default()
        };
        let drawn = Drawn {
            frame,
            view: (1306, 963),
        };
        meter.frame(ui(ui_ms[run]), Some(&drawn), None, 1.0);
        meter.frame(ui(ui_ms[run]), None, None, 1.0);
        meter.single("play_enter_ms", enter[run]);
        meter.end_run();
    }
    meter.finish()
}

fn row<'a>(round: &'a EditorRound, metric: &str) -> &'a Row {
    let found = round.rows.iter().find(|r| r.metric == metric);
    found.unwrap_or_else(|| panic!("{metric}: {:?}", round.rows))
}

#[test]
fn the_meter_folds_each_runs_frames_and_singles_into_rows_with_their_spread() {
    let round = round([2.0, 4.0], [100.0, 140.0]);
    let ui_ms = row(&round, "editor.ui_ms");
    assert_eq!((ui_ms.avg, ui_ms.lo, ui_ms.hi), (3.0, 2.0, 4.0));
    assert_eq!(row(&round, "editor.edit_ms").avg, 1.5, "a row of its own");
    assert_eq!(row(&round, "editor.cpu_clock_pct").avg, 100.0);
    assert_eq!(row(&round, "editor.render.handles").avg, 0.2);
    assert_eq!(row(&round, "editor.renderer_ms").avg, 0.5);
    let view = VIEW_ROWS.map(|m| row(&round, m));
    assert_eq!(
        view.map(|r| (r.avg, r.lo, r.hi, r.quiet)),
        [
            (1306.0, 1306.0, 1306.0, false),
            (963.0, 963.0, 963.0, false)
        ],
        "always printed"
    );
    let enter = row(&round, "play_enter_ms");
    assert_eq!((enter.avg, enter.lo, enter.hi), (120.0, 100.0, 140.0));
    assert!(enter.worst.is_empty(), "a single sample has no viewpoint");
    assert!(!round
        .rows
        .iter()
        .any(|r| r.metric.starts_with("editor.gpu")));
    assert!(round.warnings.is_empty(), "{:?}", round.warnings);
}

#[test]
fn a_clock_that_moves_inside_a_run_warns_in_the_rounds_own_words() {
    let mut meter = Meter::new();
    for speed in [0.5, 1.0, 1.5, 2.0] {
        let time = UiTime {
            edit_ms: 0.5,
            ms: 1.0,
            speed: Some(speed),
        };
        meter.frame(time, None, None, 1.0);
    }
    meter.end_run();
    let round = meter.finish();
    assert!(
        !round
            .rows
            .iter()
            .any(|r| VIEW_ROWS.contains(&r.metric.as_str())),
        "no frame drew the Scene view: no size made up"
    );
    let warnings = round.warnings;
    let clock = warnings.iter().find(|w| w.contains("CPU clock moved"));
    let clock = clock.unwrap_or_else(|| panic!("{warnings:?}"));
    assert!(clock.starts_with("editor round: "), "{clock}");
    assert!(
        clock.contains("editor run 1") && clock.contains("read editor.ui_ms"),
        "{clock}"
    );
}

#[test]
fn only_the_first_run_is_a_fresh_process() {
    let mut meter = Meter::new();
    assert!(meter.first_run());
    meter.end_run();
    assert!(!meter.first_run(), "the pages Play used are resident now");
}

#[test]
fn a_round_round_trips_through_its_file() {
    let dir = rusty_test_support::temp::dir().join(format!("editor_rt_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("yard.json");
    let kept = round([1.0, 1.0], [10.0, 10.0]);
    write(&file, &kept).unwrap();
    assert_eq!(read(&file).unwrap(), kept);
    std::fs::write(&file, "{ truncated").unwrap();
    assert!(
        read(&file).is_err(),
        "a malformed round is said, not read as empty"
    );
}

#[test]
fn the_round_carries_the_scene_views_counters() {
    let mut meter = Meter::new();
    let mut counters = rusty_render::RenderCounters::default();
    (
        counters.draw_calls,
        counters.triangles,
        counters.forward_unlit,
    ) = (90, 5000, 42);
    let frame = RenderedFrame {
        counters,
        ..RenderedFrame::default()
    };
    let drawn = Drawn {
        frame,
        view: (8, 8),
    };
    meter.frame(ui(1.0), Some(&drawn), None, 1.0);
    meter.end_run();
    let round = meter.finish();
    assert_eq!(row(&round, "editor.draw_calls").avg, 90.0);
    assert_eq!(row(&round, "editor.triangles").avg, 5000.0);
    assert_eq!(row(&round, "editor.forward_unlit").avg, 42.0);
}
