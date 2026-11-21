//! `wall_ms` in a scene's report (#2494): its change against the last run, the
//! note saying what it is, its column in the run's summary.

use super::super::super::{Report, Summary};
use super::super::fold::aggregate;
use super::*;

fn scene(wall_ms: f64) -> SceneReport {
    let rows = [
        ("frame_ms", 2.0),
        ("renderer_ms", 1.0),
        ("wall_ms", wall_ms),
    ];
    let view = Report {
        frames: 3,
        metrics: rows
            .iter()
            .map(|&(k, ms)| (k.to_string(), Summary::new(ms, ms)))
            .collect(),
        ..Report::default()
    };
    SceneReport {
        scene: "pool".into(),
        views: vec![ViewKey {
            label: "spawn".into(),
            ..ViewKey::default()
        }],
        runs: 1,
        frames: 300,
        size: (1920, 1080),
        rows: aggregate(&[("spawn".into(), view)]),
        ..SceneReport::default()
    }
}

#[test]
fn a_frame_that_takes_longer_on_the_wall_shows_with_the_other_rows_level() {
    let text = scene(5.0).render(Some(&scene(4.0)));
    let line = |k: &str| text.lines().find(|l| l.starts_with(k)).unwrap();
    assert!(line("wall_ms").contains("+1.000 (+25.0%)"), "{text}");
    assert!(line("renderer_ms").trim_end().ends_with('='), "{text}");
    assert!(
        text.contains("(wall_ms: the frame end to end on the wall"),
        "{text}"
    );
    let summary = summary(&[scene(5.0)]);
    let head = summary.lines().nth(1).unwrap();
    assert!(
        head.contains("frame_ms") && head.contains("wall_ms"),
        "{summary}"
    );
}

#[test]
fn a_cpu_clock_that_moved_names_wall_ms_beside_frame_ms() {
    let mut before = scene(4.0);
    before.push_value("cpu_clock_pct", 80.0);
    let mut now = scene(4.0);
    now.push_value("cpu_clock_pct", 40.0);
    let w = now.against(Some(&before));
    assert!(
        w[0].ends_with("compare cpu_ms, not frame_ms or wall_ms"),
        "{w:?}"
    );
}
