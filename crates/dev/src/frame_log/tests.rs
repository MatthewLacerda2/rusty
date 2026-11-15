//! The frame log read back (#2206): a cut line, the filters, and one frame of a
//! live world written end to end.

use std::io::Write;

use super::*;
use crate::stats::{record_window, WindowFrame};

/// Write `lines` as segment `n` of `dir`.
fn segment(dir: &Path, n: u64, lines: &[&str]) {
    let mut out = std::fs::File::create(writer::segment_path(dir, n)).unwrap();
    for line in lines {
        writeln!(out, "{line}").unwrap();
    }
}

fn ts(read: &LogRead) -> Vec<u64> {
    read.records
        .iter()
        .map(|r| r["t"].as_u64().unwrap())
        .collect()
}

#[test]
fn a_cut_last_line_is_skipped() {
    let dir = rusty_test_support::temp::dir();
    segment(&dir, 0, &[r#"{"ch":"frames","t":1,"ms":16}"#]);
    let mut out = std::fs::File::create(writer::segment_path(&dir, 1)).unwrap();
    let cut = concat!(r#"{"ch":"frames","t":2,"ms":16}"#, "\n", r#"{"ch":"fra"#);
    out.write_all(cut.as_bytes()).unwrap();
    let got = read(&dir, &LogQuery::default(), 10);
    assert_eq!((ts(&got), got.skipped), (vec![1, 2], 1));
}

#[test]
fn a_look_back_stops_at_the_segments_before_it() {
    let dir = rusty_test_support::temp::dir();
    segment(&dir, 0, &["not json at all"]);
    segment(
        &dir,
        1,
        &[r#"{"ch":"frames","t":1000}"#, r#"{"ch":"frames","t":4000}"#],
    );
    segment(
        &dir,
        2,
        &[r#"{"ch":"frames","t":5000}"#, r#"{"ch":"frames","t":6000}"#],
    );
    let q = LogQuery {
        last_seconds: Some(3.5),
        ..LogQuery::default()
    };
    let got = read(&dir, &q, 7000);
    assert_eq!(ts(&got), [4000, 5000, 6000]);
    assert_eq!(got.skipped, 0, "segment 0 was never read");
    let q = LogQuery {
        limit: 2,
        ..LogQuery::default()
    };
    assert_eq!(ts(&read(&dir, &q, 7000)), [5000, 6000], "the newest two");
}

#[test]
fn over_ms_names_the_heaviest_pass_and_system() {
    let dir = rusty_test_support::temp::dir();
    segment(
        &dir,
        0,
        &[
            r#"{"ch":"frames","t":1,"ms":16}"#,
            r#"{"ch":"frames","t":2,"ms":40,"gpu_passes":{"forward":3,"shadow":9},"systems":{"rebake_nav":27.7}}"#,
            r#"{"ch":"console","t":3,"level":"info","msg":"hi"}"#,
        ],
    );
    let q = LogQuery {
        over_ms: Some(33.0),
        ..LogQuery::default()
    };
    let got = read(&dir, &q, 10);
    assert_eq!(ts(&got), [2]);
    assert_eq!(got.records[0]["heaviest_pass"], "shadow");
    assert_eq!(got.records[0]["heaviest_system"], "rebake_nav");
    let console = LogQuery {
        channel: Some("console".into()),
        ..LogQuery::default()
    };
    assert_eq!(ts(&read(&dir, &console, 10)), [3]);
    let passes = LogQuery {
        channel: Some("gpu_passes".into()),
        ..LogQuery::default()
    };
    assert_eq!(
        ts(&read(&dir, &passes, 10)),
        [2],
        "the frame that carries them"
    );
    assert!(
        read(&dir.join("none"), &q, 10).records.is_empty(),
        "no log is no records"
    );
}

/// One frame of a world, written and read back: its place, sim time and heavy
/// system, and the console line beside it; then a channel switched off.
#[test]
fn a_frame_of_a_world_is_written_with_what_was_logged() {
    let dir = rusty_test_support::temp::dir();
    let session = crate::session::Session::new("").unwrap();
    let game = session.world();
    let mut log = FrameLog::start(&dir, LogSettings::default()).unwrap();
    let frame = WindowFrame {
        wall_ms: 16.6,
        fps: Some(60.0),
        render: None,
    };
    let run = |log: &mut FrameLog, msg: &str| {
        let stats = game.script_manager().stats_cell();
        let mut stats = stats.borrow_mut();
        stats.record("frame_ms", 2.0);
        stats.record_system("step_physics", 1.5);
        stats.record_system("tiny", 0.01);
        stats.end_tick();
        record_window(&mut stats, &frame, true);
        drop(stats);
        game.console().borrow_mut().info(msg.into());
        log.frame(game, &frame);
    };
    run(&mut log, "before the first frame: not logged");
    run(&mut log, "hello");
    let off = [("console".to_string(), false)];
    game.resources
        .application
        .borrow_mut()
        .set_log_channels(&off)
        .unwrap();
    run(&mut log, "console is off");
    drop(log);
    let got = read(&dir, &LogQuery::default(), now_ms());
    let chs: Vec<&str> = got
        .records
        .iter()
        .map(|r| r["ch"].as_str().unwrap())
        .collect();
    assert_eq!(chs, ["frames", "frames", "console", "frames"]);
    let f = &got.records[0];
    assert_eq!(
        (f["ms"].as_f64(), f["fps"].as_f64()),
        (Some(16.6), Some(60.0))
    );
    assert_eq!(
        (f["ticks"].as_u64(), f["sim_ms"].as_f64()),
        (Some(1), Some(2.0))
    );
    assert_eq!(f["systems"], serde_json::json!({ "step_physics": 1.5 }));
    assert!(f["cam"].is_array() && f["fwd"].is_array() && f["scene"].is_string());
    assert_eq!(got.records[2]["msg"], "hello");
}

#[test]
fn a_host_off_or_a_budget_of_zero_starts_no_log() {
    let dir = rusty_test_support::temp::dir();
    let none = LogSettings {
        budget_mb: 0,
        ..LogSettings::default()
    };
    assert!(FrameLog::start(&dir, none).is_none());
    assert!(FrameLog::for_host(LogHost::Player, LogSettings::default()).is_none());
}
