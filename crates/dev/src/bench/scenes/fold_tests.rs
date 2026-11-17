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
        w[0].ends_with("(worst at nav 0 yaw 0; 2 of 3 viewpoints)"),
        "{}",
        w[0]
    );
    assert!(w[1].starts_with("cpu_ms spread 60%"), "{}", w[1]);
    assert!(
        w[1].ends_with("(worst at nav 0 yaw 0; 1 of 3 viewpoints)"),
        "{}",
        w[1]
    );
    assert!(spread_warnings(&spreads[1..2]).is_empty());
}
