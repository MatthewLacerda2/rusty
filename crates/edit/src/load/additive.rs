//! An additive load: the scene is applied to a scene of its own (bones, prefab links,
//! requirements), then each entity joins the World under a fresh id past all it handed out, in
//! document order.

use rusty_components::EntityRefs;
use rusty_scene::runtime::membership::SceneSettings;
use rusty_scene::runtime::probe_sets::SceneProbes;
use rusty_scene::runtime::renumber::Moves;
use rusty_scene::{Entity, Scene};

use super::prepare::{staged, PreparedScene, SceneSidecars};
use rusty_scene::lighting::io::apply_lighting_sidecar;
use std::collections::BTreeMap;

use crate::prefab::merge::{merge_materials, rename_entity_material, uniquify, Merge};

pub struct Joined {
    pub ids: Vec<u32>,
    pub roots: Vec<u32>,
    pub settings: SceneSettings,
    /// For `Scene::park_probes`.
    pub probes: SceneProbes,
    /// The bakes it carries but does not apply yet.
    pub skipped: Vec<&'static str>,
    /// Its layers or collision matrix differ; the World's stay.
    pub disagrees: bool,
}

pub fn join_prepared(scene: &mut Scene, prepared: PreparedScene) -> Joined {
    let PreparedScene {
        path: _,
        document,
        sidecars,
        baked_navmesh: _,
    } = prepared;
    let skipped = skipped_bakes(document.data(), &sidecars);
    let mut incoming = staged(document);
    if let Err(why) = apply_lighting_sidecar(&mut incoming, sidecars.lighting) {
        log::warn!("[Lighting] {why}: the additive scene's probes are unbaked");
    }
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
    Joined {
        ids: joined,
        roots,
        settings: incoming.settings(),
        probes: SceneProbes {
            light: std::mem::take(&mut incoming.probes),
            reflection: std::mem::take(&mut incoming.reflection_probes),
        },
        skipped,
        disagrees,
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

fn rename_entity_mesh(entity: &mut Entity, renames: &BTreeMap<String, String>) {
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
