//! Folding a scene's viewpoints into one report: every row over them (the
//! average of their averages, the worst p95 and the viewpoint that gave it), and
//! one warning per kind of CPU spread their windows showed.

use super::super::clock::Spread;
use super::super::{Report, Summary};
use super::report::Row;

/// Fold the viewpoints' reports, labelled, into rows: metrics in the order they
/// were first seen, each the average of the averages and the worst p95.
pub fn aggregate(reports: &[(String, Report)]) -> Vec<Row> {
    let mut rows: Vec<(Row, u32)> = Vec::new();
    for (label, report) in reports {
        for (metric, s) in &report.metrics {
            let at = match rows.iter().position(|(r, _)| &r.metric == metric) {
                Some(at) => at,
                None => {
                    rows.push((first(metric, label, s), 0));
                    rows.len() - 1
                }
            };
            let (row, n) = &mut rows[at];
            row.avg += s.avg;
            *n += 1;
            if s.p95 > row.p95 {
                (row.p95, row.worst) = (s.p95, label.clone());
            }
        }
    }
    rows.into_iter()
        .map(|(mut row, n)| {
            row.avg /= f64::from(n.max(1));
            row
        })
        .collect()
}

fn first(metric: &str, label: &str, s: &Summary) -> Row {
    Row {
        metric: metric.into(),
        avg: 0.0,
        p95: s.p95,
        worst: label.into(),
    }
}

/// One warning per kind of CPU spread the viewpoints' windows showed (each
/// viewpoint's label with its spreads, `clock::cpu_spreads`): the worst one's,
/// naming its viewpoint and how many of them had it.
pub fn spread_warnings(spreads: &[(String, Vec<(Spread, f64)>)]) -> Vec<String> {
    let mut out = Vec::new();
    for kind in [Spread::Clock, Spread::Cpu] {
        let had: Vec<(&str, f64)> = spreads
            .iter()
            .filter_map(|(label, s)| {
                let by = s.iter().find(|(k, _)| *k == kind)?.1;
                Some((label.as_str(), by))
            })
            .collect();
        let Some(&(worst, by)) = had.iter().max_by(|a, b| a.1.total_cmp(&b.1)) else {
            continue;
        };
        out.push(format!(
            "{} (worst at {worst}; {} of {} viewpoints)",
            kind.warning(by),
            had.len(),
            spreads.len()
        ));
    }
    out
}

#[cfg(test)]
#[path = "fold_tests.rs"]
mod tests;
