//! src/core/project/engine/ — which engine a project was last opened with, against
//! the one running (#853, #1319).
//!
//! An engine is a commit and its place in history ([`commit`]). Every merge to
//! `main` is a new engine, and rusty keeps no backward compatibility, so moving
//! **forward** is the normal case: an [`Upgrade`](Verdict::Upgrade) opens without
//! asking and says what changed hands. Only an **older** engine, or one from another
//! line of history, is worth a warning. (Unity Hub asks on every version change;
//! rusty departs from it on the operator's say-so.)

pub mod commit;

use commit::{engine_commit, engine_order, UNKNOWN_COMMIT};

const DIRTY: &str = "-dirty";

/// The open-time engine check: which engine the project was last opened with, and
/// which one is running. A frontend decides how to tell the user ([`warning`],
/// [`upgrade`]); [`open`](super::open) only reports it.
///
/// [`warning`]: EngineCheck::warning
/// [`upgrade`]: EngineCheck::upgrade
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineCheck {
    /// The commit the project recorded; `None` when it recorded none (a new project,
    /// or one from before `project.rusty`).
    pub recorded: Option<String>,
    /// Where the recorded commit sits in history; `None` when the file doesn't say.
    pub recorded_order: Option<u64>,
    /// The running engine's commit ([`engine_commit`]).
    pub running: String,
    /// The running engine's place in history ([`engine_order`]).
    pub running_order: Option<u64>,
}

/// How the running engine relates to the one a project recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The same commit (a `-dirty` suffix aside), or nothing recorded yet.
    Same,
    /// A later engine: it counts higher, or the file recorded no count.
    Upgrade,
    /// An earlier engine: it counts lower.
    Older,
    /// Another line of history: the same count, a different commit (a branch build).
    Diverged,
    /// Can't be told: an `unknown` commit, or a running engine that can't count.
    Unknown,
}

impl EngineCheck {
    /// The check of a project that recorded `recorded` at `order` against this build.
    pub fn against(recorded: Option<&str>, order: Option<u64>) -> Self {
        Self {
            recorded: recorded.filter(|c| !c.is_empty()).map(str::to_string),
            recorded_order: order,
            running: engine_commit().to_string(),
            running_order: engine_order(),
        }
    }

    /// How the two engines relate. Local edits on the same commit are the engine
    /// developer's normal state, so `-dirty` is ignored. A shallow clone can't count,
    /// and its count of 1 must never read as older, so a running engine without a
    /// count is [`Unknown`](Verdict::Unknown); a record without one (every file from
    /// before counts were kept) is an upgrade.
    pub fn verdict(&self) -> Verdict {
        let Some(recorded) = self.recorded.as_deref() else {
            return Verdict::Same;
        };
        let (recorded, running) = (base(recorded), base(&self.running));
        if recorded == UNKNOWN_COMMIT || running == UNKNOWN_COMMIT {
            return Verdict::Unknown;
        }
        if recorded == running {
            return Verdict::Same;
        }
        let Some(running_order) = self.running_order else {
            return Verdict::Unknown;
        };
        match self.recorded_order.map(|order| running_order.cmp(&order)) {
            None | Some(std::cmp::Ordering::Greater) => Verdict::Upgrade,
            Some(std::cmp::Ordering::Less) => Verdict::Older,
            Some(std::cmp::Ordering::Equal) => Verdict::Diverged,
        }
    }

    /// The one line to confirm before opening: `Some` only for an older engine or
    /// one from another line of history.
    pub fn warning(&self) -> Option<String> {
        if !matches!(self.verdict(), Verdict::Older | Verdict::Diverged) {
            return None;
        }
        Some(format!(
            "this project was last opened with engine {}, this is engine {}: \
             scenes and scripts may not behave as they did",
            short(self.recorded.as_deref()?),
            short(&self.running)
        ))
    }

    /// The line an upgrade reports, naming the `git log` that lists what changed
    /// in the engine checkout; `None` for anything but an upgrade.
    pub fn upgrade(&self) -> Option<String> {
        if self.verdict() != Verdict::Upgrade {
            return None;
        }
        let (old, new) = (hash(self.recorded.as_deref()?), hash(&self.running));
        let engine = super::engine_dir();
        let checkout = engine.parent().unwrap_or(engine).display();
        Some(format!(
            "upgraded from engine {old} to {new}; what changed: \
             git -C {checkout} log --oneline {old}..{new}"
        ))
    }
}

/// A commit without its `-dirty` suffix.
pub(super) fn base(commit: &str) -> &str {
    commit.strip_suffix(DIRTY).unwrap_or(commit)
}

/// A commit's hash as people (and `git log`) read it: 12 hex digits. Cut by
/// characters, so a hand-edited commit that isn't ASCII can't split one.
fn hash(commit: &str) -> &str {
    let hash = base(commit);
    hash.char_indices()
        .nth(12)
        .map_or(hash, |(end, _)| &hash[..end])
}

/// [`hash`], plus `-dirty` when it had one.
fn short(commit: &str) -> String {
    let dirty = if base(commit).len() < commit.len() {
        DIRTY
    } else {
        ""
    };
    format!("{}{dirty}", hash(commit))
}

#[cfg(test)]
mod tests;
