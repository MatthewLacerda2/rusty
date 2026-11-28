//! An additive load: the scene is applied to a scene of its own (bones, prefab links,
//! requirements), then each entity joins the World under a fresh id past all it handed out, in
//! document order.

use rusty_components::EntityRefs;
use rusty_scene::runtime::membership::SceneSettings;
use rusty_scene::runtime::renumber::Moves;
use rusty_scene::{Entity, Scene};

use super::prepare::{staged, PreparedScene, SceneSidecars};
use crate::prefab::merge::{merge_materials, rename_entity_material, Merge};

pub struct Joined {
    pub ids: Vec<u32>,
    pub roots: Vec<u32>,
    pub settings: SceneSettings,
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
    let incoming = staged(document);
    let ids = incoming.world.ids().to_vec();
    let moves = Moves::past(ids.iter().copied(), scene.world.next_id());
    let (renames, _) = merge_materials(scene, &incoming.materials, Merge::Uniquify);
    let mut joined = Vec::with_capacity(ids.len());
    let mut roots = Vec::new();
    for id in ids {
        let Some(mut entity) = incoming.world.entity_document(id) else {
            continue;
        };
        move_entity(&mut entity, &moves);
        rename_entity_material(&mut entity, &renames);
        if entity.parent_id.is_none() {
            roots.push(entity.id);
        }
        joined.push(entity.id);
        scene.world.insert_entity(entity);
    }
    scene.world.bump_next_id(moves.next_id());
    merge_meshes(scene, &incoming);
    scene.sync_scene_meshes();
    scene.update_all_colliders();
    let disagrees =
        incoming.layers != scene.layers || incoming.collision_matrix != scene.collision_matrix;
    Joined {
        ids: joined,
        roots,
        settings: incoming.settings(),
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

/// A name the World holds keeps the World's mesh.
fn merge_meshes(scene: &mut Scene, incoming: &Scene) {
    for (name, mesh) in &incoming.meshes {
        match scene.meshes.get(name) {
            None => {
                scene.meshes.insert(name.clone(), mesh.clone());
            }
            Some(held) if held == mesh => {}
            Some(_) => log::warn!(
                "[Scene] an additive scene's library mesh {name:?} differs from the one already \
                 loaded under that name: the loaded one is kept"
            ),
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
