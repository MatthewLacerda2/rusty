//! The text of a bake's progress lines: percentages, counts, durations and the ETA.

use std::time::Duration;

use super::{Progress, Step};

/// One progress line, e.g. `[Lighting] 26:02 lightmaps 41% (612/1501 meshes,
/// 3.1M/7.6M texels), ~38 min left (then: light probes)`: `elapsed` since the bake
/// began, and `then` the steps still to come ([`then`]). A step whose total is not
/// known yet says it is preparing.
pub(super) fn progress(
    p: &Progress,
    elapsed: Duration,
    left: Option<Duration>,
    then: &str,
) -> String {
    let at = format!("[Lighting] {}", clock(elapsed));
    let what = match p.step {
        Step::LightProbes { bounce, max } => {
            format!("light probes, bounce {bounce} of at most {max}:")
        }
        other => other.name().to_string(),
    };
    if p.total == 0 {
        return format!("{at} {} preparing{then}", what.trim_end_matches(':'));
    }
    let percent = p.done.min(p.total) * 100 / p.total;
    let counts = match (p.step, p.meshes) {
        (Step::Lightmaps, Some((done, total))) => format!(
            "{done}/{total} meshes, {}/{} texels",
            count(p.done),
            count(p.total)
        ),
        (Step::Lightmaps, None) => format!("{}/{} texels", count(p.done), count(p.total)),
        (Step::LightmapPages, _) => format!("{}/{} pages encoded", p.done, p.total),
        (Step::Occlusion, _) => format!("{}/{} cells", p.done, p.total),
        _ => format!("{}/{} probes", p.done, p.total),
    };
    let mut line = format!("{at} {what} {percent}% ({counts})");
    if let Some(left) = left {
        line.push_str(&format!(", ~{} left", duration(left)));
        if matches!(p.step, Step::LightProbes { .. }) {
            line.push_str(" in this bounce");
        }
    }
    line.push_str(then);
    line
}

/// The steps of `plan` after `step`'s, as ` (then: light probes, occlusion)`;
/// empty for the last step, or a step the plan does not name.
pub(super) fn then(plan: &[Step], step: Step) -> String {
    let kind = std::mem::discriminant(&step);
    let Some(at) = plan.iter().position(|s| std::mem::discriminant(s) == kind) else {
        return String::new();
    };
    let rest: Vec<&str> = plan[at + 1..].iter().map(|s| s.name()).collect();
    match rest.is_empty() {
        true => String::new(),
        false => format!(" (then: {})", rest.join(", ")),
    }
}

/// The time since a bake began, as a clock reads: `0:05`, `26:02`, `1:05:10`.
pub(super) fn clock(d: Duration) -> String {
    let secs = d.as_secs();
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    match h {
        0 => format!("{m}:{s:02}"),
        h => format!("{h}:{m:02}:{s:02}"),
    }
}

/// Time left at the step's rate since `from` (when, and how much was done then: the
/// start of its window, `rate::Window::from`): `None` until some work was done
/// since, or once it is all done.
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
