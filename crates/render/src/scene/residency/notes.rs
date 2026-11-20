//! crates/render/src/scene/residency/notes.rs — when the holders note their maps
//! (`maps`): only on a render where what they would note can have changed, so a
//! settled frame notes nothing (#2458).
//!
//! A render's notes are **due** ([`Renderer::notes_due`]) when the render before it
//! of the same kind (the main view, or one aside: a preview or a capture) drew
//! another scene or the same one in another play state (a load, a Play entry,
//! Stop, a preview opened), or when the texture cache gained or lost an entry
//! since (a holder's map changed to one not yet resident). On a due render every
//! holder notes what it binds, once, for the scene drawn. Besides, a holder notes
//! on its own change, where it already takes a slow path: the sky when its path
//! changes, a UI image when its bind group is rebuilt, the lightmaps when the
//! scene's lightmap set moves its revision. What this costs a settled render is
//! three compares (the scene and play state, the cache's size, the lightmap
//! revision) and no note.
//!
//! A holder that moves to a map already resident under another scene is not
//! noted until the next due render; a release meanwhile can drop the map only if
//! no kept scene draws it, and a wrong one costs a re-read (`maps`).
//!
//! Note calls are counted on a thread that asked [`notes`] once: a test hook.

use std::cell::Cell;

use rusty_scene::revision::Revision;
use rusty_scene::{Scene, SceneId};

use crate::Renderer;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static NOTES: Cell<u64> = const { Cell::new(0) };
}

/// Note calls (`note_map`, `note_lightmaps`) made on this thread since the first
/// call here on it, which starts the count. A test hook.
#[cfg(test)]
pub(crate) fn notes() -> u64 {
    ARMED.with(|a| a.set(true));
    NOTES.with(Cell::get)
}

/// Count one note call, when this thread is armed.
pub(super) fn counted() {
    if ARMED.with(Cell::get) {
        NOTES.with(|n| n.set(n.get() + 1));
    }
}

/// What one kind of render last drew, against which the next one's notes fall due.
#[derive(Default)]
struct Seen {
    drew: Option<(SceneId, bool)>,
    textures: usize,
    lightmaps: Option<Revision>,
}

/// The main view's and the aside renders' last draws, and whether this render's
/// notes are due.
#[derive(Default)]
pub(crate) struct Gate {
    seen: [Seen; 2],
    due: bool,
}

impl Renderer {
    /// Open a render of `scene`: decide whether its notes are due.
    pub(crate) fn open_notes(&mut self, scene: &Scene) {
        let (textures, aside) = (self.gpu_textures.len(), self.is_aside());
        let gate = &mut self.retained.residency.notes;
        let seen = &mut gate.seen[usize::from(aside)];
        let drew = Some((scene.id(), scene.is_playing()));
        gate.due = seen.drew != drew || seen.textures != textures;
        (seen.drew, seen.textures) = (drew, textures);
    }

    /// Whether this render's holders note what they bind (see the module docs).
    pub(crate) fn notes_due(&self) -> bool {
        self.retained.residency.notes.due
    }

    /// Note the lightmap lists `scene` binds, on a due render or when its set
    /// moved since the last render of this kind.
    pub(crate) fn note_scene_lightmaps(&mut self, scene: &Scene) {
        let (revision, aside) = (Some(scene.lightmaps.revision()), self.is_aside());
        let gate = &mut self.retained.residency.notes;
        let seen = &mut gate.seen[usize::from(aside)];
        if !gate.due && seen.lightmaps == revision {
            return;
        }
        seen.lightmaps = revision;
        let set = &scene.lightmaps;
        self.note_lightmaps(scene.id(), [&set.pages, &set.directions, &set.masks]);
    }
}
