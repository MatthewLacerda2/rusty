use super::*;

fn frames(stats: &mut FrameStats, values: &[f64], in_session: bool, forward: bool) {
    for v in values {
        stats.record_frame(in_session, |f| {
            f.record("wall_ms", *v);
            if forward {
                f.record_gpu_pass("forward", 4.0);
                f.rendered();
            }
        });
    }
}

#[test]
fn window_folds_the_newest_samples() {
    let mut stats = FrameStats::default();
    stats.record_system("tick_nav", 0.5);
    stats.end_tick();
    frames(&mut stats, &[100.0, 1.0, 2.0, 3.0], true, true);
    let w = stats.window(3);
    let s = w.get("wall_ms").unwrap();
    assert_eq!((s.last, s.min, s.max, s.samples), (3.0, 1.0, 3.0, 3));
    assert!(
        (s.avg - 2.0).abs() < 1e-9,
        "the spike before the window is gone"
    );
    assert_eq!(
        (w.frames, w.drawn_frames),
        (0, 3),
        "the tick is 4 frames old"
    );
    assert!(w.systems.is_empty(), "so is its system's sample");
    assert_eq!(stats.window(4).systems["tick_nav"].samples, 1);
    assert_eq!(w.gpu_passes["forward"].samples, 3);
    assert_eq!(
        stats.get("wall_ms").unwrap().max,
        100.0,
        "the session is kept"
    );
    assert!(
        stats.window(0).metrics.is_empty(),
        "an empty window holds nothing"
    );
}

#[test]
fn a_metric_that_stopped_leaves_the_window() {
    let mut stats = FrameStats::default();
    frames(&mut stats, &[1.0; 5], true, true);
    frames(&mut stats, &[1.0; 3], true, false);
    assert!(
        stats.window(3).gpu_passes.is_empty(),
        "forward stopped 3 frames ago"
    );
    assert_eq!(stats.window(4).gpu_passes["forward"].samples, 1);
    assert_eq!(stats.window(3).drawn_frames, 0);
    stats.record("load_ms", 9.0);
    stats.end_tick();
    for _ in 0..3 {
        stats.end_tick();
    }
    assert!(
        stats.window(3).get("load_ms").is_none(),
        "a load 4 ticks ago"
    );
}

#[test]
fn a_frame_after_stop_moves_the_window_not_the_session() {
    let mut stats = FrameStats::default();
    stats.record("fixed_update_ms", 0.2);
    stats.end_tick();
    frames(&mut stats, &[16.0], true, true);
    frames(&mut stats, &[500.0, 500.0], false, true);
    let session = stats.get("wall_ms").unwrap();
    assert_eq!((session.samples, session.max), (1, 16.0));
    assert_eq!((stats.frames, stats.drawn_frames), (1, 1));
    let w = stats.window(2);
    assert_eq!(w.get("wall_ms").map(|s| s.avg), Some(500.0));
    assert_eq!(w.drawn_frames, 2);
    assert!(
        w.get("fixed_update_ms").is_none(),
        "no tick in the last 2 frames"
    );
    assert!(stats.window(3).get("fixed_update_ms").is_some());
}

#[test]
fn window_is_clamped_to_the_ring() {
    let mut stats = FrameStats::default();
    for v in 0..RECENT + 10 {
        stats.record("fps", v as f64);
        stats.end_tick();
    }
    let s = *stats.window(usize::MAX).get("fps").unwrap();
    assert_eq!((s.samples, s.min), (RECENT as u64, 10.0));
    assert_eq!(stats.window(usize::MAX).frames, RECENT as u64);
    assert_eq!(stats.get("fps").unwrap().samples, RECENT as u64 + 10);
    let counts = stats.to_json(false);
    assert!(counts["metrics"].get("fps").is_none(), "fps is a timing");
    assert_eq!(
        counts["drawn_frames"], 0,
        "results.json carries drawn_frames"
    );
}
