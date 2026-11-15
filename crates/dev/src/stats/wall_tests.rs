//! A window frame's `wall_ms` is its own (#2255): its wall time is known only when
//! the next frame starts, so it lands on the frame that took it, as the frame log's
//! `ms` does, and the newest frame has none yet.

use super::*;
use crate::frame_log::{now_ms, read, FrameLog, LogQuery};

/// The frame starting after one that took `last_wall_ms`, with renderer index `frame`.
fn starts(last_wall_ms: f64, frame: u64) -> WindowFrame {
    WindowFrame {
        last_wall_ms,
        frame,
        ..WindowFrame::default()
    }
}

/// One tick of the frame: its systems' CPU ms, then the window frame.
fn run(stats: &mut FrameStats, systems: &[(&str, f64)], frame: &WindowFrame) {
    for (name, ms) in systems {
        stats.record_system(name, *ms);
    }
    stats.end_tick();
    record_window(stats, frame, true);
}

/// Frames of 16, 60 and 16 ms, the 60 one running a heavy system: the 60 sits on
/// that frame, beside its own heaviest system, and the frames after it are clean.
#[test]
fn a_spike_is_on_the_frame_that_took_it() {
    let mut stats = FrameStats::default();
    let took = [16.0, 60.0, 16.0];
    for (i, ms) in [0.0, 16.0, 60.0, 16.0].into_iter().enumerate() {
        let heavy = if i == 1 { 40.0 } else { 0.5 };
        run(&mut stats, &[("spike", heavy)], &starts(ms, i as u64));
    }
    let wall = stats.get("wall_ms").unwrap();
    assert_eq!(
        (wall.samples, wall.max),
        (3, 60.0),
        "the session: every completed frame"
    );
    let newest = stats.window(1);
    assert!(
        newest.get("wall_ms").is_none(),
        "the newest frame has no wall time yet"
    );
    let after = stats.window(2);
    assert_eq!(
        after.get("wall_ms").unwrap().max,
        took[2],
        "frames 2 and 3: no spike"
    );
    assert_eq!(after.systems["spike"].max, 0.5);
    let with = stats.window(3);
    assert_eq!(with.get("wall_ms").unwrap().max, took[1]);
    assert_eq!(
        with.systems["spike"].max, 40.0,
        "beside its own heaviest system"
    );
}

/// A frame the window could not show ends the frame before it, so the drawn frame
/// after it does not put the undrawn frame's time onto that one.
#[test]
fn an_undrawn_frame_takes_its_own_time_away() {
    let mut stats = FrameStats::default();
    record_window(&mut stats, &starts(0.0, 0), true);
    window_ended(&mut stats, &starts(16.0, 0));
    record_window(&mut stats, &starts(500.0, 1), true);
    let wall = stats.get("wall_ms").unwrap();
    assert_eq!((wall.samples, wall.last), (1, 16.0));
}

/// The frame log's `ms` and `Debug.Stats`' `wall_ms` read the same value for the
/// same `frame`.
#[test]
fn the_frame_log_and_the_stats_agree_frame_by_frame() {
    let dir = rusty_test_support::temp::dir();
    let session = crate::session::Session::new("").unwrap();
    let game = session.world();
    let mut log = FrameLog::start(&dir, Default::default()).unwrap();
    let stats = game.script_manager().stats_cell();
    let mut from_stats = Vec::new();
    for (i, ms) in [0.0, 16.0, 60.0, 16.0, 33.0].into_iter().enumerate() {
        let frame = starts(ms, i as u64);
        run(&mut stats.borrow_mut(), &[], &frame);
        log.frame(game, &frame, false);
        if let Some(wall) = stats.borrow().window(2).get("wall_ms") {
            from_stats.push((i as u64 - 1, wall.last));
        }
    }
    drop(log);
    let got = read(&dir, &LogQuery::default(), now_ms());
    let from_log: Vec<(u64, f64)> = got
        .records
        .iter()
        .filter_map(|r| Some((r["frame"].as_u64()?, r["ms"].as_f64()?)))
        .collect();
    assert_eq!(from_stats, [(0, 16.0), (1, 60.0), (2, 16.0), (3, 33.0)]);
    assert_eq!(from_log, from_stats);
}
