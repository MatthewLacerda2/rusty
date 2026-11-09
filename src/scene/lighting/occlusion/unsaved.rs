//! Materials a bake reads that its scene file lacks (#1958): the bake
//! fingerprints the live library, so a material a script defined in memory and
//! never saved makes a fresh load of the file read the bake as stale.

use std::collections::{BTreeMap, BTreeSet};

use super::gather::{as_occluder, drawn_runs, takes_part};
use crate::components::MaterialAsset;
use crate::scene::Scene;

/// The material keys the baked meshes draw that a fresh load of a scene file
/// whose library is `saved` reads otherwise as an occluder ([`as_occluder`]; a key
/// `saved` lacks reads as the engine's default). The bake fingerprints the live
/// library, so those meshes will not match it on that load (#1958): a material a
/// script defined in memory and the file never got.
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
