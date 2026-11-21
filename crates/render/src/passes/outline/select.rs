//! As in Unity a shown renderer rings alone; a LOD group's root or a hidden renderer
//! stands for the level the camera shows. ShadowsOnly meshes never ring.

use crate::lod::LodSelection;
use rusty_scene::Scene;

pub(crate) fn outlined(scene: &Scene, lod: &LodSelection) -> Vec<u32> {
    scene
        .selected_entity_id
        .map_or_else(Vec::new, |selected| outline_targets(scene, selected, lod))
}

pub(super) fn outline_targets(scene: &Scene, selected: u32, lod: &LodSelection) -> Vec<u32> {
    if !scene.world.is_active_in_hierarchy(selected) {
        return Vec::new();
    }
    let mut ids = vec![selected];
    let stands_for_level = |&g: &u32| g == selected || lod.hides(selected);
    if let Some(group_id) = owning_group(scene, selected).filter(stands_for_level) {
        let group = scene
            .world
            .lod_group(group_id)
            .expect("owning_group found it");
        ids = std::iter::once(group_id).chain(group.renderers()).collect();
    }
    let mut seen = Vec::new();
    ids.retain(|&id| {
        let fresh = !seen.contains(&id);
        seen.push(id);
        fresh
            && scene
                .world
                .mesh(id)
                .is_some_and(|m| m.cast_shadows.is_visible())
            && scene.world.is_active_in_hierarchy(id)
            && !lod.hides(id)
    });
    ids
}

fn owning_group(scene: &Scene, id: u32) -> Option<u32> {
    let world = &scene.world;
    if world.has_lod_group(id) {
        return Some(id);
    }
    world.ids_with_lod_group().into_iter().find(|&g| {
        world
            .lod_group(g)
            .is_some_and(|l| l.renderers().any(|r| r == id))
            && world.is_active_in_hierarchy(g)
    })
}
