//! src/scene/undo/ — Undo / Redo for edit mode (#1631): Unity's `Undo`.
//!
//! **What is recorded.** The state of each object an edit touched, from before
//! the edit (Unity's `Undo.RecordObject`), not a snapshot of the scene: the World's
//! change journal (`ecs::journal`) records an entity the first time anything writes
//! to it, and the scene level (Scene Settings, fog, layers, the material library,
//! …; `level`) is recorded as one more object. Both catch every caller at once, the
//! inspector, the gizmo, the hierarchy and every API call, because every entity
//! write passes the World's column accessors and the scene level is compared.
//!
//! **Gestures.** An edit runs between [`Scene::begin_edits`] and
//! [`Scene::commit_edits`]; a commit makes what it recorded one step. The editor
//! keeps one open across frames while a drag lasts (a slider, the gizmo, a colour
//! picker: Unity's grouping by gesture, `Undo.IncrementCurrentGroup`), and closes
//! it when the pointer is released; a console or API command is one step of its
//! own (`ScriptManager::eval`). A commit that changed nothing records nothing.
//!
//! **The stack** is one per open scene: `Scene::history`, so the editor, the
//! console and the headless session (which has no editor) share it. It holds only
//! edit-mode history: it is cleared when a scene is loaded and when Play starts
//! (Stop restores Play's changes already), and nothing is recorded in Play. A new
//! edit clears Redo. It keeps the last [`DEPTH`] steps.

mod level;
mod restore;
mod side;

use std::collections::{BTreeMap, VecDeque};

use crate::ecs::Recorded;
use crate::scene::Scene;

use level::{LevelDelta, SceneLevel};

/// How many steps Undo keeps. Unity bounds its history by memory, not count; a
/// step here holds only what one edit touched (documents without geometry), so
/// 200 covers a long session of hand edits while an API call that touched every
/// entity of a Dust2-sized scene cannot pile up without bound.
pub const DEPTH: usize = 200;

/// One undoable edit: each touched object's state before it.
#[derive(Debug)]
struct Step {
    name: String,
    entities: BTreeMap<u32, Option<Recorded>>,
    level: Option<LevelDelta>,
    /// The side entries of the step's destroyed entities (`side`).
    side: side::Side,
}

/// The scene's Undo and Redo stacks, and the open edit's scene-level baseline.
#[derive(Debug, Default)]
pub struct UndoStack {
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    /// Whether an edit is open (between `begin_edits` and `commit_edits`).
    open: bool,
    /// The scene level as it stood when the open edit began.
    level: Option<SceneLevel>,
    /// Side entries the open edit's despawns dropped (`side`).
    dropped: side::Side,
}

impl Scene {
    /// Open an edit (or resume the open one): writes from here are recorded until
    /// [`Scene::pause_edits`] or [`Scene::commit_edits`]. A no-op in Play.
    pub fn begin_edits(&mut self) {
        if self.is_playing() {
            return;
        }
        if !self.history.open {
            self.history.open = true;
            self.rebaseline_level();
        }
        self.world.journal_resume();
    }

    /// Stop recording without closing the open edit: what runs until the next
    /// [`Scene::begin_edits`] (a frame's followers, a render) is not an edit.
    pub fn pause_edits(&mut self) {
        self.world.journal_pause();
    }

    /// Close the open edit: what it changed becomes one Undo step, named `name`
    /// (or after what it touched), and Redo is cleared. Returns whether a step
    /// was recorded.
    pub fn commit_edits(&mut self, name: Option<&str>) -> bool {
        let Some(step) = self.take_step(name) else {
            return false;
        };
        self.push_undo(step);
        self.history.redo.clear();
        true
    }

    /// Close the open edit as part of the last step, not a step of its own: what
    /// the editor changed in a frame no one touched it (a card folding last
    /// frame's Add into the material library) finishes that frame's gesture, as
    /// Unity folds it into the current group. With no step yet, it is its own.
    /// It never clears Redo: it is no new edit, so an Undo stays redoable.
    pub fn commit_follow_up(&mut self) -> bool {
        let Some(step) = self.take_step(None) else {
            return false;
        };
        let Some(last) = self.history.undo.back_mut() else {
            self.push_undo(step);
            return true;
        };
        // The last step's record of an object is older, so it wins.
        for (id, before) in step.entities {
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

    /// Run `f` as no edit: what it writes (a lighting bake landing; Unity's Undo
    /// never reverts a bake) is neither recorded nor folded into a step. An edit
    /// open around it is closed first, as its own step, and reopened after, so
    /// a drag the bake lands in becomes two steps and neither holds the bake.
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

    /// The open edit closed into a step: `None` when it changed nothing.
    fn take_step(&mut self, name: Option<&str>) -> Option<Step> {
        self.world.journal_pause();
        let journaled = self.world.journal_take();
        if !std::mem::take(&mut self.history.open) || self.is_playing() {
            return None;
        }
        let before = journaled.map(|j| j.before).unwrap_or_default();
        let mut dropped = std::mem::take(&mut self.history.dropped);
        let entities: BTreeMap<_, _> = before
            .into_iter()
            .filter(|(id, before)| changed(before.as_ref(), self.world.record(*id).as_ref()))
            .collect();
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

    /// Undo the last step (the open edit is committed first, as Unity closes the
    /// current group). Returns the undone step's name, `None` when there was none.
    pub fn undo(&mut self) -> Option<String> {
        self.step_back(true)
    }

    /// Redo the last undone step. Returns its name, `None` when there was none.
    pub fn redo(&mut self) -> Option<String> {
        self.step_back(false)
    }

    /// The name of the step [`Scene::undo`] would revert.
    pub fn undo_name(&self) -> Option<&str> {
        self.history.undo.back().map(|s| s.name.as_str())
    }

    /// The name of the step [`Scene::redo`] would re-apply.
    pub fn redo_name(&self) -> Option<&str> {
        self.history.redo.last().map(|s| s.name.as_str())
    }

    /// Forget all history: a scene was loaded over this one, or Play started.
    pub(crate) fn clear_undo(&mut self) {
        self.world.journal_pause();
        self.world.journal_take();
        self.history = UndoStack::default();
    }

    /// Undo (`back`) or redo one step, moving its inverse to the other stack.
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
        if self.history.undo.len() == DEPTH {
            self.history.undo.pop_front();
        }
        self.history.undo.push_back(step);
    }

    /// Take the scene level as the open edit's baseline, unless it already is.
    fn rebaseline_level(&mut self) {
        let held = self.history.level.as_ref();
        if !held.is_some_and(|level| level.matches(self)) {
            self.history.level = Some(SceneLevel::capture(self));
        }
    }

    /// Whether the scene level differs from the open edit's baseline (which the
    /// commit then takes as the step's state before; the next edit re-takes one).
    fn level_changed(&self) -> bool {
        self.history
            .level
            .as_ref()
            .is_some_and(|level| !level.matches(self))
    }

    /// A name after what a step touched, as Unity names its undo groups.
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

/// Whether an entity's state moved: it appeared, vanished, or its saved form
/// differs (a runtime-only field, such as an animator's playhead, is no edit).
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
mod destroy_tests;
#[cfg(test)]
mod ids_tests;
#[cfg(test)]
mod level_tests;
#[cfg(test)]
mod tests;
