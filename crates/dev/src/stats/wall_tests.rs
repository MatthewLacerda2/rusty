//! A window frame's `wall_ms` is its own (#2255): its wall time is known only when
//! the next frame starts, so it lands on the frame that took it, as the frame log's
//! `ms` does, as the next frame starts, before its work: a read during frame N sees
//! N-1's, and the newest frame recorded has none yet.

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

/// One frame, as the shell runs it: the frame before ends with its wall time
/// (`shell::frame::stats::started`), then a tick of its systems' CPU ms, then the
/// window frame is recorded.
fn run(stats: &mut FrameStats, systems: &[(&str, f64)], frame: &WindowFrame) {
    window_ended(stats, frame.last_wall_ms);
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

/// A frame the window could not show records no entry: the frame before it gets
/// its own time as the undrawn one starts, and the undrawn frame's time, ended as
/// the next drawn frame starts, is dropped rather than put on another frame.
#[test]
fn an_undrawn_frame_s_time_is_dropped() {
    let mut stats = FrameStats::default();
    run(&mut stats, &[], &starts(0.0, 0));
    window_ended(&mut stats, 16.0); // the undrawn frame starts, and records nothing
    run(&mut stats, &[], &starts(500.0, 1));
    let wall = stats.get("wall_ms").unwrap();
    assert_eq!((wall.samples, wall.last), (1, 16.0));
}

/// The frame log's `ms` and `Debug.Stats`' `wall_ms` read the same value for the
/// same `frame`, and a read inside frame N (after it started, before its work)
/// already sees N-1's.
#[test]
fn the_frame_log_and_the_stats_agree_frame_by_frame() {
    let dir = rusty_test_support::temp::dir();
    let session = crate::session::Session::new("").unwrap();
    let game = session.world();
    let mut log = FrameLog::start(&dir, Default::default()).unwrap();
    let stats = game.script_manager().stats_cell();
    let mut inside = Vec::new();
    for (i, ms) in [0.0, 16.0, 60.0, 16.0, 33.0].into_iter().enumerate() {
        let frame = starts(ms, i as u64);
        window_ended(&mut stats.borrow_mut(), frame.last_wall_ms);
        let newest = stats.borrow().window(1);
        inside.push(newest.get("wall_ms").map(|s| s.last));
        run(&mut stats.borrow_mut(), &[], &frame);
        log.frame(game, &frame, false);
    }
    drop(log);
    let none = None;
    assert_eq!(
        inside,
        [none, Some(16.0), Some(60.0), Some(16.0), Some(33.0)]
    );
    // Read inside frame i, the value is frame i - 1's: i starts at 1 here.
    let from_stats: Vec<(u64, f64)> = (inside.iter().enumerate().skip(1))
        .filter_map(|(i, wall)| Some((i as u64 - 1, (*wall)?)))
        .collect();
    let got = read(&dir, &LogQuery::default(), now_ms());
    let from_log: Vec<(u64, f64)> = got
        .records
        .iter()
        .filter_map(|r| Some((r["frame"].as_u64()?, r["ms"].as_f64()?)))
        .collect();
    assert_eq!(from_log, from_stats);
}
