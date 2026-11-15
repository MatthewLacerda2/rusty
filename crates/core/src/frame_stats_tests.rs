use super::*;

#[test]
fn series_tracks_last_min_avg_max() {
    let mut s = Series::default();
    for v in [4.0, 2.0, 6.0] {
        s.record(v);
    }
    assert_eq!((s.last, s.min, s.max, s.samples), (6.0, 2.0, 6.0, 3));
    assert!((s.avg - 4.0).abs() < 1e-9);
}

#[test]
fn budget_passes_at_the_limit_and_fails_above_it() {
    let mut stats = FrameStats::default();
    stats.record("draw_calls", 10.0);
    stats.record("draw_calls", 12.0);
    assert_eq!(
        stats.check_budget("draw_calls", 12.0),
        Ok("budget draw_calls <= 12".to_string())
    );
    let err = stats.check_budget("draw_calls", 11.0).unwrap_err();
    assert!(err.contains("worst frame 12.000"), "{err}");
}

#[test]
fn budget_on_an_unmeasured_metric_fails_naming_the_known_ones() {
    let mut stats = FrameStats::default();
    stats.record("entities", 3.0);
    let err = stats.check_budget("draw_calls", 1.0).unwrap_err();
    assert!(
        err.contains("never measured") && err.contains("entities"),
        "{err}"
    );
}

#[test]
fn json_without_timings_drops_ms_metrics_and_systems() {
    let mut stats = FrameStats::default();
    stats.record("entities", 3.0);
    stats.record("update_ms", 0.5);
    stats.record("render_extract_us", 40.0);
    stats.record_system("tick_nav", 0.1);
    let counts = stats.to_json(false);
    assert!(counts["metrics"].get("entities").is_some());
    assert!(counts["metrics"].get("update_ms").is_none());
    assert!(
        counts["metrics"].get("render_extract_us").is_none(),
        "µs are clocks too"
    );
    assert!(counts.get("systems").is_none());
    let full = stats.to_json(true);
    assert!(full["metrics"].get("update_ms").is_some());
    assert!(full["systems"].get("tick_nav").is_some());
}

/// A window summarises each metric's newest samples only, caps the frame counts,
/// and leaves out a metric none of them recorded.
#[test]
fn window_folds_the_newest_samples() {
    let mut stats = FrameStats::default();
    for v in [100.0, 1.0, 2.0, 3.0] {
        stats.record("wall_ms", v);
    }
    stats.record_system("tick_nav", 0.5);
    stats.record_gpu_pass("forward", 4.0);
    (stats.frames, stats.drawn_frames) = (40, 4);
    let w = stats.window(3);
    let s = w.get("wall_ms").unwrap();
    assert_eq!((s.last, s.min, s.max, s.samples), (3.0, 1.0, 3.0, 3));
    assert!(
        (s.avg - 2.0).abs() < 1e-9,
        "the spike before the window is gone"
    );
    assert_eq!((w.frames, w.drawn_frames), (3, 3));
    assert_eq!(w.systems["tick_nav"].samples, 1);
    assert_eq!(w.gpu_passes["forward"].last, 4.0);
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

/// Only the newest [`RECENT`] values are kept: a wider window is the whole ring.
#[test]
fn window_is_clamped_to_the_ring() {
    let mut stats = FrameStats::default();
    for v in 0..RECENT + 10 {
        stats.record("fps", v as f64);
    }
    let s = *stats.window(usize::MAX).get("fps").unwrap();
    assert_eq!((s.samples, s.min), (RECENT as u64, 10.0));
    assert_eq!(stats.get("fps").unwrap().samples, RECENT as u64 + 10);
    assert!(
        stats.to_json(false)["metrics"].get("fps").is_none(),
        "fps is a timing"
    );
}
