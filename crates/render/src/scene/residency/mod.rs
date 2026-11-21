use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::MeshHandle;
use crate::{MeshId, Renderer};
use rusty_asset::MapRef;
use rusty_scene::{Scene, SceneId};

#[derive(Default)]
pub(crate) struct Residency {
    main: Option<(SceneId, bool)>,
    held: Option<SceneId>,
    particles: BTreeMap<SceneId, BTreeSet<MeshHandle>>,
    /// Meshes resident without clusters because the pool was full.
    pub(crate) refused: BTreeSet<MeshId>,
    warmed: Vec<maps::Warmed>,
    maps: BTreeMap<SceneId, BTreeSet<MapRef>>,
    lightmaps: BTreeMap<SceneId, [Vec<String>; 3]>,
    notes: notes::Gate,
}

impl Residency {
    fn drew(&mut self, id: SceneId, playing: bool) -> Option<SceneId> {
        let last = self.main.replace((id, playing));
        match (last, playing) {
            (_, false) => self.held = None,
            (Some((was, false)), true) => self.held = Some(was),
            _ => {}
        }
        let left = last.map(|(was, _)| was).filter(|&was| was != id)?;
        Some(left)
    }

    fn enters_play(&self, scene: &Scene) -> bool {
        scene.is_playing() && self.main == Some((scene.id(), false))
    }

    pub(crate) fn warmed(&mut self, token: &Arc<()>, id: MeshId) {
        self.warmup(token).meshes.insert(id);
    }

    fn warmup(&mut self, token: &Arc<()>) -> &mut maps::Warmed {
        let at = (self.warmed.iter()).position(|w| w.token.as_ptr() == Arc::as_ptr(token));
        let at = at.unwrap_or_else(|| {
            self.warmed.push(maps::Warmed::new(token));
            self.warmed.len() - 1
        });
        &mut self.warmed[at]
    }

    fn warming(&mut self) -> &[maps::Warmed] {
        self.warmed.retain(|w| w.token.strong_count() > 0);
        &self.warmed
    }

    pub(super) fn forget(&mut self, id: SceneId) {
        self.particles.remove(&id);
        self.maps.remove(&id);
        self.lightmaps.remove(&id);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Released {
    pub meshes: u32,
    pub clustered: bool,
}

impl Renderer {
    pub(crate) fn note_particle_mesh(&mut self, id: SceneId, handle: MeshHandle) {
        let residency = &mut self.retained.residency;
        residency.particles.entry(id).or_default().insert(handle);
    }

    pub(crate) fn finish_main_frame(&mut self, view: &crate::RenderView, scene: &Scene) {
        self.settle_residency(scene);
        self.finish_counters(view);
    }

    fn settle_residency(&mut self, scene: &Scene) {
        if self.is_aside() {
            return;
        }
        if self.retained.residency.enters_play(scene) {
            self.note_held_maps(scene);
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

    pub(super) fn forget_scene(&mut self, id: SceneId) {
        if self.retained.drop_scene(id) {
            self.materials.release_scene(id);
        }
        self.retained.residency.forget(id);
    }

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
pub(crate) mod notes;

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod edges_gpu_tests;

#[cfg(test)]
mod maps_gpu_tests;

#[cfg(test)]
mod holders_gpu_tests;

#[cfg(test)]
mod held_gpu_tests;

#[cfg(test)]
mod notes_gpu_tests;
