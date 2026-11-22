use super::super::clock::{ClockRows, Spread};
use super::super::{Report, Summary};
use super::report::Row;
use crate::bench::verbs::absent_is_zero;

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
    let all = u32::try_from(reports.len()).unwrap_or(u32::MAX);
    rows.into_iter()
        .map(|(mut row, n)| {
            let n = if absent_is_zero(&row.metric) { all } else { n };
            row.avg /= f64::from(n.max(1));
            (row.lo, row.hi) = (row.avg, row.avg);
            row
        })
        .collect()
}

pub fn over_runs(runs: &[Vec<Row>]) -> Vec<Row> {
    let mut rows: Vec<(Row, u32)> = Vec::new();
    for run in runs {
        for r in run {
            match rows.iter_mut().find(|(row, _)| row.metric == r.metric) {
                Some((row, n)) => {
                    row.avg += r.avg;
                    *n += 1;
                    row.lo = row.lo.min(r.avg);
                    row.hi = row.hi.max(r.avg);
                    if r.p95 > row.p95 {
                        (row.p95, row.worst) = (r.p95, r.worst.clone());
                    }
                }
                None => rows.push((r.clone(), 1)),
            }
        }
    }
    let all = u32::try_from(runs.len()).unwrap_or(u32::MAX);
    rows.into_iter()
        .map(|(mut row, n)| {
            if absent_is_zero(&row.metric) && n < all {
                // A run without the verb's row called it for no time: that run counts as
                // zero, the lowest, and the mean divides by every run.
                row.lo = row.lo.min(0.0);
                row.avg /= f64::from(all);
            } else {
                row.avg /= f64::from(n);
            }
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
        lo: 0.0,
        hi: 0.0,
        quiet: s.quiet,
    }
}

pub fn spread_warnings(spreads: &[(String, Vec<(Spread, f64)>)], rows: ClockRows) -> Vec<String> {
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
            "{} (worst at {worst}; {} of {} measured windows)",
            kind.warning(by, rows),
            had.len(),
            spreads.len()
        ));
    }
    out
}

#[cfg(test)]
#[path = "fold_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "fold_script_tests.rs"]
mod script_tests;
