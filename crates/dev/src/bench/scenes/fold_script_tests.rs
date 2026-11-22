use super::super::super::{Report, Summary};
use super::*;

fn view(rows: &[(&str, f64)]) -> Report {
    let metrics = rows
        .iter()
        .map(|&(k, avg)| (k.to_string(), Summary::new(avg, avg)))
        .collect();
    Report {
        frames: 3,
        metrics,
        ..Report::default()
    }
}

fn avg(rows: &[Row], metric: &str) -> f64 {
    rows.iter()
        .find(|r| r.metric == metric)
        .map_or(f64::NAN, |r| r.avg)
}

#[test]
fn a_verb_absent_from_the_first_viewpoints_averages_over_all_of_them() {
    let update = ("cpu.update_scripts", 1.0);
    let mut views: Vec<(String, Report)> = (0..3)
        .map(|n| (format!("view {n}"), view(&[update])))
        .collect();
    let late = view(&[update, ("script.A.f", 0.4), ("script.A.f.calls", 4.0)]);
    views.push(("view 3".into(), late));
    let rows = aggregate(&views);
    assert_eq!(avg(&rows, "cpu.update_scripts"), 1.0);
    assert!(
        (avg(&rows, "script.A.f") - 0.1).abs() < 1e-12,
        "a quarter of 0.4"
    );
    assert!((avg(&rows, "script.A.f.calls") - 1.0).abs() < 1e-12);
}

#[test]
fn a_verb_one_run_never_called_counts_zero_in_it_and_spreads_to_zero() {
    let with = vec![
        Row::value("script.A.f".into(), 0.3),
        Row::value("gpu_ms".into(), 2.0),
    ];
    let without = vec![Row::value("gpu_ms".into(), 4.0)];
    let rows = over_runs(&[without, with]);
    let verb = rows.iter().find(|r| r.metric == "script.A.f");
    let verb = verb.expect("the verb has a row");
    assert!((verb.avg - 0.15).abs() < 1e-12, "{verb:?}");
    assert_eq!((verb.lo, verb.hi), (0.0, 0.3));
    assert_eq!(avg(&rows, "gpu_ms"), 3.0, "other rows are as before");
}
