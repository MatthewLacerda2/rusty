use super::super::super::{Report, Summary};
use super::super::fold::aggregate;
use super::*;

fn view(rows: &[(&str, f64, f64)]) -> Report {
    Report {
        frames: 3,
        metrics: rows
            .iter()
            .map(|&(k, avg, p95)| (k.to_string(), Summary::new(avg, p95)))
            .collect(),
        ..Report::default()
    }
}

fn key(label: &str) -> ViewKey {
    ViewKey {
        label: label.into(),
        ..ViewKey::default()
    }
}

fn scene(frame_ms: f64) -> SceneReport {
    let mut r = SceneReport {
        scene: "pool".into(),
        views: vec![key("spawn")],
        runs: 1,
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
        first.contains("pool — 1 viewpoints x 300 frames x 1 runs at 1920x1080"),
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
    assert!(lines[1]
        .trim_start_matches("scene")
        .trim_start()
        .starts_with("cpu_ms"));
    assert!(
        lines[3].starts_with("tower-bar") && lines[3].contains(" 5 "),
        "{text}"
    );
    assert!(lines[3].trim_end().ends_with('-'), "no gpu_mb: {text}");
    // `IMPORT=1`: the project's one import_ms, after the scenes.
    let mut first = scene(2.0);
    first.push_value("import_ms", 840.0);
    let text = summary(&[first, scene(3.0)]);
    let last = text.lines().last().unwrap_or_default();
    assert!(
        last.starts_with("import_ms 840") && last.ends_with("loading pool)"),
        "{text}"
    );
}

#[test]
fn a_last_run_of_another_shape_is_said_once_and_gets_no_change_column() {
    let mut now = scene(3.0);
    now.size = (1280, 720);
    let before = scene(2.0);
    let w = now.against(Some(&before));
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("size 1920x1080 -> 1280x720"), "{}", w[0]);
    let text = now.render(Some(&before));
    let line = text.lines().find(|l| l.starts_with("frame_ms")).unwrap();
    assert!(
        !line.contains('%'),
        "no change against another shape: {line}"
    );
    assert!(!text.contains("no previous report"), "{text}");
    now.size = before.size;
    now.views.push(key("nav 0 yaw 0"));
    assert!(now.against(Some(&before))[0].contains("viewpoints 1 -> 2"));
}

#[test]
fn a_cpu_clock_that_moved_since_the_last_run_is_said_with_the_clock_notes() {
    let mut before = scene(2.0);
    before.push_value("cpu_clock_pct", 80.0);
    let mut now = scene(2.0);
    now.push_value("cpu_clock_pct", 40.0);
    now.push_value("cpu_ms", 1.0);
    let w = now.against(Some(&before));
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].starts_with("the CPU clock moved -50%"), "{}", w[0]);
    assert!(
        scene(2.0).against(Some(&before)).is_empty(),
        "no clock now: no drift"
    );
    let text = now.render(Some(&before));
    assert!(
        text.contains("(cpu_ms, cpu.<system>: ms at the reference CPU clock"),
        "{text}"
    );
}
