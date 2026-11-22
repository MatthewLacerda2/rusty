pub mod prepare;

#[cfg(test)]
mod load_tests;
#[cfg(test)]
mod renumber_tests;
#[cfg(test)]
mod repin_tests;
#[cfg(test)]
mod stale_ids_tests;
#[cfg(test)]
mod swap_refs_tests;

use std::collections::{BTreeMap, BTreeSet};

use crate::serialize::apply_rehydrated;
use prepare::PreparedScene;
use rusty_components::EntityRefs;
use rusty_scene::runtime::renumber::Moves;
use rusty_scene::serialize::SceneData;
use rusty_scene::{Entity, MaterialAsset, Scene};

pub fn swap_in(
    scene: &mut Scene,
    data: SceneData,
    path: &str,
    survivors: &BTreeSet<u32>,
) -> Result<(), String> {
    swap_in_prepared(scene, PreparedScene::from_data(data, path), survivors)
}

pub fn swap_in_prepared(
    scene: &mut Scene,
    prepared: PreparedScene,
    survivors: &BTreeSet<u32>,
) -> Result<(), String> {
    let PreparedScene {
        path: _,
        mut document,
        sidecars,
        baked_navmesh: _,
    } = prepared;
    let carried: Vec<Entity> = scene
        .entity_ids()
        .into_iter()
        .filter(|id| survivors.contains(id))
        .filter_map(|id| scene.world.entity_document(id))
        .collect();
    let materials: BTreeMap<String, MaterialAsset> = carried
        .iter()
        .filter_map(|e| e.material.as_ref())
        .flat_map(|m| &m.materials)
        .filter_map(|key| Some((key.clone(), scene.materials.get(key)?.clone())))
        .collect();

    let meshes = carried_meshes(scene, &carried);
    let decals = scene.take_persistent_decals(survivors);
    let mut ragdolls = std::mem::take(&mut scene.ragdolls);
    ragdolls.retain(|id| survivors.contains(&id));
    let moved = make_room(document.data_mut(), survivors);
    apply_rehydrated(scene, document);
    scene.ragdolls = ragdolls;
    for mut entity in carried {
        if entity.parent_id.is_some_and(|p| !survivors.contains(&p)) {
            entity.parent_id = None;
        }
        entity.forget(&|r| !survivors.contains(&r));
        scene.world.insert_entity(entity);
    }
    for (name, material) in materials {
        scene.materials.entry(name).or_insert(material);
    }
    for (name, mesh) in meshes {
        scene.meshes.entry(name).or_insert(mesh);
    }
    scene.sync_scene_meshes();
    scene.persistent.retain(|id| survivors.contains(id));
    scene.history_cuts.retain(|id| survivors.contains(&id));
    scene.pending_destroy.retain(|id| survivors.contains(id));
    scene.land_persistent_decals(decals);
    scene.renew_id();
    rusty_scene::nav_settings::apply_navmesh(scene, sidecars.navmesh);
    let loaded = rusty_scene::lighting::io::apply_lighting_sidecar(scene, sidecars.lighting);
    scene.renumber_sidecars(&moved);
    loaded
}

fn carried_meshes(scene: &Scene, carried: &[Entity]) -> rusty_scene::mesh_library::MeshLibrary {
    use rusty_scene::mesh_library::scene_mesh_users;
    let names = carried
        .iter()
        .flat_map(|e| scene_mesh_users(e.mesh.as_ref(), e.collider.as_ref()));
    let found = names.filter_map(|n| Some((n.to_string(), scene.meshes.get(n)?.clone())));
    found.collect()
}

pub(crate) fn make_room(data: &mut SceneData, taken: &BTreeSet<u32>) -> Moves {
    let moves = Moves::out_of_the_way(data, taken);
    data.next_entity_id = moves.next_id();
    if !moves.is_empty() {
        let remap = |id: u32| moves.to(id);
        for entity in &mut data.entities {
            entity.id = remap(entity.id);
            entity.parent_id = entity.parent_id.map(remap);
            for child in &mut entity.children {
                *child = remap(*child);
            }
            entity.remap_refs(&|r| Some(remap(r)));
        }
    }
    moves.rename_document(data);
    moves
}
