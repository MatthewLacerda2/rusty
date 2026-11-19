use super::*;

fn view(rows: &[(&str, f64, f64)]) -> Report {
    Report {
        frames: 3,
        metrics: rows
            .iter()
            .map(|&(k, avg, p95)| (k.to_string(), Summary { avg, p95 }))
            .collect(),
        ..Report::default()
    }
}

#[test]
fn rows_average_the_averages_and_keep_the_worst_p95_and_where() {
    let reports = [
        (
            "camera A".to_string(),
            view(&[("frame_ms", 2.0, 3.0), ("draw_calls", 10.0, 10.0)]),
        ),
        (
            "nav 0 yaw 90".to_string(),
            view(&[("frame_ms", 4.0, 9.0), ("gpu_ms", 1.0, 1.5)]),
        ),
    ];
    let rows = aggregate(&reports);
    let names: Vec<_> = rows.iter().map(|r| r.metric.as_str()).collect();
    assert_eq!(names, ["frame_ms", "draw_calls", "gpu_ms"]);
    assert_eq!((rows[0].avg, rows[0].p95), (3.0, 9.0));
    assert_eq!(rows[0].worst, "nav 0 yaw 90");
    // A row one viewpoint lacks averages over the viewpoints that have it.
    assert_eq!((rows[1].avg, rows[1].worst.as_str()), (10.0, "camera A"));
    assert_eq!(rows[2].avg, 1.0);
}

#[test]
fn a_spread_warns_once_per_kind_naming_the_worst_viewpoint_and_how_many_had_it() {
    let spreads = [
        ("camera A".to_string(), vec![(Spread::Clock, 0.2)]),
        ("spawn".to_string(), vec![]),
        (
            "nav 0 yaw 0".to_string(),
            vec![(Spread::Clock, 0.4), (Spread::Cpu, 0.6)],
        ),
    ];
    let w = spread_warnings(&spreads);
    assert_eq!(w.len(), 2, "{w:?}");
    assert!(w[0].starts_with("the CPU clock moved 40%"), "{}", w[0]);
    assert!(
        w[0].ends_with("(worst at nav 0 yaw 0; 2 of 3 measured windows)"),
        "{}",
        w[0]
    );
    assert!(w[1].starts_with("cpu_ms spread 60%"), "{}", w[1]);
    assert!(
        w[1].ends_with("(worst at nav 0 yaw 0; 1 of 3 measured windows)"),
        "{}",
        w[1]
    );
    assert!(spread_warnings(&spreads[1..2]).is_empty());
}

#[test]
fn runs_fold_into_their_average_with_the_lowest_and_highest_as_the_spread() {
    let run = |frame_ms: f64, p95: f64| {
        aggregate(&[("spawn".into(), view(&[("frame_ms", frame_ms, p95)]))])
    };
    let rows = over_runs(&[run(2.0, 3.0), run(4.0, 5.0), run(3.0, 4.0)]);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!((row.avg, row.lo, row.hi, row.p95), (3.0, 2.0, 4.0, 5.0));
    // One run: no spread.
    let one = over_runs(&[run(2.0, 3.0)]);
    assert_eq!((one[0].lo, one[0].hi), (2.0, 2.0));
}

/// One viewpoint's report over its frames, each frame's Hi-Z parts as given.
fn sampled(frames: &[&[(rusty_render::GpuPart, f64)]]) -> Report {
    use super::super::super::{sample, Samples, Speeds};
    use rusty_render::{GpuPass, GpuTimes};
    let mut samples = Samples::default();
    for parts in frames {
        let gpu = GpuTimes {
            passes: vec![(GpuPass::Occlusion, parts.iter().map(|p| p.1).sum())],
            parts: parts.to_vec(),
            ..GpuTimes::default()
        };
        let frame = crate::capture::RenderedFrame {
            gpu: Some(gpu),
            ..Default::default()
        };
        let speed = Speeds {
            gpu: Some(1.0),
            ..Speeds::default()
        };
        let stats = rusty_core::frame_stats::FrameStats::default();
        sample(&mut samples, &stats, Some(&frame), speed);
    }
    samples.report()
}

/// The cluster cull on one frame of one viewpoint only (#2401): its row is its
/// share of the stage, so the parts still sum to `gpu.occlusion` after the fold.
#[test]
fn parts_on_some_frames_only_still_sum_to_their_stage() {
    use rusty_render::GpuPart::{OcclusionClusters as Clusters, OcclusionTest as Test};
    let culls: &[_] = &[(Test, 1.0), (Clusters, 4.0)];
    let plain: &[_] = &[(Test, 1.0)];
    let reports = [
        ("a".to_string(), sampled(&[culls, plain])),
        ("b".to_string(), sampled(&[plain, plain])),
    ];
    let rows = aggregate(&reports);
    let avg = |k: &str| rows.iter().find(|r| r.metric == k).map(|r| r.avg);
    assert_eq!(avg("gpu.occlusion"), Some(2.0));
    assert_eq!(avg("gpu.occlusion.clusters"), Some(1.0), "its share, not 4");
    let parts: f64 = rusty_render::GpuPart::ALL
        .iter()
        .filter(|p| p.within().is_none())
        .map(|p| avg(&format!("gpu.{}", p.row())).expect("every part has a row"))
        .sum();
    assert_eq!(parts, 2.0, "the parts sum to the stage");
}

/// A CPU part lapped on some frames only (a Mask submitted on one of them), first
/// or last: each part is averaged over its stage's frames, so the parts sum to it.
#[test]
fn cpu_parts_on_some_frames_only_still_sum_to_their_stage() {
    use super::super::super::{sample, Samples, Speeds};
    let viewpoint = |frames: &[&[(&'static str, f64)]]| {
        let mut samples = Samples::default();
        for stages in frames {
            let frame = crate::capture::RenderedFrame {
                cpu_ms: stages.iter().map(|s| s.1).sum(),
                stages: stages.to_vec(),
                ..Default::default()
            };
            let stats = rusty_core::frame_stats::FrameStats::default();
            sample(&mut samples, &stats, Some(&frame), Speeds::default());
        }
        samples.report()
    };
    let masked: &[_] = &[("ui.layout", 1.0), ("ui.mask_submit", 2.0)];
    let plain: &[_] = &[("ui.layout", 1.0)];
    let reports = [
        ("late".to_string(), viewpoint(&[plain, masked])),
        ("early".to_string(), viewpoint(&[masked, plain])),
    ];
    let rows = aggregate(&reports);
    let avg = |k: &str| rows.iter().find(|r| r.metric == k).map(|r| r.avg);
    assert_eq!(avg("render.ui"), Some(2.0));
    assert_eq!(avg("render.ui.mask_submit"), Some(1.0), "its share, not 2");
    assert_eq!(avg("render.ui.layout"), Some(1.0));
}
