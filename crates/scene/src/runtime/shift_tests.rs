use glam::{IVec3, Vec3};

use crate::Scene;
use rusty_components::TransformComponent;
use rusty_ecs::Cut;

fn scene_with_child() -> (Scene, u32, u32) {
    let mut s = Scene::new();
    let root = s.add_entity("Root".into());
    let child = s.add_entity("Child".into());
    s.set_parent(child, Some(root)).unwrap();
    if let Some(mut t) = s.world.transform_mut(root) {
        *t = TransformComponent {
            position: Vec3::new(30_000.123, 5.0, -8_192.5),
            ..Default::default()
        };
    }
    if let Some(mut t) = s.world.transform_mut(child) {
        t.position = Vec3::new(1.0, 2.0, 3.0);
    }
    (s, root, child)
}

#[test]
fn a_root_moves_by_whole_steps_and_its_child_rides_it() {
    let (mut s, root, child) = scene_with_child();
    let by = s.shift_origin(IVec3::new(29, 0, -8));
    assert_eq!(by, Vec3::new(-29_696.0, 0.0, 8_192.0));
    let p = s.world.transform(root).unwrap().position;
    assert_eq!(p, Vec3::new(30_000.123 - 29_696.0, 5.0, -0.5));
    assert_eq!(
        s.world.transform(child).unwrap().position,
        Vec3::new(1.0, 2.0, 3.0)
    );
    assert_eq!(s.floating_origin.offset().x, 29_696.0);
}

#[test]
fn a_shift_and_its_reverse_give_back_the_same_bits() {
    let (mut s, root, _) = scene_with_child();
    let before = s.world.transform(root).unwrap().position;
    s.shift_origin(IVec3::new(29, 0, -8));
    s.shift_origin(IVec3::new(-29, 0, 8));
    let after = s.world.transform(root).unwrap().position;
    assert_eq!(
        before.to_array().map(f32::to_bits),
        after.to_array().map(f32::to_bits)
    );
    assert!(s.floating_origin.is_zero());
}

#[test]
fn every_moved_entity_is_a_cut() {
    let (mut s, root, child) = scene_with_child();
    let mut cuts = s.world.change_cursor::<Cut>();
    let _ = s.world.changes(&mut cuts);
    s.shift_origin(IVec3::X);
    match s.world.changes(&mut cuts) {
        rusty_ecs::Changes::Ids(ids) => {
            assert!(ids.contains(&root) && ids.contains(&child), "{ids:?}")
        }
        rusty_ecs::Changes::Rebuild => {}
    }
}

#[test]
fn world_decals_and_debug_lines_move_with_the_world() {
    let mut s = Scene::new();
    s.debug_lines.push(Vec3::ZERO, Vec3::X, [1.0; 4], 10);
    let by = s.shift_origin(IVec3::Z);
    let line = s.debug_lines.lines().next().copied().unwrap();
    assert_eq!((line.from, line.to), (by, Vec3::X + by));
}
