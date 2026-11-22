use std::collections::BTreeMap;

use super::PrefabData;
use rusty_components::{Entity, EntityRefs};
use rusty_scene::Scene;

pub fn extract_prefab(scene: &Scene, root_id: u32) -> Option<PrefabData> {
    if !scene.world.contains(root_id) {
        return None;
    }
    // Bones are rebuilt from the model, so a prefab saves only what was authored on them.
    let mut docs = collect_subtree(scene, root_id)
        .into_iter()
        .map(|id| scene.world.saved_document(id))
        .collect::<Option<Vec<_>>>()?;
    rusty_scene::skeleton::strip_bones(&mut docs);

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
        for key in crate::authoring::material::entity_keys(&entity) {
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

fn collect_subtree(scene: &Scene, root_id: u32) -> Vec<u32> {
    scene.world.self_and_descendants(root_id).collect()
}

/// Any `prefab_link` is dropped: extracting a linked instance bakes it into a flat prefab with no nested links.
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
