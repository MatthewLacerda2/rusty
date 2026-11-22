use super::*;

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
        .any(|l| l.starts_with("ui_laid_out") && l.ends_with("absent on the last run")));
    assert!(text.contains("(1 other counters unchanged since the last run: in bench.json)"));
    let first = now.render(None);
    assert!(!first.contains("ui_meshed") && first.contains("(3 other counters in bench.json"));
}

fn rows(rows: &[(&str, f64, bool)]) -> Vec<(String, Summary)> {
    let row = |&(m, avg, quiet): &(&str, f64, bool)| {
        let s = Summary {
            quiet,
            ..Summary::new(avg, avg)
        };
        (m.to_string(), s)
    };
    rows.iter().map(row).collect()
}

#[test]
fn a_quiet_counter_prints_in_the_window_it_moved_in() {
    use super::super::window::{prefixed, window_of};
    let run = |first: f64, fire: f64| {
        let window = |v| Report {
            frames: 1,
            metrics: rows(&[
                ("draw_calls", 9.0, false),
                ("bones", 3.0, true),
                ("ui_meshed", v, true),
            ]),
            ..Report::default()
        };
        let mut kept = window(first);
        kept.metrics
            .extend(prefixed(window(fire), "firefight").metrics);
        kept.windows.push("firefight".to_string());
        (window(first), window(fire), kept)
    };
    let (.., last) = run(1.0, 1.0);
    let (first, fire, kept) = run(1.0, 0.5);
    let quiet_table = first.render(Some(&last));
    assert!(!quiet_table.contains("ui_meshed"), "{quiet_table}");
    assert!(quiet_table.contains("(2 other counters unchanged since the last run"));
    let fire_table = fire.render(Some(&window_of(&last, "firefight")));
    assert!(
        fire_table.lines().any(|l| l.starts_with("ui_meshed")),
        "{fire_table}"
    );
    assert!(fire_table.contains("(1 other counters unchanged since the last run"));

    let json = serde_json::to_string(&kept).unwrap();
    let back: Report = serde_json::from_str(&json).unwrap();
    let quiet = |m: &str| {
        back.metrics
            .iter()
            .find(|(k, _)| k == m)
            .map(|(_, s)| s.quiet)
    };
    assert_eq!(quiet("firefight.ui_meshed"), Some(true));
    assert_eq!(quiet("firefight.draw_calls"), Some(false));
    assert_eq!(back, kept);

    let never = Report {
        metrics: rows(&[("bones", 3.0, true)]),
        ..Report::default()
    };
    let table = fire.render(Some(&window_of(&never, "firefight")));
    assert!(!table.contains("ui_meshed") && table.contains("each prints once it changes"));
    assert!(table
        .lines()
        .any(|l| l.starts_with("draw_calls") && l.ends_with("absent on the last run")));
}
