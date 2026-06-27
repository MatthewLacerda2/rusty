//! src/scene/snapshot.rs — edit-mode snapshot / restore around Play
//!
//! "Save as-is in EDIT mode" guarantee: edit-mode is authoritative.
//! - Entering Play -> snapshot the edit scene (clone); run Play on the live one.
//! - Exiting Play  -> restore the snapshot, discarding all play-mode mutation
//!   (script/physics moves, health changes, spawns/despawns).
//!
//! So Save never persists played state. Mirrors Unity's play-mode behaviour.
//!
//! Implemented as a `SceneData` snapshot taken on the play-state transition and
//! applied back via `serialize::apply_scene_data` on Stop.
//!
//! Allowed deps: scene::serialize, ecs.

use crate::scene::serialize::{apply_scene_data, to_scene_data, SceneData};
use crate::scene::Scene;

/// A frozen copy of the edit-mode scene, taken when entering Play.
pub struct SceneSnapshot {
    data: SceneData,
    /// The full probe volume INCLUDING baked SH. `SceneData` skips probe SH (it
    /// lives in the sidecar on disk), but an in-memory snapshot must preserve it so
    /// exiting Play doesn't blank out probe-driven ambient (#240).
    probes: crate::scene::lighting::probe::ProbeVolume,
}

impl SceneSnapshot {
    /// Snapshot the current (edit-mode) scene. Captures component VALUES only —
    /// no GPU buffers — exactly like a save, but kept in memory.
    pub fn capture(scene: &Scene) -> Self {
        Self {
            data: to_scene_data(scene),
            probes: scene.probes.clone(),
        }
    }

    /// Restore the snapshot into `scene`, discarding any play-mode mutations.
    /// Consumes the snapshot since a snapshot is single-use per Play session.
    pub fn restore(self, scene: &mut Scene) {
        apply_scene_data(scene, self.data);
        // Re-apply the baked SH the document-level apply dropped (it only carries
        // probe positions). Positions are identical, so this overwrites cleanly.
        scene.probes = self.probes;
    }
}
