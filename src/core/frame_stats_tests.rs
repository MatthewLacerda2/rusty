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
    stats.record_system("tick_nav", 0.1);
    let counts = stats.to_json(false);
    assert!(counts["metrics"].get("entities").is_some());
    assert!(counts["metrics"].get("update_ms").is_none());
    assert!(counts.get("systems").is_none());
    let full = stats.to_json(true);
    assert!(full["metrics"].get("update_ms").is_some());
    assert!(full["systems"].get("tick_nav").is_some());
}
