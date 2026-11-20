//! The quiet counters' rows in the printed report (#2449).

use super::*;

/// A quiet counter prints only when it moved against the last run or that run
/// lacks it (#2449); the rest are counted in one line. With no last run none
/// prints.
#[test]
fn a_quiet_counter_prints_only_when_it_changed() {
    let report = |rows: &[(&str, f64, bool)]| Report {
        frames: 1,
        metrics: rows
            .iter()
            .map(|&(m, avg, quiet)| {
                (
                    m.to_string(),
                    Summary {
                        avg,
                        p95: avg,
                        quiet,
                    },
                )
            })
            .collect(),
        ..Report::default()
    };
    let last = report(&[
        ("draw_calls", 9.0, false),
        ("bones", 3.0, true),
        ("ui_meshed", 1.0, true),
    ]);
    let now = report(&[
        ("draw_calls", 9.0, false),
        ("bones", 3.0, true),
        ("ui_meshed", 0.5, true),
        ("ui_laid_out", 0.25, true),
    ]);
    let text = now.render(Some(&last));
    assert!(
        text.contains("draw_calls"),
        "a shown counter prints unchanged"
    );
    assert!(
        text.contains("ui_meshed") && !text.contains("bones"),
        "{text}"
    );
    assert!(text
        .lines()
        .any(|l| l.starts_with("ui_laid_out") && l.ends_with("absent in last")));
    assert!(text.contains("(1 other counters unchanged since the last run: in bench.json)"));
    let first = now.render(None);
    assert!(!first.contains("ui_meshed") && first.contains("(3 other counters in bench.json"));
}
