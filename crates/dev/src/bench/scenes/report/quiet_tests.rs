//! A scene's quiet counters (#2449): folded over viewpoints and runs they stay
//! quiet, and the table prints one only when it moved against the last run.

use super::super::super::{Report, Summary};
use super::super::fold::{aggregate, over_runs};
use super::*;

fn scene(meshed: f64, laid_out: Option<f64>) -> SceneReport {
    let row = |m: &str, avg, quiet| {
        (
            m.to_string(),
            Summary {
                avg,
                p95: avg,
                quiet,
            },
        )
    };
    let mut metrics = vec![row("draw_calls", 9.0, false), row("bones", 3.0, true)];
    metrics.push(row("ui_meshed", meshed, true));
    metrics.extend(laid_out.map(|v| row("ui_laid_out", v, true)));
    let view = Report {
        frames: 3,
        metrics,
        ..Report::default()
    };
    let run = aggregate(&[("spawn".into(), view)]);
    SceneReport {
        scene: "pool".into(),
        runs: 1,
        frames: 3,
        rows: over_runs(&[run]),
        ..SceneReport::default()
    }
}

#[test]
fn a_scene_prints_a_quiet_counter_only_when_it_moved() {
    let now = scene(0.5, Some(0.25));
    assert!(
        now.row("bones").is_some_and(|r| r.quiet),
        "the fold keeps it quiet"
    );
    assert!(now.row("draw_calls").is_some_and(|r| !r.quiet));
    let text = now.render(Some(&scene(1.0, None)));
    assert!(
        text.contains("draw_calls") && text.contains("ui_meshed"),
        "{text}"
    );
    assert!(!text.contains("bones"), "{text}");
    let laid = text.lines().find(|l| l.starts_with("ui_laid_out"));
    assert!(
        laid.is_some_and(|l| l.trim_end().ends_with("absent in last")),
        "{text}"
    );
    assert!(text.contains("(1 other counters unchanged since the last run: in pool.json)"));
    let first = now.render(None);
    assert!(!first.contains("ui_meshed") && first.contains("(3 other counters in pool.json"));
}
