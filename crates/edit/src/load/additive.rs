//! An additive load: the scene is applied to a scene of its own (bones, prefab links,
//! requirements), then each entity joins the World under a fresh id past all it handed out, in
//! document order.

use rusty_components::EntityRefs;
use rusty_scene::runtime::membership::file::SceneFile;
use rusty_scene::runtime::membership::SceneSettings;
use rusty_scene::runtime::renumber::Moves;
use rusty_scene::serialize::SceneData;
use rusty_scene::{Entity, Scene};

use super::prepare::{staged, PreparedScene, SceneSidecars};
use std::collections::BTreeMap;

use crate::prefab::merge::{merge_materials, rename_entity_material, uniquify, Merge};

pub struct Joined {
    pub ids: Vec<u32>,
    pub roots: Vec<u32>,
    pub settings: SceneSettings,
    /// The bakes it carries but does not apply yet.
    pub skipped: Vec<&'static str>,
    /// Its layers or collision matrix differ; the World's stay.
    pub disagrees: bool,
    pub file: SceneFile,
}

pub fn join_prepared(scene: &mut Scene, prepared: PreparedScene) -> Joined {
    let PreparedScene {
        path: _,
        document,
        sidecars,
        baked_navmesh: _,
    } = prepared;
    let skipped = skipped_bakes(document.data(), &sidecars);
    let carried = carried(document.data());
    let incoming = staged(document);
    let ids = incoming.world.ids().to_vec();
    let moves = Moves::past(ids.iter().copied(), scene.world.next_id());
    let (renames, _) = merge_materials(scene, &incoming.materials, Merge::Uniquify);
    let mesh_renames = merge_meshes(scene, &incoming);
    let mut joined = Vec::with_capacity(ids.len());
    let mut roots = Vec::new();
    for id in ids {
        let Some(mut entity) = incoming.world.entity_document(id) else {
            continue;
        };
        move_entity(&mut entity, &moves);
        rename_entity_material(&mut entity, &renames);
        rename_entity_mesh(&mut entity, &mesh_renames);
        if entity.parent_id.is_none() {
            roots.push(entity.id);
        }
        joined.push(entity.id);
        scene.world.insert_entity(entity);
    }
    scene.world.bump_next_id(moves.next_id());
    scene.sync_scene_meshes();
    scene.update_all_colliders();
    let disagrees =
        incoming.layers != scene.layers || incoming.collision_matrix != scene.collision_matrix;
    let file_ids = ids_of(&incoming, &moves);
    let mut file = SceneFile::new(file_ids, carried.next_entity_id, incoming.world.next_id());
    file.materials = names_of(incoming.materials.keys(), &renames);
    file.meshes = names_of(incoming.meshes.keys(), &mesh_renames);
    file.carried = Some(Box::new(carried));
    Joined {
        ids: joined,
        roots,
        settings: incoming.settings(),
        skipped,
        disagrees,
        file,
    }
}

/// The staged scene holds the file's ids.
fn ids_of(incoming: &Scene, moves: &Moves) -> BTreeMap<u32, u32> {
    let ids = incoming.world.ids().iter();
    ids.map(|&id| (moves.to(id), id)).collect()
}

fn names_of<'a>(
    names: impl Iterator<Item = &'a String>,
    renames: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let world = |n: &String| renames.get(n).cloned().unwrap_or_else(|| n.clone());
    names.map(|n| (world(n), n.clone())).collect()
}

fn carried(data: &SceneData) -> SceneData {
    SceneData {
        entities: Vec::new(),
        next_entity_id: data.next_entity_id,
        selected_entity_id: data.selected_entity_id,
        skybox_path: data.skybox_path.clone(),
        ambient_color: data.ambient_color,
        ambient_intensity: data.ambient_intensity,
        ambient: data.ambient,
        nav_settings: data.nav_settings.clone(),
        fog: data.fog,
        sky: data.sky,
        layers: data.layers.clone(),
        collision_matrix: data.collision_matrix.clone(),
        materials: Default::default(),
        meshes: Default::default(),
        probes: data.probes.clone(),
        reflection_probes: data.reflection_probes.clone(),
        lightmaps: data.lightmaps.clone(),
        lighting_settings: data.lighting_settings,
    }
}

/// A reference outside the incoming scene is dropped: its id may be live here.
fn move_entity(entity: &mut Entity, moves: &Moves) {
    let to = |id: u32| moves.moved(id);
    entity.id = moves.to(entity.id);
    entity.parent_id = entity.parent_id.and_then(to);
    entity.children = entity.children.iter().filter_map(|&c| to(c)).collect();
    entity.remap_refs(&to);
    // A skin's bones are outside `remap_refs`, never saved.
    let skin = entity.mesh.iter_mut().flat_map(|m| &mut m.skeleton.bones);
    skin.for_each(|slot| *slot = slot.and_then(to));
}

/// The prefab's rule for materials (`Merge::Uniquify`); the renames repoint the incoming entities.
fn merge_meshes(scene: &mut Scene, incoming: &Scene) -> BTreeMap<String, String> {
    let mut renames = BTreeMap::new();
    for (name, mesh) in &incoming.meshes {
        let name = match scene.meshes.get(name) {
            None => name.clone(),
            Some(held) if held == mesh => continue,
            Some(_) => {
                let unique = uniquify(&scene.meshes, name);
                renames.insert(name.clone(), unique.clone());
                unique
            }
        };
        scene.meshes.insert(name, mesh.clone());
    }
    renames
}

pub(crate) fn rename_entity_mesh(entity: &mut Entity, renames: &BTreeMap<String, String>) {
    let kind = rusty_components::mesh::SCENE_KIND;
    let own = entity.mesh.iter_mut().filter(|m| m.primitive_type == kind);
    let own = own.map(|m| &mut m.asset_ref);
    let kept = entity
        .collider
        .iter_mut()
        .filter_map(|c| c.shared_mesh.as_mut());
    let kept = kept
        .filter(|k| k.primitive_type == kind)
        .map(|k| &mut k.asset_ref);
    for name in own.chain(kept).flatten() {
        if let Some(renamed) = renames.get(name) {
            *name = renamed.clone();
        }
    }
}

fn skipped_bakes(data: &rusty_scene::SceneData, sidecars: &SceneSidecars) -> Vec<&'static str> {
    let found = [
        ("lightmaps", !data.lightmaps.is_empty()),
        ("light probes", sidecars.lighting.has_probes()),
        ("reflection probes", !data.reflection_probes.is_empty()),
        ("occlusion culling", sidecars.lighting.has_occlusion()),
        ("navmesh", sidecars.has_navmesh()),
    ];
    let found = found.into_iter().filter(|(_, has)| *has);
    found.map(|(name, _)| name).collect()
}

#[cfg(test)]
#[path = "additive_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "additive_mesh_tests.rs"]
mod mesh_tests;
