mod edited;
pub use edited::Saves;
mod level;
mod restore;
mod side;

use std::collections::{BTreeMap, VecDeque};

use crate::Scene;
use rusty_ecs::Recorded;

use level::{LevelDelta, SceneLevel};

/// Steps kept: a step holds only what one edit touched, and an API call touching every entity
/// of a Dust2-sized scene cannot pile up without bound.
pub const DEPTH: usize = 200;

#[derive(Debug)]
struct Step {
    name: String,
    entities: BTreeMap<u32, Option<Recorded>>,
    level: Option<LevelDelta>,
    side: side::Side,
}

#[derive(Debug, Default)]
pub struct UndoStack {
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    open: bool,
    level: Option<SceneLevel>,
    dropped: side::Side,
    edited: bool,
}

impl Scene {
    /// Records writes until `pause_edits` or `commit_edits`; a paused edit stays open and
    /// this resumes it.
    pub fn begin_edits(&mut self) {
        if self.is_playing() {
            self.world.journal_resume();
            return;
        }
        if !self.history.open {
            self.history.open = true;
            self.rebaseline_level();
        }
        self.world.journal_resume();
    }

    pub fn pause_edits(&mut self) {
        self.world.journal_pause();
        if self.is_playing() {
            self.take_step(None);
        }
    }

    pub fn commit_edits(&mut self, name: Option<&str>) -> bool {
        let Some(step) = self.take_step(name) else {
            return false;
        };
        self.push_undo(step);
        self.history.redo.clear();
        true
    }

    /// The open edit joins the last step, as Unity folds it into the current group, and
    /// never clears Redo.
    pub fn commit_follow_up(&mut self) -> bool {
        let Some(step) = self.take_step(None) else {
            return false;
        };
        if self.history.undo.is_empty() {
            self.push_undo(step);
            return true;
        }
        self.note_edit();
        let Some(last) = self.history.undo.back_mut() else {
            return true;
        };
        for (id, before) in step.entities {
            // The last step's record is older, so it wins.
            last.entities.entry(id).or_insert(before);
        }
        match (&mut last.level, step.level) {
            (Some(older), Some(later)) => older.absorb(later),
            (slot @ None, later) => *slot = later,
            (Some(_), None) => {}
        }
        last.side.absorb(step.side);
        true
    }

    /// An open edit closes first and reopens after, so a mid-drag bake joins neither step.
    pub fn unrecorded<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let (open, recording) = (self.history.open, self.world.journal_recording());
        self.commit_edits(None);
        self.world.journal_pause();
        let out = f(self);
        if open {
            self.begin_edits();
            if !recording {
                self.pause_edits();
            }
        }
        out
    }

    fn take_step(&mut self, name: Option<&str>) -> Option<Step> {
        self.world.journal_pause();
        let journaled = self.world.journal_take();
        let open = std::mem::take(&mut self.history.open);
        let before = journaled.map(|j| j.before).unwrap_or_default();
        let changed_now = |(id, before): &(u32, Option<Recorded>)| {
            changed(before.as_ref(), self.world.record(*id).as_ref())
        };
        if self.is_playing() {
            self.history.edited |= before.into_iter().any(|entry| changed_now(&entry));
            return None;
        }
        if !open {
            return None;
        }
        let mut dropped = std::mem::take(&mut self.history.dropped);
        let entities: BTreeMap<_, _> = before.into_iter().filter(changed_now).collect();
        let level = self
            .level_changed()
            .then(|| self.history.level.take())
            .flatten()
            .map(|baseline| baseline.delta(self));
        if entities.is_empty() && level.is_none() {
            return None;
        }
        let name = name.map_or_else(|| self.step_name(&entities), str::to_string);
        dropped.retain(|id| entities.contains_key(&id));
        Some(Step {
            name,
            entities,
            level,
            side: dropped,
        })
    }

    pub fn undo(&mut self) -> Option<String> {
        self.step_back(true)
    }

    pub fn redo(&mut self) -> Option<String> {
        self.step_back(false)
    }

    pub fn undo_name(&self) -> Option<&str> {
        self.history.undo.back().map(|s| s.name.as_str())
    }

    pub fn redo_name(&self) -> Option<&str> {
        self.history.redo.last().map(|s| s.name.as_str())
    }

    pub fn clear_undo(&mut self) {
        self.world.journal_pause();
        self.world.journal_take();
        self.history = UndoStack::default();
    }

    fn step_back(&mut self, back: bool) -> Option<String> {
        if self.is_playing() {
            return None;
        }
        let was_open = self.history.open;
        self.commit_edits(None);
        let step = if back {
            self.history.undo.pop_back()
        } else {
            self.history.redo.pop()
        }?;
        let inverse = restore::apply(self, step);
        self.note_edit();
        let name = inverse.name.clone();
        if back {
            self.history.redo.push(inverse);
        } else {
            self.push_undo(inverse);
        }
        if was_open {
            self.begin_edits();
        }
        Some(name)
    }

    fn push_undo(&mut self, step: Step) {
        self.note_edit();
        if self.history.undo.len() == DEPTH {
            self.history.undo.pop_front();
        }
        self.history.undo.push_back(step);
    }

    fn rebaseline_level(&mut self) {
        let held = self.history.level.as_ref();
        if !held.is_some_and(|level| level.matches(self)) {
            self.history.level = Some(SceneLevel::capture(self));
        }
    }

    fn level_changed(&self) -> bool {
        self.history
            .level
            .as_ref()
            .is_some_and(|level| !level.matches(self))
    }

    fn step_name(&self, entities: &BTreeMap<u32, Option<Recorded>>) -> String {
        let named = |id: u32, before: &Option<Recorded>| {
            let live = self.world.name(id).map(|n| (*n).clone());
            let name = live.or_else(|| before.as_ref().map(|r| r.doc.name.clone()));
            let verb = match (before, self.world.contains(id)) {
                (None, _) => "Create",
                (Some(_), false) => "Destroy",
                (Some(_), true) => "Edit",
            };
            format!("{verb} {}", name.unwrap_or_default())
        };
        match entities.len() {
            0 => "Scene Settings".to_string(),
            1 => entities.iter().map(|(&id, b)| named(id, b)).collect(),
            n => format!("Edit {n} objects"),
        }
    }
}

/// By the saved form, so a runtime-only field (an animator's playhead) is no edit.
fn changed(before: Option<&Recorded>, after: Option<&Recorded>) -> bool {
    match (before, after) {
        (None, None) => false,
        (Some(b), Some(a)) => serde_json::to_vec(&b.doc).ok() != serde_json::to_vec(&a.doc).ok(),
        _ => true,
    }
}

#[cfg(test)]
mod bake_tests;
#[cfg(test)]
mod changes_tests;
#[cfg(test)]
mod level_tests;
