pub mod commit;
pub mod content;
pub mod digest;

use commit::{engine_commit, engine_order, UNKNOWN_COMMIT};

const DIRTY: &str = "-dirty";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineCheck {
    pub recorded: Option<String>,
    pub recorded_order: Option<u64>,
    pub running: String,
    pub running_order: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Same,
    Upgrade,
    Older,
    Diverged,
    Unknown,
}

impl EngineCheck {
    pub fn against(recorded: Option<&str>, order: Option<u64>) -> Self {
        Self {
            recorded: recorded.filter(|c| !c.is_empty()).map(str::to_string),
            recorded_order: order,
            running: engine_commit().to_string(),
            running_order: engine_order(),
        }
    }

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

pub(super) fn base(commit: &str) -> &str {
    commit.strip_suffix(DIRTY).unwrap_or(commit)
}

fn hash(commit: &str) -> &str {
    let hash = base(commit);
    hash.char_indices()
        .nth(12)
        .map_or(hash, |(end, _)| &hash[..end])
}

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
