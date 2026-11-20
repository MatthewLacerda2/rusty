//! A row against the same row of an earlier run: the change in its average, or
//! `unresolved` when that change is inside the run-to-run spread, so noise is not
//! read as a win. A row's spread is the lowest to the highest of its runs'
//! averages (`RUNS`); two runs' rows resolve a change only when their spreads do
//! not overlap. A run of one, or a one-off row (load, memory), has no spread, and
//! its change is printed as measured. Rows compare only between runs of one
//! shape (`SceneReport::same_shape`); [`SceneReport::against`] says why not.

use super::super::super::clock::{clock_drift, GAME_ROWS};
use super::super::super::report::{change, num, tick_timed_since};
use super::super::editor;
use super::{Row, SceneReport, ViewKey};

impl SceneReport {
    /// What the comparison with `previous` should warn of: a run of another
    /// shape ([`Self::same_shape`]), whose rows get no change column, or a CPU
    /// clock that moved since it (`clock::cpu_drift`).
    pub fn against(&self, previous: Option<&SceneReport>) -> Vec<String> {
        let Some(previous) = previous else {
            return Vec::new();
        };
        if !self.same_shape(previous) {
            let mut what = Vec::new();
            if self.size != previous.size {
                let ((w, h), (pw, ph)) = (self.size, previous.size);
                what.push(format!("size {pw}x{ph} -> {w}x{h}"));
            }
            if self.views != previous.views {
                what.push(views_moved(&previous.views, &self.views));
            }
            if self.frames != previous.frames {
                what.push(format!("frames {} -> {}", previous.frames, self.frames));
            }
            if self.script != previous.script {
                let timed = |on: bool| if on { "verbs timed" } else { "plain" };
                what.push(format!(
                    "{} -> {}",
                    timed(previous.script),
                    timed(self.script)
                ));
            }
            let other = if self.baseline.is_some() {
                "baseline"
            } else {
                "last run"
            };
            return vec![format!(
                "the {other} benched another shape ({}): no change column this time",
                what.join(", ")
            )];
        }
        // The game rows' clock, and the editor round's (#2314), each on its own.
        let rounds = [
            ("cpu_clock_pct", GAME_ROWS, ""),
            ("editor.cpu_clock_pct", editor::CLOCK_ROWS, editor::SAID),
        ];
        let drift = rounds.map(|(row, rows, said)| {
            let clock = |r: &SceneReport| r.row(row).map(|r| r.avg);
            let moved = clock(self).zip(clock(previous));
            let drift = moved.and_then(|(now, before)| clock_drift(now, before, rows));
            drift.map(|w| format!("{said}{w}"))
        });
        let warnings = drift.into_iter().flatten().chain(self.set_moved(previous));
        warnings.chain(self.tick_timed_since(previous)).collect()
    }

    /// [`tick_timed_since`] on this report's rows and `previous`'s.
    fn tick_timed_since(&self, previous: &SceneReport) -> Option<String> {
        tick_timed_since(|k| self.row(k).is_some(), |k| previous.row(k).is_some())
    }

    /// The warning for a comparison across other `--set` overrides: the change
    /// is the settings' as much as the code's.
    fn set_moved(&self, previous: &SceneReport) -> Option<String> {
        let said = |s: &[String]| match s.is_empty() {
            true => "none".to_string(),
            false => s.join(", "),
        };
        (self.set != previous.set).then(|| {
            format!(
                "compared with a run of other quality overrides ({} -> {}): the change \
                 is the settings' as well as the code's",
                said(&previous.set),
                said(&self.set)
            )
        })
    }
}

impl SceneReport {
    /// The notes on what the change column compares: the spread, or no earlier run.
    pub(super) fn comparison_notes(&self, first: bool) -> [(bool, &'static str); 3] {
        [
            (
                self.runs.max(1) > 1,
                "(spread: the lowest to the highest run's average; a change inside both \
                 runs' spreads is unresolved)",
            ),
            (
                first && self.baseline.is_none(),
                "(no previous report: the next run prints its change against this one)",
            ),
            (
                first && self.baseline.is_some(),
                "(the baseline holds no report of this scene: no change column)",
            ),
        ]
    }
}

/// What moved between two runs' viewpoints: their count, or the first one that
/// looks from elsewhere (a navmesh point that moved, another camera of one name).
fn views_moved(before: &[ViewKey], now: &[ViewKey]) -> String {
    if before.len() != now.len() {
        return format!("viewpoints {} -> {}", before.len(), now.len());
    }
    let moved = before.iter().zip(now).find(|(b, n)| b != n);
    moved.map_or_else(String::new, |(b, n)| {
        let at = |k: &ViewKey| {
            let [x, y, z] = k.at;
            let cam = k
                .camera
                .map_or(String::new(), |id| format!(" camera #{id}"));
            format!(
                "{}{cam} at ({x:.2}, {y:.2}, {z:.2}) yaw {:.1}",
                k.label, k.yaw
            )
        };
        format!("viewpoint {} -> {}", at(b), at(n))
    })
}

/// `now`'s average against `before`'s: `unresolved` when either has a spread
/// and the two overlap, else the change (`+0.123 (+4.5%)`, `=`).
pub fn delta(before: &Row, now: &Row) -> String {
    let spread = now.hi > now.lo || before.hi > before.lo;
    let overlap = now.lo <= before.hi && before.lo <= now.hi;
    if spread && overlap && now.avg != before.avg {
        return "unresolved (inside the spread)".to_string();
    }
    change(before.avg, now.avg)
}

/// The row's spread as the table prints it, `lo..hi`; empty without one.
pub fn spread(row: &Row) -> String {
    if row.hi > row.lo {
        format!("{}..{}", num(row.lo), num(row.hi))
    } else {
        String::new()
    }
}

#[cfg(test)]
#[path = "compare_tests.rs"]
mod tests;
