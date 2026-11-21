//! crates/edit/src/load/mod.rs — the swap half of a play-mode scene
//! load (#432): replace the World with a document read from its files, carrying the
//! `DontDestroyOnLoad` survivors across. The request and the survivors' marks are
//! the base's (`rusty_scene::runtime::load`); the swap applies the document through
//! `apply_rehydrated`, which runs the authoring holds, so it sits on the edit side.
//!
//! Survivors keep their ids, because scripts, timers and the UI event system hold
//! them. An incoming entity whose id a survivor already holds is moved to a fresh id
//! instead (`make_room`), its references rewritten with it through the one id-field
//! walk; a survivor's references to the entities left behind are cleared through the
//! same walk, and what the scene keys by entity id outside the World (the selection,
//! the lightmaps, the baked PVS) follows the moves through `Scene::renumber`.

pub mod prepare;

#[cfg(test)]
mod load_tests;
#[cfg(test)]
mod renumber_tests;
#[cfg(test)]
mod repin_tests;
#[cfg(test)]
mod swap_refs_tests;

use std::collections::{BTreeMap, BTreeSet};

use crate::serialize::apply_rehydrated;
use prepare::PreparedScene;
use rusty_components::EntityRefs;
use rusty_scene::serialize::SceneData;
use rusty_scene::{Entity, MaterialAsset, Scene};

/// Replace `scene`'s World with `data` (the document read from `path`), carrying
/// `survivors` across unchanged: same ids, same components, same order among
/// themselves, appended after the incoming entities. A survivor whose parent does
/// not survive becomes a root. The materials survivors reference come along
/// unless the incoming library already defines that name, and so do the library
/// meshes they draw (#2059). Persistent decals and the survivors' ragdoll state
/// (#1895) come along (#1518, `scene::decal::persist`); the rest, and queued
/// destroys of the outgoing scene, are dropped, and the scene takes a fresh
/// runtime identity — it is a different scene now.
pub fn swap_in(
    scene: &mut Scene,
    data: SceneData,
    path: &str,
    survivors: &BTreeSet<u32>,
) -> Result<(), String> {
    swap_in_prepared(scene, PreparedScene::from_data(data, path), survivors)
}

/// [`swap_in`] for a scene already prepared from its files: the activation half of
/// a load ([`prepare`], #2123). Only what depends on the survivors and the live
/// scene happens here.
pub fn swap_in_prepared(
    scene: &mut Scene,
    prepared: PreparedScene,
    survivors: &BTreeSet<u32>,
) -> Result<(), String> {
    let PreparedScene {
        path: _,
        mut document,
        sidecars,
        baked_navmesh: _, // activation's, taken before (`app::scene_load`)
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
        // The outgoing scene is destroyed: what a survivor named in it is gone,
        // as after a despawn (#1264), and its id may now be an incoming entity's.
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
    scene.renumber(&moved);
    loaded
}

/// The library meshes (#2059) `carried` use (a Mesh, or a Mesh collider's kept
/// copy), to come along like their materials.
fn carried_meshes(scene: &Scene, carried: &[Entity]) -> rusty_scene::mesh_library::MeshLibrary {
    use rusty_scene::mesh_library::scene_mesh_users;
    let names = carried
        .iter()
        .flat_map(|e| scene_mesh_users(e.mesh.as_ref(), e.collider.as_ref()));
    let found = names.filter_map(|n| Some((n.to_string(), scene.meshes.get(n)?.clone())));
    found.collect()
}

/// Move every incoming entity whose id is in `taken` to a fresh id past both the
/// document's ids and `taken`, in document order (so the result is a pure function of
/// the inputs), rewriting parent, child and component references to match. Returns
/// the moves, old id → new, which the scene's other id-keyed fields follow once the
/// document is in (`Scene::renumber`).
pub(crate) fn make_room(data: &mut SceneData, taken: &BTreeSet<u32>) -> BTreeMap<u32, u32> {
    let mut next = data
        .entities
        .iter()
        .map(|e| e.id + 1)
        .chain(taken.iter().map(|&id| id + 1))
        .fold(data.next_entity_id, u32::max);
    let mut moved = BTreeMap::new();
    for entity in &data.entities {
        if taken.contains(&entity.id) {
            moved.insert(entity.id, next);
            next += 1;
        }
    }
    data.next_entity_id = next;
    if moved.is_empty() {
        return moved;
    }
    let remap = |id: u32| moved.get(&id).copied().unwrap_or(id);
    for entity in &mut data.entities {
        entity.id = remap(entity.id);
        entity.parent_id = entity.parent_id.map(remap);
        for child in &mut entity.children {
            *child = remap(*child);
        }
        entity.remap_refs(&|r| Some(remap(r)));
    }
    moved
}
