//! src/ecs/journal.rs — the edit-mode change journal (#1631): Unity's
//! `Undo.RecordObject`, recorded at the one place every entity write passes.
//!
//! While the journal is recording, the first write to an entity (a column
//! borrowed mutably, attached, detached or overwritten; the entity placed or
//! despawned) records that entity's state *before* the write, once: its document
//! with the mesh's geometry left out ([`MeshComponent::without_geometry`]) and its
//! slot in the insertion order. An entity that did not exist yet records `None`.
//! Nothing else is copied, so a gesture costs what it touched, not the scene.
//!
//! The journal only records. The scene's undo stack (`scene::undo`) turns it on
//! around an edit (an editor frame, a console line), takes what it recorded as
//! one step, and turns it off; outside Play only. Every hot-path write pays one
//! `bool` test while it is off.

use std::collections::BTreeMap;

use crate::components::{Entity, MeshComponent};

use super::bundle::Core;
use super::world::World;

/// One entity's state before an edit: its document (mesh geometry left out) and
/// its insertion-sequence stamp, which [`World::restore_entity`] puts it back at.
#[derive(Clone, Debug)]
pub struct Recorded {
    pub doc: Entity,
    pub seq: u64,
}

/// What the journal recorded since it was last taken.
#[derive(Debug, Default)]
pub struct Journaled {
    /// Each touched entity's state before its first write; `None` for one created.
    pub before: BTreeMap<u32, Option<Recorded>>,
}

/// The journal's state on the World.
#[derive(Debug, Default)]
pub(super) struct Journal {
    recording: bool,
    taken: Option<Journaled>,
}

impl World {
    /// Start (or resume) recording; what was recorded and not taken is kept.
    pub fn journal_resume(&mut self) {
        self.journal.recording = true;
    }

    /// Stop recording; what was recorded is kept for [`World::journal_take`].
    pub fn journal_pause(&mut self) {
        self.journal.recording = false;
    }

    /// Whether writes are being recorded now.
    pub fn journal_recording(&self) -> bool {
        self.journal.recording
    }

    /// Take what was recorded (`None` when nothing was); recording is unchanged.
    pub fn journal_take(&mut self) -> Option<Journaled> {
        self.journal.taken.take()
    }

    /// Entity `id`'s state as the journal records it: `None` for a dead id.
    pub fn record(&self, id: u32) -> Option<Recorded> {
        let seq = self.component::<Core>(id)?.seq;
        let doc = self.document(id, MeshComponent::without_geometry)?;
        Some(Recorded { doc, seq })
    }

    /// Record `id`'s state before a write, if recording and not yet recorded.
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
