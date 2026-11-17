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

fn scene(frame_ms: f64) -> SceneReport {
    let mut r = SceneReport {
        scene: "pool".into(),
        views: vec!["spawn".into()],
        frames: 300,
        size: (1920, 1080),
        rows: aggregate(&[("spawn".into(), view(&[("frame_ms", frame_ms, frame_ms)]))]),
        ..SceneReport::default()
    };
    r.push_value("load_ms", 120.0);
    r.push_value("rss_peak_mb", 512.5);
    r
}

#[test]
fn the_table_shows_each_rows_change_and_what_the_adapter_lacked() {
    let first = scene(2.0).render(None);
    assert!(
        first.contains("pool — 1 viewpoints x 300 frames at 1920x1080"),
        "{first}"
    );
    assert!(first.contains("no previous report"), "{first}");
    // Lavapipe has no timestamps, and a GL backend no memory report: said, not hidden.
    assert!(first.contains("nothing was rendered"), "{first}");
    assert!(first.contains("no gpu_mb"), "{first}");
    let second = scene(3.0).render(Some(&scene(2.0)));
    let line = second.lines().find(|l| l.starts_with("frame_ms")).unwrap();
    assert!(line.contains("+1.000 (+50.0%)"), "{line}");
    let load = second.lines().find(|l| l.starts_with("load_ms")).unwrap();
    assert!(load.trim_end().ends_with('='), "{load}");
    assert!(!second.contains("no previous report"));
}

#[test]
fn a_report_round_trips_through_its_file() {
    let dir = rusty_test_support::temp::dir().join(format!("scenes_rt_{}", std::process::id()));
    let report = scene(2.0);
    write(&dir, &report, "table").unwrap();
    assert_eq!(read(&dir, "pool"), Some(report));
    assert_eq!(
        std::fs::read_to_string(dir.join("pool.txt")).unwrap(),
        "table"
    );
    assert_eq!(read(&dir, "missing"), None);
    // A malformed file is no last run, not an error.
    std::fs::write(path(&dir, "pool"), "{ truncated").unwrap();
    assert_eq!(read(&dir, "pool"), None);
}

#[test]
fn the_summary_is_one_line_per_scene_with_a_dash_where_unmeasured() {
    let mut other = scene(5.0);
    other.scene = "tower-bar".into();
    let text = summary(&[scene(2.0), other]);
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 4, "{text}");
    assert!(lines[1].contains("frame_ms") && lines[1].contains("gpu_mb"));
    assert!(
        lines[2].starts_with("pool") && lines[2].contains("512.500"),
        "{text}"
    );
    assert!(
        lines[3].starts_with("tower-bar") && lines[3].contains(" 5 "),
        "{text}"
    );
    assert!(lines[3].trim_end().ends_with('-'), "no gpu_mb: {text}");
}
