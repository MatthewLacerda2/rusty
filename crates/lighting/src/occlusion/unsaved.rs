use std::collections::{BTreeMap, BTreeSet};

use super::gather::{as_occluder, drawn_runs, takes_part};
use rusty_components::MaterialAsset;
use rusty_scene::Scene;

pub fn unsaved_materials(scene: &Scene, saved: &BTreeMap<String, MaterialAsset>) -> Vec<String> {
    let mut keys = BTreeSet::new();
    for id in scene.world.ids_with_mesh() {
        let Some(mesh) = scene.world.mesh(id) else {
            continue;
        };
        let Some(material) = scene.world.material(id) else {
            continue;
        };
        if !takes_part(scene, id, &mesh) {
            continue;
        }
        for (slot, _) in drawn_runs(scene, id, &mesh) {
            let Some(key) = material.materials.get(slot) else {
                continue;
            };
            let live = as_occluder(scene.materials.get(key));
            if live != as_occluder(saved.get(key)) {
                keys.insert(key.clone());
            }
        }
    }
    keys.into_iter().collect()
}
