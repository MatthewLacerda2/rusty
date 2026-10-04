//! src/physics/query_tests.rs — the two segment-shaped queries: the wall ray a
//! world canvas finds (`first_surface_ahead`, #429) and audio occlusion
//! (`segment_blocked`, #467). Both pass triggers and look past the collider the
//! origin starts inside; occlusion reaches exactly to the segment's end.

use glam::Vec3;

use super::PhysicsWorld;
use crate::components::{ColliderComponent, ColliderShape};
use crate::scene::Scene;

/// A static unit-thick wall (or trigger) facing -x, its near face at `x - 0.5`.
fn add_wall(scene: &mut Scene, x: f32, is_trigger: bool) -> u32 {
    let id = scene.add_entity("Wall".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(x, 0.0, 0.0);
    scene.world.set_static(id, true);
    let collider = ColliderComponent {
        active: true,
        shape: ColliderShape::Box {
            size: Vec3::new(1.0, 4.0, 4.0),
        },
        is_trigger,
        material: Default::default(),
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    };
    scene.world.set_collider(id, Some(collider));
    id
}

/// A player-sized sphere around the origin, a trigger at x = 2 and a solid wall
/// at x = 5 (near face at 4.5). Returns the wall.
fn corridor(scene: &mut Scene) -> u32 {
    let body = scene.add_entity("Body".to_string());
    scene.world.set_static(body, true);
    let sphere = ColliderShape::Sphere { radius: 0.5 };
    let mut collider = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    collider.shape = sphere;
    scene.world.set_collider(body, Some(collider));
    add_wall(scene, 2.0, true);
    add_wall(scene, 5.0, false)
}

#[test]
fn the_surface_ahead_is_the_first_solid_wall_past_the_own_body() {
    let mut scene = Scene::new();
    let wall = corridor(&mut scene);
    let physics = PhysicsWorld::from_scene(&scene);
    let hit = physics
        .first_surface_ahead(Vec3::ZERO, Vec3::X * 3.0, 20.0)
        .expect("the wall is ahead");
    assert_eq!(hit.id, wall, "the trigger and the own body are passed");
    assert!((hit.distance - 4.5).abs() < 1e-3, "{hit:?}");
    assert!((hit.point - Vec3::new(4.5, 0.0, 0.0)).length() < 1e-3);
    assert!((hit.normal + Vec3::X).length() < 1e-3, "faces the ray");
    let short = physics.first_surface_ahead(Vec3::ZERO, Vec3::X, 4.0);
    assert!(short.is_none(), "the wall is past max_toi: {short:?}");
}

#[test]
fn a_segment_is_blocked_only_by_a_solid_on_it() {
    let mut scene = Scene::new();
    let wall = corridor(&mut scene);
    let physics = PhysicsWorld::from_scene(&scene);
    let any = |_: u32| true;
    let to = |x: f32| Vec3::new(x, 0.0, 0.0);
    assert!(physics.segment_blocked(Vec3::ZERO, to(8.0), any));
    // Ends 0.5 m short of the wall: the trigger and the own body never block,
    // and the segment's length is the reach, not a multiple of it.
    assert!(!physics.segment_blocked(Vec3::ZERO, to(4.0), any));
    assert!(!physics.segment_blocked(Vec3::ZERO, to(8.0), |id| id != wall));
    assert!(!physics.segment_blocked(to(1.0), to(1.0), any), "no length");
}
