//! Which entities the selection outline rings (#1160): the selection itself, or the
//! LOD level the camera shows when the selection stands for a LOD group.

use crate::render::lod::LodSelection;
use crate::scene::Scene;

/// The entities the selection outline rings as `lod` shows them; empty with nothing
/// selected. The forward pass marks their draws (`DrawItem::outline`).
pub(crate) fn outlined(scene: &Scene, lod: &LodSelection) -> Vec<u32> {
    scene
        .selected_entity_id
        .map_or_else(Vec::new, |selected| outline_targets(scene, selected, lod))
}

/// The entities whose outlines ring `selected`. A LOD group's root, or a renderer the
/// current level hides, stands for the level `lod` shows (#1160): selecting LOD0 while
/// LOD2 draws rings LOD2, and a culled group rings nothing. A group's own mesh rings
/// with its level. A shown renderer, like anything else, is outlined alone, as Unity
/// does: selecting Hat in a `[Body, Hat]` level rings Hat. Only active, shown meshes
/// ring.
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
            && scene.world.has_mesh(id)
            && scene.world.is_active_in_hierarchy(id)
            && !lod.hides(id)
    });
    ids
}

/// The active LOD group `id` is, or lists as a renderer. An inactive group hides
/// nothing (`LodSelection`), so it owns nothing here either.
fn owning_group(scene: &Scene, id: u32) -> Option<u32> {
    let world = &scene.world;
    if world.has_lod_group(id) {
        return Some(id);
    }
    world.ids_with_lod_group().into_iter().find(|&g| {
        world.is_active_in_hierarchy(g)
            && world
                .lod_group(g)
                .is_some_and(|l| l.renderers().any(|r| r == id))
    })
}
