//! Prefabs v1 (unpacked instantiate).
//!
//! A *prefab* is a saved **GameObject subtree** — a root entity plus its whole child
//! hierarchy, every component configured, every asset reference intact — kept in its
//! own `.prefab` asset so it can be stamped into any scene as an independent copy. v1
//! is **unpacked**: each instance is a plain copy with no live link back to the asset;
//! a **linked** instance (#216, [`instantiate_prefab_linked`]) keeps one, its edits
//! kept as overrides (#1527). An imported model is a source too, built from its
//! import rather than read from a file (Unity's model prefab, #1637, `source`). Like `SceneData`,
//! a `PrefabData` is references-only: component VALUES + the material-library slice,
//! never GPU buffers — meshes rehydrate from `primitive_type`/`asset_ref`.
//!
//! Ids are **local** (0-based, root = 0) so the file is self-contained and id-stable
//! on disk; on instantiate they are remapped to fresh, sequential, deterministic
//! scene ids (no wall-clock / RNG). The canonical verbs (`extract_prefab` /
//! `instantiate_prefab`) are re-exported through `scene::authoring` so editor and API
//! can never drift. Apply/link/overrides for linked instances live in submodules.

pub mod apply;
pub mod diff;
mod extract;
pub mod instance;
pub mod link;
pub mod live;
mod merge;
pub mod overrides;
mod pointer;
pub mod restamp;
pub mod source;
mod structure;

pub use extract::extract_prefab;
use merge::{merge_materials, rename_entity_material, Merge};

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::components::{Entity, EntityRefs, MaterialAsset, PrefabLink};
use crate::scene::serialize::rehydrate_entity_mesh;
use crate::scene::skeleton::{take_bone_parents, take_bone_refs, BoneRef, Hung};
use crate::scene::Scene;

/// The one standardised prefab file extension (JSON inside) — a distinct extension
/// from `.scene` so the inspector dispatch recognises it cleanly.
pub const PREFAB_EXTENSION: &str = "prefab";

/// True if `path` looks like a prefab file (`.prefab`).
pub fn is_prefab_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(PREFAB_EXTENSION))
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

/// Clone a prefab into `scene` as an independent **unpacked** copy (v1 behaviour, no
/// live link): fresh deterministic ids, materials merged into the library, the new
/// root parented under `parent` (or the scene root when `None`). Returns the new
/// root's id.
///
/// Material name collision: an identical existing entry is reused; otherwise the
/// material is inserted under a uniquified name and the instance's references are
/// rewritten to it — so an instance never silently adopts a different scene material.
pub fn instantiate_prefab(scene: &mut Scene, prefab: &PrefabData, parent: Option<u32>) -> u32 {
    stamp_prefab(scene, prefab, parent, None, Merge::Uniquify).0
}

/// Clone a prefab into `scene` as a **linked** instance (#216): identical to
/// [`instantiate_prefab`], but every entity is stamped with a [`PrefabLink`] back to
/// `source` (the `.prefab` path) carrying its source `local_id` and an empty override
/// set. The link is what lets a later scene-load / reimport re-baseline the instance
/// against the source and re-apply the instance's overrides on top.
pub fn instantiate_prefab_linked(
    scene: &mut Scene,
    prefab: &PrefabData,
    parent: Option<u32>,
    source: &str,
) -> u32 {
    let merge = Merge::for_source(source);
    stamp_prefab(scene, prefab, parent, Some(source), merge).0
}

/// Stamp one fresh copy of `prefab` into `scene`. `link_source = Some(path)` makes it
/// a linked instance (every entity gets a `PrefabLink`); `None` is the unpacked v1
/// copy. Fresh sequential ids come from the allocator (local id `l` -> `base + l`),
/// so the operation is deterministic (no wall-clock/RNG). Returns the new root and
/// the material keys the merge added to the library.
fn stamp_prefab(
    scene: &mut Scene,
    prefab: &PrefabData,
    parent: Option<u32>,
    link_source: Option<&str>,
    merge: Merge,
) -> (u32, Vec<String>) {
    let (renames, added) = merge_materials(scene, &prefab.materials, merge);
    let base = scene.world.next_id();
    let mut new_root = base + prefab.root;
    let mut bone_parents = Vec::new();
    let mut bone_refs = Vec::new();
    // A model's entities are keyed by node path too (#1637, `source`).
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
        let stamped = stamp_entity(entity, |l| base + l, &renames, link);
        if stamped.entity.parent_id.is_none() {
            new_root = stamped.entity.id;
        }
        bone_parents.extend(stamped.hung);
        bone_refs.extend(stamped.bone_refs);
        scene.world.insert_entity(stamped.entity);
    }
    scene.hang_on_skeletons(bone_parents, bone_refs);

    // Parent the new root under the requested parent (cycle-checked by `set_parent`).
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

/// One prefab entity made ready to insert ([`stamp_entity`]), with what it hangs
/// from by bone name and the bones its references name, for
/// [`Scene::hang_on_skeletons`] once every entity of the stamp is in.
struct Stamped {
    entity: Entity,
    hung: Vec<Hung>,
    bone_refs: Vec<BoneRef>,
}

/// The per-entity part of a stamp, shared by [`stamp_prefab`] and a model
/// instance taking a node its model gained (`structure`, #1665): `entity`'s id,
/// parent, children and component references moved through `id_of` (local id to
/// scene id), its materials renamed through the merge's `renames`, its mesh
/// rehydrated, `link` set, and its bone parent and bone references taken out.
fn stamp_entity(
    entity: &Entity,
    id_of: impl Fn(u32) -> u32,
    renames: &BTreeMap<String, String>,
    link: Option<PrefabLink>,
) -> Stamped {
    let mut entity = entity.clone();
    entity.id = id_of(entity.id);
    entity.parent_id = entity.parent_id.map(&id_of);
    entity.children = entity.children.iter().map(|&c| id_of(c)).collect();
    entity.remap_refs(&|r| Some(id_of(r)));
    rename_entity_material(&mut entity, renames);
    rehydrate_entity_mesh(&mut entity);
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
    // lint: allow(asset-write) a scene, prefab or lighting document, read whole on load
    std::fs::write(path, json).map_err(|e| format!("Failed to write prefab file: {e}"))
}

/// Read and deserialize a `.prefab` document from `path`, its values held through
/// their authoring ops as a scene file's are (#1418), once here rather than on
/// every stamp. Shared by every loader so the read/parse error wording stays in
/// one place.
pub fn read_prefab_file(path: &str) -> Result<PrefabData, String> {
    let json =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read prefab file: {e}"))?;
    let mut prefab: PrefabData =
        serde_json::from_str(&json).map_err(|e| format!("Failed to deserialize prefab: {e}"))?;
    crate::scene::authoring::fields::hold_entities(&mut prefab.entities);
    Ok(prefab)
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
#[path = "tests.rs"]
mod prefab_tests;
