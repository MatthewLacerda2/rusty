//! Extracting a live subtree into a [`PrefabData`] (split from `prefab/mod.rs` to
//! stay under the size cap).

use std::collections::BTreeMap;

use super::PrefabData;
use crate::components::{Entity, EntityRefs};
use crate::scene::Scene;

/// Walk `root_id` + its descendants, deep-clone their bundles, remap to local
/// (0-based) ids — rewriting `parent_id`/`children` through the map and detaching
/// the root's own parent — and slice out every material the subtree names.
/// Returns `None` if `root_id` is not a live entity.
pub fn extract_prefab(scene: &Scene, root_id: u32) -> Option<PrefabData> {
    if !scene.world.contains(root_id) {
        return None;
    }
    // Bones are rebuilt from the model, so a prefab saves only what was authored
    // on them (#453) — the same strip a scene save does.
    let mut docs = collect_subtree(scene, root_id)
        .into_iter()
        .map(|id| scene.world.entity_document(id))
        .collect::<Option<Vec<_>>>()?;
    crate::scene::skeleton::strip_bones(&mut docs);

    // Stable old-id -> local-id map (root = 0, then descendants in subtree order).
    let local: BTreeMap<u32, u32> = docs
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id, i as u32))
        .collect();

    let mut entities = Vec::with_capacity(docs.len());
    let mut materials = BTreeMap::new();
    for mut entity in docs {
        let is_root = entity.id == root_id;
        remap_entity(&mut entity, &local, is_root);
        // Every key the entity names, slots and particles alike (#1576).
        for key in crate::scene::authoring::material::entity_keys(&entity) {
            if let Some(asset) = scene.materials.get(&key) {
                materials.insert(key, asset.clone());
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

/// Depth-first subtree ids starting at `root_id` (root first, children in declared
/// order), so extract order is deterministic and parent precedes child.
fn collect_subtree(scene: &Scene, root_id: u32) -> Vec<u32> {
    scene.world.self_and_descendants(root_id).collect()
}

/// Rewrite one entity's identity into the local id space: its own `id`, its
/// `children`, its component references (dropped when outside the subtree), and its
/// `parent_id` (cleared for the root, which detaches from its scene). The `pending_material` staging slot is never
/// part of a freshly cloned runtime entity, so nothing to scrub there. Any
/// `prefab_link` is **dropped**: extracting an already-linked instance bakes it down
/// into a fresh flat prefab — the new `.prefab` is a plain subtree with no nested
/// links back to whatever the instance came from (#216 scope).
fn remap_entity(entity: &mut Entity, local: &BTreeMap<u32, u32>, is_root: bool) {
    entity.id = local[&entity.id];
    entity.prefab_link = None;
    entity.remap_refs(&|r| local.get(&r).copied());
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
