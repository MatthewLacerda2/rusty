use rusty_core::frame_stats::{is_counter, FrameStats};

use super::*;
use crate::capture::RenderedFrame;

#[test]
fn gpu_rows_are_scaled_by_the_frame_clock_speed() {
    use rusty_render::{GpuPass, GpuTimes};
    let gpu = GpuTimes {
        frame: 1,
        passes: vec![(GpuPass::Shadows, 2.0), (GpuPass::Forward, 6.0)],
        other_ms: 0.4,
        between_submits_ms: 0.6,
        ..Default::default()
    };
    let frame = RenderedFrame {
        gpu: Some(gpu),
        ..RenderedFrame::default()
    };
    let stats = FrameStats::default();
    let mut samples = Samples::default();
    let half = Speeds {
        gpu: Some(0.5),
        ..Speeds::default()
    };
    sample(&mut samples, &stats, Some((&frame, 1.0)), half);
    sample(&mut samples, &stats, Some((&frame, 1.0)), Speeds::default());
    let report = samples.report();
    let avg = |k: &str| report.metrics.iter().find(|(m, _)| m == k).unwrap().1.avg;
    assert_eq!(avg("gpu_ms"), 4.0);
    assert_eq!(avg("gpu.shadows"), 1.0);
    assert_eq!(avg("gpu.forward"), 3.0);
    assert_eq!(avg("gpu.ui"), 0.0);
    assert_eq!(avg("gpu.other"), 0.2);
    assert_eq!(avg("gpu_between_submits_ms"), 0.6, "a wait as measured");
    let rows = report.metrics.iter().filter(|(m, _)| m.starts_with("gpu."));
    let work: f64 = rows.map(|(_, s)| s.avg).sum();
    assert!((work - 4.2).abs() < 1e-9, "gpu. rows {work} vs 4.2");
    assert_eq!(avg("gpu_raw_ms"), 8.0);
    assert_eq!(avg("gpu_clock_pct"), 50.0);
    let mut untimed = Samples::default();
    sample(
        &mut untimed,
        &stats,
        Some((&RenderedFrame::default(), 1.0)),
        half,
    );
    assert!(!untimed
        .report()
        .metrics
        .iter()
        .any(|(m, _)| m.starts_with("gpu")));
}

#[test]
fn cpu_rows_are_scaled_by_the_cpu_clock_speed() {
    let mut stats = FrameStats::default();
    stats.record("frame_ms", 4.0);
    stats.record_system("tick_nav", 2.0);
    let mut samples = Samples::default();
    let cpu = |cpu| Speeds { cpu, gpu: None };
    sample(&mut samples, &stats, None, cpu(Some(0.5)));
    sample(&mut samples, &stats, None, cpu(None));
    assert_eq!(samples.values("frame_ms"), [4.0, 4.0]);
    assert_eq!(samples.values("cpu_ms"), [2.0]);
    assert_eq!(samples.values("cpu.tick_nav"), [1.0]);
    assert_eq!(samples.values("cpu_clock_pct"), [50.0]);
    assert!(samples.values("tick_nav").is_empty(), "no raw system rows");
}

#[test]
fn every_recorded_counter_is_a_row_and_only_the_shown_ones_are_loud() {
    let mut stats = FrameStats::default();
    stats.record("frame_ms", 4.0);
    stats.record("made_up_counter", 3.0);
    stats.record("load_scene_ms", 9.0);
    let mut frame = RenderedFrame::default();
    frame.counters.draw_calls = 7;
    frame.counters.ui_meshed = 1;
    frame.counters.render_extract_us = 250;
    let mut samples = Samples::default();
    sample(&mut samples, &stats, Some((&frame, 1.0)), Speeds::default());
    let report = samples.report();
    let row = |k: &str| report.metrics.iter().find(|(m, _)| m == k).map(|r| r.1);
    assert_eq!(
        row("made_up_counter").map(|s| (s.avg, s.quiet)),
        Some((3.0, true))
    );
    assert_eq!(
        row("ui_meshed").map(|s| (s.avg, s.quiet)),
        Some((1.0, true))
    );
    assert_eq!(
        row("draw_calls").map(|s| (s.avg, s.quiet)),
        Some((7.0, false))
    );
    for timing in ["render_extract_us", "load_scene_ms"] {
        assert!(
            row(timing).is_none(),
            "{timing} is a timing, no counter row"
        );
    }
    let quiet = report.metrics.iter().filter(|(_, s)| s.quiet);
    assert!(quiet.clone().all(|(m, _)| !SHOWN.contains(&m.as_str())));
    let pairs = RenderedFrame::default().counters.pairs();
    let counted = pairs.iter().filter(|(k, _)| is_counter(k)).count();
    // `made_up_counter`, the one tick counter; `frame_ms` and `load_scene_ms` are timings.
    let tick = 1;
    assert_eq!(quiet.count(), counted - SHOWN.len() + tick, "every counter");
}

#[test]
fn a_counter_first_recorded_late_averages_over_the_whole_window() {
    let mut stats = FrameStats::default();
    stats.record("frame_ms", 4.0);
    let mut samples = Samples::default();
    for frame in 0..4 {
        if frame == 2 {
            stats.record("changes_late", 1.0);
        } else if frame == 3 {
            stats.record("changes_late", 0.0);
        }
        sample(&mut samples, &stats, None, Speeds::default());
    }
    assert_eq!(samples.values("changes_late"), [0.0, 0.0, 1.0, 0.0]);
    let report = samples.report();
    let late = report.metrics.iter().find(|(m, _)| m == "changes_late");
    assert_eq!(late.map(|r| r.1.avg), Some(0.25));
}
