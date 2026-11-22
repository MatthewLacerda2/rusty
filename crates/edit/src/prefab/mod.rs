//! A *prefab* is a saved GameObject subtree in a `.prefab` asset. Ids are local (0-based, root = 0) so the file is self-contained; on instantiate they become fresh sequential scene ids.
//! Only values are saved, never GPU buffers: meshes rehydrate from their reference. An imported model is a source too (Unity's model prefab).
//! stored-format: the `.prefab` asset (`PrefabData`, instances and overrides)
//! stored-format: the `.prefab` asset (`PrefabData`, instances and overrides)

pub mod apply;
pub mod diff;
mod extract;
pub use rusty_scene::prefab_link::instance;
pub mod link;
pub mod live;
mod merge;
pub mod overrides;
mod pass;
mod pointer;
pub mod restamp;
pub mod source;
mod structure;

pub use extract::extract_prefab;
use merge::{merge_materials, rename_entity_material, Merge};

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use rusty_components::{Entity, EntityRefs, MaterialAsset, PrefabLink};
use rusty_scene::rehydrate::{rehydrate_in, Imports};
use rusty_scene::skeleton::{take_bone_parents, take_bone_refs, BoneRef, Hung};
use rusty_scene::Scene;

pub use rusty_scene::prefab_link::{is_prefab_path, PREFAB_EXTENSION};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrefabData {
    /// Always `0` as `extract_prefab` produces it, stored so the format does not bake that in.
    pub root: u32,
    pub entities: Vec<Entity>,
    pub materials: BTreeMap<String, MaterialAsset>,
}

pub fn instantiate_prefab(scene: &mut Scene, prefab: &PrefabData, parent: Option<u32>) -> u32 {
    let imports = &mut Imports::default();
    stamp_prefab(scene, prefab, parent, None, Merge::Uniquify, imports).0
}

pub fn instantiate_prefab_linked(
    scene: &mut Scene,
    prefab: &PrefabData,
    parent: Option<u32>,
    source: &str,
) -> u32 {
    let merge = Merge::for_source(source);
    let imports = &mut Imports::default();
    stamp_prefab(scene, prefab, parent, Some(source), merge, imports).0
}

fn stamp_prefab(
    scene: &mut Scene,
    prefab: &PrefabData,
    parent: Option<u32>,
    link_source: Option<&str>,
    merge: Merge,
    imports: &mut Imports,
) -> (u32, Vec<String>) {
    let (renames, added) = merge_materials(scene, &prefab.materials, merge);
    let base = scene.world.next_id();
    let mut new_root = base + prefab.root;
    let mut bone_parents = Vec::new();
    let mut bone_refs = Vec::new();
    let nodes = match link_source.is_some_and(source::is_model_source) {
        true => source::node_paths(prefab),
        false => BTreeMap::new(),
    };
    for entity in &prefab.entities {
        let link = link_source.map(|source| PrefabLink {
            source: source.to_string(),
            local_id: entity.id,
            node: nodes.get(&entity.id).cloned(),
            ..Default::default()
        });
        let stamped = stamp_entity(entity, |l| base + l, &renames, link, imports);
        if stamped.entity.parent_id.is_none() {
            new_root = stamped.entity.id;
        }
        bone_parents.extend(stamped.hung);
        bone_refs.extend(stamped.bone_refs);
        scene.world.insert_entity(stamped.entity);
    }
    scene.hang_on_skeletons(bone_parents, bone_refs);

    if let Some(parent_id) = parent {
        let _ = scene.set_parent(new_root, Some(parent_id));
    }
    scene.update_all_colliders();
    if link_source.is_some() {
        let ids = prefab.entities.iter().map(|e| base + e.id);
        live::take_baselines(&mut scene.world, ids.collect::<Vec<_>>());
    }
    (new_root, added)
}

struct Stamped {
    entity: Entity,
    hung: Vec<Hung>,
    bone_refs: Vec<BoneRef>,
}

fn stamp_entity(
    entity: &Entity,
    id_of: impl Fn(u32) -> u32,
    renames: &BTreeMap<String, String>,
    link: Option<PrefabLink>,
    imports: &mut Imports,
) -> Stamped {
    let mut entity = entity.clone();
    entity.id = id_of(entity.id);
    entity.parent_id = entity.parent_id.map(&id_of);
    entity.children = entity.children.iter().map(|&c| id_of(c)).collect();
    entity.remap_refs(&|r| Some(id_of(r)));
    rename_entity_material(&mut entity, renames);
    rehydrate_in(&mut entity, imports);
    entity.prefab_link = link;
    let one = std::slice::from_mut(&mut entity);
    let hung = take_bone_parents(one);
    let bone_refs = take_bone_refs(one);
    Stamped {
        entity,
        hung,
        bone_refs,
    }
}

pub fn save_prefab(scene: &Scene, root_id: u32, path: &str) -> Result<(), String> {
    let prefab = extract_prefab(scene, root_id)
        .ok_or_else(|| format!("SavePrefab: no entity with id {root_id}"))?;
    write_prefab_file(&prefab, path)
}

/// Refuses a prefab whose entities draw a library mesh: it lives in its scene only, so an instance elsewhere would draw nothing.
pub fn write_prefab_file(prefab: &PrefabData, path: &str) -> Result<(), String> {
    if let Some((entity, name)) = scene_mesh_user(prefab) {
        return Err(format!(
            "entity '{}' draws the scene mesh {name:?}, which a prefab cannot hold: it lives \
             in its scene only (give it a primitive or a model first)",
            entity.name
        ));
    }
    let json = serde_json::to_string_pretty(prefab)
        .map_err(|e| format!("Failed to serialize prefab: {e}"))?;
    // lint: allow(asset-write) a scene, prefab or lighting document, read whole on load
    rusty_core::fs::write_atomic(std::path::Path::new(path), json.as_bytes())
        .map_err(|e| format!("Failed to write prefab file: {e}"))
}

fn scene_mesh_user(prefab: &PrefabData) -> Option<(&Entity, &str)> {
    use rusty_scene::mesh_library::scene_mesh_users;
    prefab.entities.iter().find_map(|e| {
        let mut names = scene_mesh_users(e.mesh.as_ref(), e.collider.as_ref());
        Some((e, names.next()?))
    })
}

pub fn read_prefab_file(path: &str) -> Result<PrefabData, String> {
    let json =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read prefab file: {e}"))?;
    let mut prefab: PrefabData =
        serde_json::from_str(&json).map_err(|e| format!("Failed to deserialize prefab: {e}"))?;
    crate::authoring::fields::hold_entities(&mut prefab.entities);
    Ok(prefab)
}

pub fn load_and_instantiate(
    scene: &mut Scene,
    path: &str,
    parent: Option<u32>,
) -> Result<u32, String> {
    let prefab = read_prefab_file(path)?;
    Ok(instantiate_prefab(scene, &prefab, parent))
}

pub fn load_and_instantiate_linked(
    scene: &mut Scene,
    path: &str,
    parent: Option<u32>,
) -> Result<u32, String> {
    let prefab = read_prefab_file(path)?;
    Ok(instantiate_prefab_linked(scene, &prefab, parent, path))
}

#[cfg(test)]
#[path = "tests.rs"]
mod prefab_tests;
