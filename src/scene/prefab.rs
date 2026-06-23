//! src/scene/prefab.rs — Prefabs v1 (unpacked instantiate).
//!
//! A *prefab* is a saved **GameObject subtree** — a root entity plus its whole
//! child hierarchy, every component configured, every asset reference intact — kept
//! in its own `.prefab` asset so it can be stamped into any scene as an independent
//! copy. v1 is **unpacked**: each instance is a plain copy with no live link back to
//! the asset (linked instances + overrides are the deferred follow-up #216).
//!
//! Like `SceneData`, a `PrefabData` is a references-only document: it stores entity
//! component VALUES and the material-library slice the subtree references, never GPU
//! buffers — meshes are rehydrated from their `primitive_type`/`asset_ref` on
//! instantiate via the shared `serialize::rehydrate_entity_mesh`.
//!
//! Ids are **local** (0-based, root = 0) so the file is self-contained and id-stable
//! on disk regardless of the scene it came from. On instantiate they are remapped to
//! fresh, sequential, deterministic scene ids (no wall-clock / RNG).
//!
//! The two canonical verbs (`extract_prefab` / `instantiate_prefab`) are re-exported
//! through `scene::authoring`, the one shared structural-authoring module, so the
//! editor and the API can never drift.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::components::{Entity, MaterialAsset};
use crate::scene::serialize::rehydrate_entity_mesh;
use crate::scene::Scene;

/// The one standardised prefab file extension (JSON inside) — a distinct extension
/// from `.scene` so the inspector dispatch and content browser recognise it cleanly.
pub const PREFAB_EXTENSION: &str = "prefab";

/// True if `path` looks like a prefab file (`.prefab`).
pub fn is_prefab_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(PREFAB_EXTENSION))
        .unwrap_or(false)
}

/// A saved GameObject subtree: the root's local id, the root + descendants with
/// LOCAL (0-based) ids, and the material-library slice the subtree references.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrefabData {
    /// Local id of the root entity (always `0` as produced by `extract_prefab`,
    /// but stored explicitly so the format does not bake that assumption in).
    pub root: u32,
    /// Root + descendants, in subtree order, carrying local ids with `parent_id` /
    /// `children` rewritten into the local id space.
    pub entities: Vec<Entity>,
    /// The materials referenced by the subtree, keyed by their library name.
    pub materials: BTreeMap<String, MaterialAsset>,
}

/// Walk `root_id` + its descendants, deep-clone their bundles, remap to local
/// (0-based) ids — rewriting `parent_id`/`children` through the map and detaching
/// the root's own parent — and slice out the materials the subtree references.
/// Returns `None` if `root_id` is not a live entity.
pub fn extract_prefab(scene: &Scene, root_id: u32) -> Option<PrefabData> {
    scene.get_entity(root_id)?; // existence check
    let order = collect_subtree(scene, root_id);

    // Stable old-id -> local-id map (root = 0, then descendants in subtree order).
    let local: BTreeMap<u32, u32> = order
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i as u32))
        .collect();

    let mut entities = Vec::with_capacity(order.len());
    let mut materials = BTreeMap::new();
    for &old_id in &order {
        let mut entity = (*scene.get_entity(old_id)?).clone();
        remap_entity(&mut entity, &local, old_id == root_id);
        if let Some(mat) = &entity.material {
            if let Some(asset) = scene.materials.get(&mat.material) {
                materials.insert(mat.material.clone(), asset.clone());
            }
        }
        entities.push(entity);
    }

    Some(PrefabData {
        root: 0,
        entities,
        materials,
    })
}

/// Depth-first subtree ids starting at `root_id` (root first), following the live
/// `children` lists. Used so extract order is deterministic and parent precedes
/// child.
fn collect_subtree(scene: &Scene, root_id: u32) -> Vec<u32> {
    let mut order = Vec::new();
    let mut stack = vec![root_id];
    while let Some(id) = stack.pop() {
        order.push(id);
        if let Some(entity) = scene.get_entity(id) {
            // Push children reversed so they come out in declared order.
            for &child in entity.children.iter().rev() {
                stack.push(child);
            }
        }
    }
    order
}

/// Rewrite one entity's identity into the local id space: its own `id`, its
/// `children`, and its `parent_id` (cleared for the root, which detaches from the
/// scene it was extracted from). The `pending_material` migration carrier is never
/// part of a freshly cloned runtime entity, so nothing to scrub there. Any
/// `prefab_link` is **dropped**: extracting an already-linked instance bakes it down
/// into a fresh flat prefab — the new `.prefab` is a plain subtree with no nested
/// links back to whatever the instance came from (#216 scope).
fn remap_entity(entity: &mut Entity, local: &BTreeMap<u32, u32>, is_root: bool) {
    entity.id = local[&entity.id];
    entity.prefab_link = None;
    entity.children = entity
        .children
        .iter()
        .filter_map(|c| local.get(c).copied())
        .collect();
    entity.parent_id = if is_root {
        None
    } else {
        entity.parent_id.and_then(|p| local.get(&p).copied())
    };
}

/// Clone a prefab into `scene` as an independent **unpacked** copy (v1 behaviour, no
/// live link): fresh deterministic ids, materials merged into the library, the new
/// root parented under `parent` (or the scene root when `None`). Returns the new
/// root's id.
///
/// Material name collision: an identical existing entry is reused; otherwise the
/// material is inserted under a uniquified name and the instance's references are
/// rewritten to it — so an instance never silently adopts a different scene material.
pub fn instantiate_prefab(scene: &mut Scene, prefab: &PrefabData, parent: Option<u32>) -> u32 {
    stamp_prefab(scene, prefab, parent, None)
}

/// Clone a prefab into `scene` as a **linked** instance (#216): identical to
/// [`instantiate_prefab`], but every entity is stamped with a [`PrefabLink`] back to
/// `source` (the `.prefab` path) carrying its source `local_id` and an empty override
/// set. The link is what lets a later scene-load / reimport re-baseline the instance
/// against the source and re-apply the instance's recorded overrides on top.
pub fn instantiate_prefab_linked(
    scene: &mut Scene,
    prefab: &PrefabData,
    parent: Option<u32>,
    source: &str,
) -> u32 {
    stamp_prefab(scene, prefab, parent, Some(source))
}

/// Stamp one fresh copy of `prefab` into `scene`. `link_source = Some(path)` makes it
/// a linked instance (every entity gets a `PrefabLink`); `None` is the unpacked v1
/// copy. Fresh sequential ids come from the allocator (local id `l` -> `base + l`),
/// so the operation is deterministic (no wall-clock/RNG).
fn stamp_prefab(
    scene: &mut Scene,
    prefab: &PrefabData,
    parent: Option<u32>,
    link_source: Option<&str>,
) -> u32 {
    let renames = merge_materials(scene, &prefab.materials);
    let base = scene.world.next_id();
    let mut new_root = base + prefab.root;
    for entity in &prefab.entities {
        let local_id = entity.id;
        let mut entity = entity.clone();
        entity.id += base;
        entity.parent_id = entity.parent_id.map(|p| base + p);
        entity.children = entity.children.iter().map(|c| base + c).collect();
        rename_entity_material(&mut entity, &renames);
        rehydrate_entity_mesh(&mut entity);
        if let Some(source) = link_source {
            entity.prefab_link = Some(crate::components::PrefabLink {
                source: source.to_string(),
                local_id,
                overrides: std::collections::BTreeMap::new(),
            });
        }
        if entity.parent_id.is_none() {
            new_root = entity.id;
        }
        scene.world.insert_entity(entity);
    }

    // Parent the new root under the requested parent (cycle-checked by `set_parent`).
    if let Some(parent_id) = parent {
        let _ = scene.set_parent(new_root, Some(parent_id));
    }
    scene.update_all_colliders();
    new_root
}

/// Rewrite an entity's material reference through the merge rename map (a no-op when
/// the key wasn't uniquified). Shared by stamping and baseline-building so an
/// instance and its propagation baseline resolve the same library key.
fn rename_entity_material(entity: &mut Entity, renames: &BTreeMap<String, String>) {
    if let Some(mat) = &mut entity.material {
        if let Some(renamed) = renames.get(&mat.material) {
            mat.material = renamed.clone();
        }
    }
}

/// Merge a prefab's material slice into the scene library, returning the rename map
/// (prefab key -> scene key) for keys that had to be uniquified. An identical
/// existing entry is reused (no rename); a new key is inserted as-is (no rename);
/// only a *conflicting* key (same name, different data) is uniquified.
fn merge_materials(
    scene: &mut Scene,
    materials: &BTreeMap<String, MaterialAsset>,
) -> BTreeMap<String, String> {
    let mut renames = BTreeMap::new();
    for (key, asset) in materials {
        match scene.materials.get(key) {
            Some(existing) if materials_equal(existing, asset) => {} // reuse, no rename
            Some(_) => {
                let unique = uniquify(&scene.materials, key);
                scene.materials.insert(unique.clone(), asset.clone());
                renames.insert(key.clone(), unique);
            }
            None => {
                scene.materials.insert(key.clone(), asset.clone());
            }
        }
    }
    renames
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

/// Extract the subtree rooted at `root_id` and write it to `path` as pretty JSON.
/// Returns an error if the root is missing or the write fails.
pub fn save_prefab(scene: &Scene, root_id: u32, path: &str) -> Result<(), String> {
    let prefab = extract_prefab(scene, root_id)
        .ok_or_else(|| format!("SavePrefab: no entity with id {root_id}"))?;
    write_prefab_file(&prefab, path)
}

/// Serialize `prefab` and write it to `path` as pretty JSON. The single writer for the
/// `.prefab` format, shared by `save_prefab` and the apply-to-source verbs (#268).
pub fn write_prefab_file(prefab: &PrefabData, path: &str) -> Result<(), String> {
    let json = serde_json::to_string_pretty(prefab)
        .map_err(|e| format!("Failed to serialize prefab: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("Failed to write prefab file: {e}"))
}

/// Read and deserialize a `.prefab` document from `path`. Shared by every loader so
/// the read/parse error wording stays in one place.
pub fn read_prefab_file(path: &str) -> Result<PrefabData, String> {
    let json =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read prefab file: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("Failed to deserialize prefab: {e}"))
}

/// Load a `.prefab` from `path` and instantiate it into `scene` under `parent` (or
/// the scene root when `None`) as an **unpacked** copy (v1). Returns the new root id.
pub fn load_and_instantiate(
    scene: &mut Scene,
    path: &str,
    parent: Option<u32>,
) -> Result<u32, String> {
    let prefab = read_prefab_file(path)?;
    Ok(instantiate_prefab(scene, &prefab, parent))
}

/// Load a `.prefab` from `path` and instantiate it into `scene` under `parent` as a
/// **linked** instance (#216): the new entities carry a `PrefabLink` back to `path`,
/// so the instance tracks the source on later loads/reimports. Returns the new root.
pub fn load_and_instantiate_linked(
    scene: &mut Scene,
    path: &str,
    parent: Option<u32>,
) -> Result<u32, String> {
    let prefab = read_prefab_file(path)?;
    Ok(instantiate_prefab_linked(scene, &prefab, parent, path))
}

#[cfg(test)]
#[path = "prefab_tests.rs"]
mod prefab_tests;
