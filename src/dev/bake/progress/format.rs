//! The text of a bake's progress lines: percentages, counts, durations and the ETA.

use std::time::Duration;

use super::{Progress, Step};

/// One progress line, e.g. `[Lighting] lightmaps 41% (612/1501 meshes, 3.1M/7.6M
/// texels), ~38 min left`. A step whose total is not known yet says it is preparing.
pub(super) fn progress(p: &Progress, left: Option<Duration>) -> String {
    let what = match p.step {
        Step::Lightmaps => "lightmaps".to_string(),
        Step::LightProbes { bounce, max } => {
            format!("light probes, bounce {bounce} of at most {max}:")
        }
        Step::ReflectionProbes => "reflection probes".to_string(),
    };
    if p.total == 0 {
        return format!("[Lighting] {} preparing", what.trim_end_matches(':'));
    }
    let percent = p.done.min(p.total) * 100 / p.total;
    let counts = match (p.step, p.meshes) {
        (Step::Lightmaps, Some((done, total))) => format!(
            "{done}/{total} meshes, {}/{} texels",
            count(p.done),
            count(p.total)
        ),
        (Step::Lightmaps, None) => format!("{}/{} texels", count(p.done), count(p.total)),
        _ => format!("{}/{} probes", p.done, p.total),
    };
    let mut line = format!("[Lighting] {what} {percent}% ({counts})");
    if let Some(left) = left {
        line.push_str(&format!(", ~{} left", duration(left)));
        if matches!(p.step, Step::LightProbes { .. }) {
            line.push_str(" in this bounce");
        }
    }
    line
}

/// Time left at the step's rate since `from` (when, and how much was done then):
/// `None` until some work was done since, or once it is all done.
pub(super) fn eta(from: (Duration, u64), now: Duration, p: &Progress) -> Option<Duration> {
    let (at, done_then) = from;
    let did = p.done.checked_sub(done_then).filter(|&d| d > 0)?;
    let left = p.total.checked_sub(p.done).filter(|&l| l > 0)?;
    let took = now.saturating_sub(at).as_secs_f64();
    Some(Duration::from_secs_f64(took * left as f64 / did as f64))
}

/// A duration as a person reads it: `45 s`, `38 min`, `1 h 4 min`. Under ten
/// minutes the seconds stay.
pub(super) fn duration(d: Duration) -> String {
    let secs = d.as_secs();
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    match (h, m) {
        (0, 0) => format!("{s} s"),
        (0, m) if m < 10 => format!("{m} min {s} s"),
        (0, m) => format!("{m} min"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// A large count shortened: `999`, `12.3k`, `3.1M` (rounding never shows `1000.0k`).
pub(super) fn count(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..999_950 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}
