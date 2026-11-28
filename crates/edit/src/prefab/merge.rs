use std::collections::BTreeMap;

use crate::prefab::source;
use rusty_components::{Entity, MaterialAsset};
use rusty_scene::Scene;

#[derive(Clone, Copy)]
pub(crate) enum Merge {
    /// A key the scene holds with other data is copied in as `"{key} (n)"`: a `.prefab`'s bare
    /// names (`Wood`) can collide.
    Uniquify,
    /// A model's keys are namespaced by its path (`kit.glb::Wood`), so the scene's entry under one
    /// is that model's material, edited or not: a re-import never overwrites Unity's extracted
    /// material.
    KeepScene,
}

impl Merge {
    pub(super) fn for_source(source: &str) -> Self {
        match source::is_model_source(source) {
            true => Merge::KeepScene,
            false => Merge::Uniquify,
        }
    }
}

pub(crate) fn rename_entity_material(entity: &mut Entity, renames: &BTreeMap<String, String>) {
    let slots = entity.material.iter_mut().flat_map(|m| &mut m.materials);
    let particles = entity.particles.iter_mut();
    let emitted = particles.filter_map(|p| p.render.material.as_mut());
    for key in slots.chain(emitted) {
        if let Some(renamed) = renames.get(key) {
            *key = renamed.clone();
        }
    }
}

pub(crate) fn merge_materials(
    scene: &mut Scene,
    materials: &BTreeMap<String, MaterialAsset>,
    merge: Merge,
) -> (BTreeMap<String, String>, Vec<String>) {
    let mut renames = BTreeMap::new();
    let mut added = Vec::new();
    for (key, asset) in materials {
        let key = match scene.materials.get(key) {
            None => key.clone(),
            Some(_) if matches!(merge, Merge::KeepScene) => continue,
            Some(existing) if materials_equal(existing, asset) => continue,
            Some(_) => {
                let unique = uniquify(&scene.materials, key);
                renames.insert(key.clone(), unique.clone());
                unique
            }
        };
        scene.materials.insert(key.clone(), asset.clone());
        added.push(key);
    }
    (renames, added)
}

/// The first `"{key} (n)"` the library does not hold.
pub(crate) fn uniquify<V>(library: &BTreeMap<String, V>, key: &str) -> String {
    (1..)
        .map(|n| format!("{key} ({n})"))
        .find(|candidate| !library.contains_key(candidate))
        .expect("an unbounded counter always finds a free name")
}

fn materials_equal(a: &MaterialAsset, b: &MaterialAsset) -> bool {
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}
