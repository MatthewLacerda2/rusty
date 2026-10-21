//! The scales and turns every conformance sample runs under (#1171), and the one
//! place the tests read a world pose.

use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};

use crate::components::Pose;
use crate::scene::Scene;

/// Uniform, non-uniform on each axis, one mirrored axis, tiny and Dust2-sized.
pub(super) const SCALES: [Vec3; 8] = [
    Vec3::ONE,
    Vec3::new(4.0, 1.0, 1.0),
    Vec3::new(1.0, 4.0, 1.0),
    Vec3::new(1.0, 1.0, 4.0),
    Vec3::new(1.0, 2.0, 3.0),
    Vec3::new(-1.5, 1.0, 1.0),
    Vec3::splat(1e-3),
    Vec3::splat(1e3),
];

/// The fixed turns, and whether each keeps the axes on the axes (a world AABB is
/// only tight around a shape turned that way).
pub(super) fn turns() -> [(Quat, bool); 4] {
    [
        (Quat::IDENTITY, true),
        (Quat::from_rotation_y(FRAC_PI_2), true),
        (
            Quat::from_rotation_x(FRAC_PI_2) * Quat::from_rotation_z(FRAC_PI_2),
            true,
        ),
        (
            Quat::from_rotation_y(0.7) * Quat::from_rotation_x(0.3),
            false,
        ),
    ]
}

/// The world pose of an entity at `turn` and `scale`: on a root entity, and on a
/// child that takes its scale from a parent. The one place the tests read a pose,
/// through [`crate::ecs::World::world_pose`] (#1170 swaps the source here).
pub(super) fn world_poses(scale: Vec3, turn: Quat) -> [Pose; 2] {
    let mut scene = Scene::new();
    let at = Vec3::new(4.0, -1.0, 2.0);
    let root = scene.add_entity("Root".to_string());
    set(&mut scene, root, at, turn, scale);
    let parent = scene.add_entity("Parent".to_string());
    set(&mut scene, parent, at, Quat::IDENTITY, scale);
    let child = scene.add_entity("Child".to_string());
    scene.set_parent(child, Some(parent)).expect("no cycle");
    set(&mut scene, child, Vec3::new(0.5, 0.0, 0.0), turn, Vec3::ONE);
    [root, child].map(|id| scene.world.world_pose(id).expect("alive"))
}

fn set(scene: &mut Scene, id: u32, pos: Vec3, rot: Quat, scale: Vec3) {
    let mut t = scene.world.transform_mut(id).expect("alive");
    (t.position, t.rotation, t.scale) = (pos, rot, scale);
}
