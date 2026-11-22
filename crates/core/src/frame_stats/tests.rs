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

#[test]
fn last_frame_holds_all_its_ticks() {
    let mut stats = FrameStats::default();
    stats.record("frame_ms", 9.0);
    stats.end_tick();
    stats.record_frame(true, |f| f.record("wall_ms", 16.0));
    for ms in [2.0, 3.0] {
        stats.record("frame_ms", ms);
        stats.record_system("step_physics", ms);
        stats.end_tick();
    }
    stats.record_frame(true, |f| f.record("wall_ms", 17.0));
    let last = stats.last_frame();
    let sim = last.get("frame_ms").unwrap();
    assert_eq!(
        (last.frames, sim.samples),
        (2, 2),
        "both ticks, not the older one"
    );
    assert!((sim.avg * sim.samples as f64 - 5.0).abs() < 1e-9);
    assert_eq!(last.systems["step_physics"].samples, 2);
    assert_eq!(last.get("wall_ms").unwrap().last, 17.0);
    assert_eq!(
        stats.window(1).frames,
        1,
        "a window of one keeps the newest tick only"
    );
    stats.record_frame(false, |_| {});
    assert!(
        stats.last_frame().get("frame_ms").is_none(),
        "an edit frame ticked nothing"
    );
}

#[test]
fn an_ended_value_lands_on_its_own_frame_once() {
    let mut stats = FrameStats::default();
    stats.frame_ended("wall_ms", 1.0);
    stats.record_frame(false, |_| {});
    stats.record_frame(true, |_| {});
    assert!(stats.window(1).get("wall_ms").is_none());
    stats.frame_ended("wall_ms", 16.0);
    stats.frame_ended("wall_ms", 99.0);
    assert_eq!(
        stats.get("wall_ms").map(|s| (s.samples, s.last)),
        Some((1, 16.0))
    );
    let edit = stats.window(2).get("wall_ms").copied();
    assert_eq!(edit.map(|s| s.samples), Some(1), "the edit frame had none");
    stats.record_frame(false, |_| {});
    stats.frame_ended("wall_ms", 5.0);
    assert_eq!(
        stats.get("wall_ms").unwrap().samples,
        1,
        "an edit frame stays out"
    );
    assert_eq!(stats.window(1).get("wall_ms").unwrap().last, 5.0);
}

#[test]
fn tick_systems_hold_the_newest_tick_summed_by_name() {
    let mut stats = FrameStats::default();
    stats.record_system("old", 9.0);
    stats.end_tick();
    for (name, ms) in [("b", 1.0), ("a", 2.0), ("b", 0.5)] {
        stats.record_system(name, ms);
    }
    stats.end_tick();
    let got = &stats.tick_systems;
    assert_eq!(*got, [("b", 1.5), ("a", 2.0)]);
}

#[test]
fn every_key_is_a_timing_or_a_counter_never_both() {
    let counters = ["draw_calls", "ui_meshed", "texture_bytes", "changes_mesh"];
    let timings = ["frame_ms", "render_extract_us", "fps", "load_scene_ms"];
    for key in counters.iter().chain(&timings) {
        assert_ne!(is_timing(key), is_counter(key), "{key} in one class");
    }
    assert!(counters.iter().all(|k| is_counter(k)));
}
