//! crates/render/src/scene/residency/mod.rs — which GPU meshes stay resident (#2440): a
//! mesh is released once no loaded scene references it, on a load that replaced
//! the World, as Unity's non-additive `SceneManager.LoadScene` unloads what the
//! scene before it alone used. Textures and lightmap page arrays are released at
//! the same moment under the same rule (`maps`).
//!
//! - **When**: at the end of a main-view frame (`Renderer::render`, outside
//!   `aside`, so no preview or capture) that drew a scene identity the last one
//!   did not. A load mints a new identity (`Scene::renew_id`) and Play and Stop
//!   keep it, so Play and Stop on one scene release nothing. It runs after the
//!   new scene's whole frame, so its meshes (objects and mesh particles) are
//!   already requested and one it shares with the scene before is kept, never
//!   uploaded again. A `Scene.LoadAsync`'s warm-up uploads while the old scene
//!   still draws and releases nothing; the release comes at the end of the first
//!   frame after the swap.
//! - **What counts as loaded**: every scene the renderer keeps (`Retained`'s up
//!   to four: the main one, an Inspector preview, a capture's), minus the one the
//!   main view just left, which is dropped with its objects and records. The one
//!   kept past it is the edit scene a Play started on (`held`): a play-mode
//!   `Scene.Load` leaves it in the editor's snapshot, and Stop draws it again.
//!   Any step from a frame not playing to one playing is a Play entry and holds
//!   the scene drawn before it, also when the entry tick loaded another (a
//!   bootstrap scene's `Start()` calling `Scene.Load`).
//! - **Used** is the meshes those scenes' objects name (inactive ones included),
//!   the mesh of each of their Mesh-mode emitters (inactive or emitting nothing
//!   included, noted by `push_mesh_particles` each frame it walks them), and the
//!   meshes a pending `Scene.LoadAsync` warm-up made, until its `Warming` is
//!   dropped (its load swapped in or was abandoned): a scene that opens the next
//!   load in its first tick keeps what that warm-up already uploaded. A release drops the buffers and
//!   the mesh's `GpuClusters`, whose lease gives the cluster pool its ranges;
//!   the handle stays interned, so an upload under it later is a plain one.
//! - **A mesh the pool refused** (`refused`) is retried after a release freed
//!   ranges, from the entity of the main scene that draws it, so no frame scans
//!   the meshes; one only a preview or a particle draws is retried at its next
//!   upload.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::MeshHandle;
use crate::{MeshId, Renderer};
use rusty_asset::MapRef;
use rusty_scene::{Scene, SceneId};

/// What decides the next release.
#[derive(Default)]
pub(crate) struct Residency {
    /// The scene the main view last drew, and whether it was playing.
    main: Option<(SceneId, bool)>,
    /// The edit scene Play started on, kept loaded until Stop.
    held: Option<SceneId>,
    /// Per kept scene, the meshes its Mesh-mode emitters name.
    particles: BTreeMap<SceneId, BTreeSet<MeshHandle>>,
    /// Meshes resident without clusters because the pool was full.
    pub(crate) refused: BTreeSet<MeshId>,
    /// Per pending `Scene.LoadAsync` warm-up (alive while its `Warming` is), what
    /// it made: used until its load swaps in or is abandoned.
    warmed: Vec<maps::Warmed>,
    /// Per kept scene, the maps its UI, particles, ribbons and sky loaded (`maps`).
    maps: BTreeMap<SceneId, BTreeSet<MapRef>>,
    /// Per kept scene, the lightmap page lists it last bound (`maps`).
    lightmaps: BTreeMap<SceneId, [Vec<String>; 3]>,
}

impl Residency {
    /// The main view drew scene `id`: the scene it left, if a load replaced it.
    fn drew(&mut self, id: SceneId, playing: bool) -> Option<SceneId> {
        let last = self.main.replace((id, playing));
        match (last, playing) {
            (_, false) => self.held = None,
            // A Play entry, even one whose first tick loaded a scene (a bootstrap
            // `Start()` calling `Scene.Load`): the edit scene Stop goes back to.
            (Some((was, false)), true) => self.held = Some(was),
            _ => {}
        }
        let left = last.map(|(was, _)| was).filter(|&was| was != id)?;
        Some(left)
    }

    /// The warm-up alive as `token` made mesh `id`.
    pub(crate) fn warmed(&mut self, token: &Arc<()>, id: MeshId) {
        self.warmup(token).meshes.insert(id);
    }

    /// The notes of the warm-up alive as `token`, made on its first.
    fn warmup(&mut self, token: &Arc<()>) -> &mut maps::Warmed {
        let at = (self.warmed.iter()).position(|w| w.token.as_ptr() == Arc::as_ptr(token));
        let at = at.unwrap_or_else(|| {
            self.warmed.push(maps::Warmed::new(token));
            self.warmed.len() - 1
        });
        &mut self.warmed[at]
    }

    /// The warm-ups still alive; a dropped one's notes go.
    fn warming(&mut self) -> &[maps::Warmed] {
        self.warmed.retain(|w| w.token.strong_count() > 0);
        &self.warmed
    }

    /// Scene `id`'s objects are gone: so are its particles', maps' and lightmaps'
    /// notes.
    pub(super) fn forget(&mut self, id: SceneId) {
        self.particles.remove(&id);
        self.maps.remove(&id);
        self.lightmaps.remove(&id);
    }
}

/// What a release freed: meshes, and whether any held cluster ranges.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Released {
    pub meshes: u32,
    pub clustered: bool,
}

impl Renderer {
    /// A Mesh-mode emitter of scene `id` names mesh `handle`.
    pub(crate) fn note_particle_mesh(&mut self, id: SceneId, handle: MeshHandle) {
        let residency = &mut self.retained.residency;
        residency.particles.entry(id).or_default().insert(handle);
    }

    /// Close a main-view frame of `scene` (`Renderer::render`): settle what stays
    /// resident, then the frame's counters, so they read what is left.
    pub(crate) fn finish_main_frame(&mut self, view: &crate::RenderView, scene: &Scene) {
        self.settle_residency(scene);
        self.finish_counters(view);
    }

    /// The end of a main-view frame that drew `scene`: on a load that replaced
    /// the World, drop the scene left and release what no kept scene uses.
    fn settle_residency(&mut self, scene: &Scene) {
        if self.is_aside() {
            return;
        }
        let residency = &mut self.retained.residency;
        let Some(left) = residency.drew(scene.id(), scene.is_playing()) else {
            return;
        };
        if residency.held != Some(left) {
            self.forget_scene(left);
        }
        let released = self.release_unused_meshes();
        if released.clustered {
            self.retry_refused(scene);
        }
        self.release_unused_textures(scene);
        self.release_unused_lightmaps();
    }

    /// Drop scene `id`'s retained objects, records and param buffers.
    pub(super) fn forget_scene(&mut self, id: SceneId) {
        if self.retained.drop_scene(id) {
            self.materials.release_scene(id);
        }
        self.retained.residency.forget(id);
    }

    /// Release every GPU mesh no kept scene's objects or mesh emitters name, and no
    /// pending warm-up made.
    pub(crate) fn release_unused_meshes(&mut self) -> Released {
        let retained = &mut self.retained;
        let objects = retained.scenes.iter().flat_map(|(_, s)| s.objects.values());
        let particles = retained.residency.particles.values().flatten();
        let used: BTreeSet<MeshHandle> =
            objects.map(|o| o.mesh).chain(particles.copied()).collect();
        let mut used: BTreeSet<MeshId> = (used.into_iter())
            .map(|h| retained.meshes.id(h).clone())
            .collect();
        let warming = retained.residency.warming().iter();
        used.extend(warming.flat_map(|w| &w.meshes).cloned());
        let gone: Vec<MeshId> = (self.gpu_meshes.keys())
            .filter(|id| !used.contains(id))
            .cloned()
            .collect();
        let mut released = Released::default();
        for id in gone {
            let mesh = self.gpu_meshes.remove(&id).expect("listed above");
            released.meshes += 1;
            released.clustered |= mesh.clusters.is_some();
            self.retained.residency.refused.remove(&id);
        }
        released
    }

    /// Lease clusters again for each refused mesh the main `scene` draws, at the
    /// revision resident.
    fn retry_refused(&mut self, scene: &Scene) {
        let residency = &self.retained.residency;
        if residency.refused.is_empty() {
            return;
        }
        let Some(rs) = self.retained.scene(scene.id()) else {
            return;
        };
        let mut sources: BTreeMap<MeshId, u32> = BTreeMap::new();
        for (&entity, object) in &rs.objects {
            let id = self.retained.meshes.id(object.mesh);
            if residency.refused.contains(id) {
                sources.entry(id.clone()).or_insert(entity);
            }
        }
        for (id, entity) in sources {
            let Some(mesh) = scene.world.mesh(entity) else {
                continue;
            };
            let resident = self.gpu_meshes.get(&id).map(|g| g.revision);
            if resident == Some(mesh.revision) {
                self.attach_clusters(&id, (mesh.clusters(), &mesh.indices));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Residency;
    use rusty_scene::SceneId;

    #[test]
    fn a_new_identity_leaves_the_last_and_play_holds_the_edit_scene() {
        let (a, b, c) = (SceneId::next(), SceneId::next(), SceneId::next());
        let mut r = Residency::default();
        assert_eq!(r.drew(a, false), None, "the first frame leaves nothing");
        assert_eq!(r.drew(a, true), None, "Play keeps the identity");
        assert_eq!(r.held, Some(a), "Play on a drawn edit scene holds it");
        assert_eq!(r.drew(b, true), Some(a), "a play-mode load leaves it");
        assert_eq!(r.held, Some(a), "but it stays held");
        assert_eq!(r.drew(c, true), Some(b));
        assert_eq!(r.drew(a, false), Some(c), "Stop puts the edit scene back");
        assert_eq!(r.held, None, "and holds nothing more");
    }

    #[test]
    fn a_play_entry_whose_first_tick_loaded_a_scene_holds_the_edit_scene() {
        let (edit, level) = (SceneId::next(), SceneId::next());
        let mut r = Residency::default();
        r.drew(edit, false);
        assert_eq!(r.drew(level, true), Some(edit), "Start() loaded a level");
        assert_eq!(r.held, Some(edit), "Stop goes back to the edit scene");
    }

    #[test]
    fn a_game_playing_from_its_first_frame_holds_nothing() {
        let (a, b) = (SceneId::next(), SceneId::next());
        let mut r = Residency::default();
        assert_eq!(r.drew(a, true), None);
        assert_eq!(r.drew(b, true), Some(a));
        assert_eq!(r.held, None, "a player build's first level is not held");
    }
}

mod maps;

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod edges_gpu_tests;

#[cfg(test)]
mod maps_gpu_tests;

#[cfg(test)]
mod holders_gpu_tests;
