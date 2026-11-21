use std::cell::Cell;

use rusty_scene::revision::Revision;
use rusty_scene::{Scene, SceneId};

use crate::Renderer;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static NOTES: Cell<u64> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn notes() -> u64 {
    ARMED.with(|a| a.set(true));
    NOTES.with(Cell::get)
}

pub(super) fn counted() {
    if ARMED.with(Cell::get) {
        NOTES.with(|n| n.set(n.get() + 1));
    }
}

#[derive(Default)]
struct Seen {
    drew: Option<(SceneId, bool)>,
    textures: usize,
    lightmaps: Option<Revision>,
}

#[derive(Default)]
pub(crate) struct Gate {
    seen: [Seen; 2],
    due: bool,
}

impl Renderer {
    pub(crate) fn open_notes(&mut self, scene: &Scene) {
        let (textures, aside) = (self.gpu_textures.len(), self.is_aside());
        let gate = &mut self.retained.residency.notes;
        let seen = &mut gate.seen[usize::from(aside)];
        let drew = Some((scene.id(), scene.is_playing()));
        gate.due = seen.drew != drew || seen.textures != textures;
        (seen.drew, seen.textures) = (drew, textures);
    }

    pub(crate) fn notes_due(&self) -> bool {
        self.retained.residency.notes.due
    }

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
