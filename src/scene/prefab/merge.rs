//! How a stamp merges a prefab's material slice into the scene's library, and
//! rewrites the stamped entities' material keys to match (split from `prefab/mod.rs`
//! to stay under the size cap).

use std::collections::BTreeMap;

use crate::components::{Entity, MaterialAsset};
use crate::scene::prefab::source;
use crate::scene::Scene;

/// How a stamp merges a prefab's materials into the scene library.
#[derive(Clone, Copy)]
pub(super) enum Merge {
    /// A key the scene holds with other data is copied in as `"{key} (n)"`: a
    /// `.prefab`'s bare names (`Wood`) can collide with another source's.
    Uniquify,
    /// A key the scene holds is the scene's: a model's keys are namespaced by its
    /// path (`kit.glb::Wood`), so the scene's entry under one *is* that model's
    /// material, edited or not, as a re-import never overwrites Unity's extracted
    /// material (#1637). A propagation reads it, never replaces it.
    KeepScene,
}

impl Merge {
    /// The merge a stamp linked to `source` takes.
    pub(super) fn for_source(source: &str) -> Self {
        match source::is_model_source(source) {
            true => Merge::KeepScene,
            false => Merge::Uniquify,
        }
    }
}

/// Rewrite an entity's material references through the merge rename map (a no-op when
/// the key wasn't uniquified). Shared by stamping and baseline-building so an
/// instance and its propagation baseline resolve the same library key.
pub(super) fn rename_entity_material(entity: &mut Entity, renames: &BTreeMap<String, String>) {
    let slots = entity.material.iter_mut().flat_map(|m| &mut m.materials);
    let particles = entity.particles.iter_mut();
    let emitted = particles.filter_map(|p| p.render.material.as_mut());
    for key in slots.chain(emitted) {
        if let Some(renamed) = renames.get(key) {
            *key = renamed.clone();
        }
    }
}

/// Merge a prefab's material slice into the scene library, returning the rename map
/// (prefab key -> scene key) for keys that had to be uniquified, and the keys added.
/// An identical existing entry is reused (no rename); a new key is inserted as-is
/// (no rename); a *conflicting* key (same name, different data) is uniquified, or
/// kept as the scene holds it under [`Merge::KeepScene`].
pub(super) fn merge_materials(
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

/// Pick a fresh `"{key} (n)"` name not already present in the library.
fn uniquify(library: &BTreeMap<String, MaterialAsset>, key: &str) -> String {
    (1..)
        .map(|n| format!("{key} ({n})"))
        .find(|candidate| !library.contains_key(candidate))
        .expect("an unbounded counter always finds a free name")
}

/// Structural equality for the collision check. `MaterialAsset` is plain serde data,
/// so a serialized comparison is the simplest faithful "is this the same material".
fn materials_equal(a: &MaterialAsset, b: &MaterialAsset) -> bool {
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}
