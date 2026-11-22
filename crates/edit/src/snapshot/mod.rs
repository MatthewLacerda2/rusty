use crate::serialize::{apply_world_data, to_scene_data};
use rusty_scene::serialize::SceneData;
use rusty_scene::Scene;

pub struct SceneSnapshot {
    data: SceneData,
    probes: rusty_scene::lighting::probe::ProbeVolume,
    decals: rusty_scene::decal::DecalSet,
    inspected_reflection_probe: Option<usize>,
    id: rusty_scene::SceneId,
    disk_text: rusty_scene::io::DiskText,
    occlusion: Option<std::sync::Arc<rusty_scene::lighting::occlusion::OcclusionData>>,
    navmesh: rusty_scene::nav_settings::SavedNavmesh,
    bake_files: rusty_scene::io::BakeFiles,
}

impl SceneSnapshot {
    pub fn capture(scene: &Scene) -> Self {
        Self {
            data: to_scene_data(scene),
            probes: scene.probes.clone(),
            decals: scene.decals.clone(),
            inspected_reflection_probe: scene.inspected_reflection_probe,
            id: scene.id(),
            disk_text: scene.disk_text(),
            occlusion: (*scene.occlusion).clone(),
            navmesh: scene.navmesh.clone(),
            bake_files: scene.bake_files().clone(),
        }
    }

    pub fn restore(self, scene: &mut Scene) {
        apply_world_data(scene, self.data);
        scene.decals.restore(self.decals);
        scene.inspected_reflection_probe = self.inspected_reflection_probe;
        scene.probes = self.probes;
        scene.restore_id(self.id);
        scene.restore_disk_text(self.disk_text);
        if *scene.occlusion != self.occlusion {
            *scene.occlusion = self.occlusion;
        }
        scene.navmesh = self.navmesh;
        scene.restore_bake_files(self.bake_files);
    }
}

#[cfg(test)]
mod held_tests;
#[cfg(test)]
mod linked_tests;
#[cfg(test)]
mod tests;
