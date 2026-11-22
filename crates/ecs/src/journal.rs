//! The edit-mode change journal, Unity's `Undo.RecordObject`: while recording, an entity's first
//! write records its state before it, once (geometry left out), `None` for one not yet alive.
//! `scene::undo` turns it on around an edit and takes the step; off, a write pays one `bool` test.

use std::collections::BTreeMap;

use rusty_components::{ColliderComponent, Entity, MeshComponent};

use super::bundle::Core;
use super::world::World;

#[derive(Clone, Debug)]
/// `seq` is the insertion-order stamp [`World::restore_entity`] puts it back at.
pub struct Recorded {
    pub doc: Entity,
    pub seq: u64,
}

#[derive(Debug, Default)]
pub struct Journaled {
    pub before: BTreeMap<u32, Option<Recorded>>,
}

#[derive(Debug, Default)]
pub(super) struct Journal {
    recording: bool,
    taken: Option<Journaled>,
}

impl World {
    /// What was recorded and not taken is kept.
    pub fn journal_resume(&mut self) {
        self.journal.recording = true;
    }

    pub fn journal_pause(&mut self) {
        self.journal.recording = false;
    }

    pub fn journal_recording(&self) -> bool {
        self.journal.recording
    }

    pub fn journal_take(&mut self) -> Option<Journaled> {
        self.journal.taken.take()
    }

    pub fn record(&self, id: u32) -> Option<Recorded> {
        let seq = self.component::<Core>(id)?.seq;
        let doc = self.document(
            id,
            MeshComponent::without_geometry,
            ColliderComponent::without_geometry,
        )?;
        Some(Recorded { doc, seq })
    }

    pub(super) fn note(&mut self, id: u32) {
        if !self.journal.recording {
            return;
        }
        if let Some(taken) = &self.journal.taken {
            if taken.before.contains_key(&id) {
                return;
            }
        }
        let recorded = self.record(id);
        let taken = self.journal.taken.get_or_insert_with(Journaled::default);
        taken.before.insert(id, recorded);
    }
}
