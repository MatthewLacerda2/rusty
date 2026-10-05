use super::*;

#[test]
fn the_summary_is_the_mean_and_the_nearest_rank_p95() {
    let values: Vec<f64> = (1..=100).map(f64::from).collect();
    assert_eq!(
        Summary::of(&values),
        Summary {
            avg: 50.5,
            p95: 95.0
        }
    );
    // Order does not matter, and one frame is its own p95.
    assert_eq!(Summary::of(&[3.0, 1.0, 2.0]).p95, 3.0);
    assert_eq!(Summary::of(&[7.0]), Summary { avg: 7.0, p95: 7.0 });
    assert_eq!(Summary::of(&[]), Summary::default());
}

#[test]
fn samples_report_in_first_seen_order() {
    let mut s = Samples::default();
    for frame in 0..4 {
        s.push("frame_ms", f64::from(frame));
        s.push("draw_calls", 10.0);
        s.end_frame();
    }
    let report = s.report();
    assert_eq!(report.frames, 4);
    let names: Vec<_> = report.metrics.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(names, ["frame_ms", "draw_calls"]);
    assert_eq!(report.metrics[0].1.avg, 1.5);
}

#[test]
fn the_table_shows_the_change_against_the_last_run() {
    let run = |ms: f64| Report {
        frames: 10,
        metrics: vec![
            ("gpu_ms".to_string(), Summary { avg: ms, p95: ms }),
            (
                "draw_calls".to_string(),
                Summary {
                    avg: 120.0,
                    p95: 120.0,
                },
            ),
        ],
    };
    let first = run(4.0).render(None);
    assert!(first.contains("no previous report"), "{first}");
    let second = run(3.0).render(Some(&run(4.0)));
    let gpu = second.lines().find(|l| l.starts_with("gpu_ms")).unwrap();
    assert!(gpu.contains("-1.000 (-25.0%)"), "{gpu}");
    let draws = second
        .lines()
        .find(|l| l.starts_with("draw_calls"))
        .unwrap();
    assert!(draws.trim_end().ends_with('='), "{draws}");
    // A metric the last run never measured shows no change.
    let only_now = Report {
        frames: 1,
        metrics: vec![("tick_nav".to_string(), Summary::of(&[0.5]))],
    };
    let line = only_now.render(Some(&run(4.0)));
    assert!(line
        .lines()
        .any(|l| l.starts_with("tick_nav") && l.trim_end().ends_with("0.500")));
}

#[test]
fn a_report_round_trips_through_json() {
    let report = Report {
        frames: 2,
        metrics: vec![(
            "frame_ms".to_string(),
            Summary {
                avg: 1.25,
                p95: 2.0,
            },
        )],
    };
    let json = serde_json::to_string(&report).unwrap();
    assert_eq!(serde_json::from_str::<Report>(&json).unwrap(), report);
}
