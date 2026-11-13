//! src/scene/snapshot.rs — edit-mode snapshot / restore around Play
//!
//! "Save as-is in EDIT mode" guarantee: edit-mode is authoritative.
//! - Entering Play -> snapshot the edit scene (clone); run Play on the live one.
//! - Exiting Play  -> restore the snapshot, discarding all play-mode mutation
//!   (script/physics moves, component changes, spawns/despawns).
//!
//! So Save never persists played state. Mirrors Unity's play-mode behaviour.
//!
//! Implemented as a `SceneData` snapshot taken on the play-state transition and
//! applied back via `serialize::apply_world_data` on Stop, plus what the scene
//! state inventory (`scene::state`, #1649) says Stop puts back and the document
//! does not carry: the probes' baked SH, the edit-mode decals, the saved navmesh, the scene's
//! identity and its disk records. Play's own requests (queued destroys and loads,
//! `DontDestroyOnLoad`) end with the play state, in `Scene::set_playing`.
//!
//! A demo closing over the edit scene puts it back with this same snapshot
//! (`dev::demo::Origin`, #1598), so the two restores cannot drift.

use crate::scene::serialize::{apply_world_data, to_scene_data, SceneData};
use crate::scene::Scene;

/// A frozen copy of the edit-mode scene, taken when entering Play.
pub struct SceneSnapshot {
    data: SceneData,
    /// The full probe volume INCLUDING baked SH. `SceneData` skips probe SH (it
    /// lives in the sidecar on disk), but an in-memory snapshot must preserve it so
    /// exiting Play doesn't blank out probe-driven ambient (#240).
    probes: crate::scene::lighting::probe::ProbeVolume,
    /// The edit-mode decals (#1518): runtime state `SceneData` does not carry, put
    /// back on Stop so the ones stamped in edit mode survive Play and Play's don't.
    decals: crate::scene::decal::DecalSet,
    /// The scene's identity (#355): a play-mode `Scene.Load` mints a new one, and
    /// Stop gives back the one Play started in, so a bake waiting for Stop still
    /// recognises the scene (#1374).
    id: crate::scene::SceneId,
    /// The text the engine last read or wrote at the scene's file (#1340), so the
    /// editor's own next write after a put-back is no outside change to reload.
    disk_text: crate::scene::io::DiskText,
    /// The baked PVS (#1714): no document field (it is the `<scene>.occlusion`
    /// file), so it is put back as it was.
    occlusion: Option<std::sync::Arc<crate::scene::lighting::occlusion::OcclusionData>>,
    /// The saved navmesh's bytes (#1887): no document field either.
    navmesh: crate::scene::nav_settings::SavedNavmesh,
    /// What the edit scene last read or wrote at its bake sidecars (#1982): a Play
    /// load adds its own, which Stop drops.
    bake_files: crate::scene::io::BakeFiles,
}

impl SceneSnapshot {
    /// Snapshot the current (edit-mode) scene. Captures component VALUES only —
    /// no GPU buffers — exactly like a save, but kept in memory.
    pub fn capture(scene: &Scene) -> Self {
        Self {
            data: to_scene_data(scene),
            probes: scene.probes.clone(),
            decals: scene.decals.clone(),
            id: scene.id(),
            disk_text: scene.disk_text(),
            occlusion: (*scene.occlusion).clone(),
            navmesh: scene.navmesh.clone(),
            bake_files: scene.bake_files.clone(),
        }
    }

    /// Restore the snapshot into `scene`, discarding any play-mode mutations: the
    /// decals go back to the edit-mode ones, dropping every one Play stamped,
    /// persistent ones too (#1518), as Unity's play-mode revert does.
    /// Consumes the snapshot since a snapshot is single-use per Play session.
    pub fn restore(self, scene: &mut Scene) {
        apply_world_data(scene, self.data);
        scene.decals.restore(self.decals);
        // Re-apply the baked SH the document-level apply dropped (it only carries
        // probe positions). Positions are identical, so this overwrites cleanly.
        scene.probes = self.probes;
        scene.restore_id(self.id);
        scene.restore_disk_text(self.disk_text);
        if *scene.occlusion != self.occlusion {
            *scene.occlusion = self.occlusion;
        }
        scene.navmesh = self.navmesh;
        scene.bake_files = self.bake_files;
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
