//! A light's PVS row (#1714, part 4), adapter-free: a light reads the cell it
//! stands in, its row follows it from room to room, and it culls nothing while a
//! baked occluder casts no shadow, has moved, or the light is outside every cell.

use std::sync::Arc;

use glam::Vec3;

use super::OcclusionBinding;
use crate::components::ShadowCasting;
use crate::scene::lighting::occlusion::{bake_occlusion, test_level::corridors};
use crate::scene::Scene;

/// In the first and the second north room, a lamp's height up.
pub(crate) const ROOM_1: Vec3 = Vec3::new(5.0, 2.0, 5.0);
pub(crate) const ROOM_2: Vec3 = Vec3::new(15.0, 2.0, 5.0);

/// The corridors level with its PVS baked.
pub(crate) fn baked() -> Scene {
    let mut scene = corridors();
    let data = bake_occlusion(&scene, 4).0.expect("the level occludes");
    *scene.occlusion = Some(Arc::new(data));
    scene
}

/// The mesh entity named `name` whose origin is nearest `at`.
pub(crate) fn nearest(scene: &Scene, name: &str, at: Vec3) -> u32 {
    let world = &scene.world;
    let d = |id: u32| world.local_to_world(id).w_axis.truncate().distance(at);
    let named = |&id: &u32| world.name(id).is_some_and(|n| n.as_str() == name);
    let ids = world.ids_with_mesh().into_iter().filter(named);
    ids.min_by(|&a, &b| d(a).total_cmp(&d(b))).expect("named")
}

#[test]
fn a_lights_row_is_its_cell_and_follows_it_from_room_to_room() {
    let scene = baked();
    let pvs = OcclusionBinding::bound(&scene);
    let near = nearest(&scene, "Prop0", Vec3::new(2.0, 0.5, 3.5));
    let far = nearest(&scene, "Prop0", Vec3::new(12.0, 0.5, 3.5));
    let first = pvs.shadow_row(ROOM_1).expect("in a cell, the bake usable");
    assert!(!pvs.hides(first, near), "its own room's prop is seen");
    assert!(pvs.hides(first, far), "the next room's is behind the wall");
    // Carried through the corridor into the next room: the mask follows.
    let second = pvs.shadow_row(ROOM_2).expect("in a cell");
    assert!(pvs.hides(second, near) && !pvs.hides(second, far));
    // The camera's row at the same place is the light's: one cell, one row.
    assert_eq!(pvs.row(ROOM_1, u32::MAX), Some(first));
    assert_eq!(
        pvs.shadow_row(Vec3::splat(500.0)),
        None,
        "outside every cell"
    );
    let wall = nearest(&scene, "Wall", Vec3::new(10.0, 1.5, 5.0));
    assert!(pvs.occludes(wall), "a baked wall the mask counts on");
    assert!(!pvs.occludes(u32::MAX), "never baked");
}

#[test]
fn a_light_culls_nothing_while_an_occluder_casts_no_shadow() {
    let mut scene = baked();
    let wall = nearest(&scene, "Wall", Vec3::new(10.0, 1.5, 5.0));
    let set = |scene: &mut Scene, mode| scene.world.mesh_mut(wall).unwrap().cast_shadows = mode;
    set(&mut scene, ShadowCasting::Off);
    let pvs = OcclusionBinding::bound(&scene);
    assert_eq!(pvs.shadow_row(ROOM_1), None, "the wall is not in the map");
    assert!(
        pvs.row(ROOM_1, u32::MAX).is_some(),
        "a camera still sees it"
    );
    // Two-sided casts more than the bake stops a view with: still a wall.
    set(&mut scene, ShadowCasting::TwoSided);
    assert!(OcclusionBinding::bound(&scene).shadow_row(ROOM_1).is_some());
    // Moved since the bake: anything may show past it, so nothing is culled.
    set(&mut scene, ShadowCasting::On);
    scene.world.transform_mut(wall).unwrap().position.y += 0.01;
    let pvs = OcclusionBinding::bound(&scene);
    assert_eq!(pvs.shadow_row(ROOM_1), None);
    assert!(!pvs.occludes(wall));
}
