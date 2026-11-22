//! The grounded state and collision flags a move reports, and edit mode.

use glam::{Quat, Vec3};
use rusty::physics::{self, PhysicsWorld};
use rusty::scene::Scene;

use super::{add_box, add_character, add_floor, pos_of, walk_for, DT};

#[test]
fn resting_on_the_floor_is_grounded() {
    let mut scene = Scene::new();
    add_floor(&mut scene);
    let id = add_character(&mut scene, Vec3::Y * 0.5);
    let mut physics = PhysicsWorld::from_scene(&scene);
    let last = walk_for(&mut physics, &mut scene, id, Vec3::ZERO, 60);
    assert!(last.grounded && last.flags & 1 != 0, "landed: {last:?}");
    assert!((last.ground_normal - Vec3::Y).length() < 1e-3);
    let grounded = scene.world.character_controller(id).unwrap().is_grounded;
    assert!(grounded, "the component keeps the grounded state");
    let flat = physics::move_character(Some(&physics), &mut scene, id, Vec3::X * 0.05).unwrap();
    assert!(flat.grounded, "a flat move on the floor stays grounded");
}

#[test]
fn falling_with_nothing_below_is_airborne() {
    let mut scene = Scene::new();
    let id = add_character(&mut scene, Vec3::Y * 5.0);
    let mut physics = PhysicsWorld::from_scene(&scene);
    let last = walk_for(&mut physics, &mut scene, id, Vec3::ZERO, 30);
    assert!(!last.grounded && last.flags == 0, "airborne: {last:?}");
    assert!(
        pos_of(&scene, id).y < 5.0,
        "the script's gravity moved it down"
    );
}

#[test]
fn walls_hit_the_sides_and_ceilings_hit_above() {
    let mut scene = Scene::new();
    add_floor(&mut scene);
    add_box(
        &mut scene,
        Vec3::new(2.0, 2.0, 0.0),
        Vec3::new(1.0, 4.0, 6.0),
        Quat::IDENTITY,
    );
    add_box(
        &mut scene,
        Vec3::new(-3.0, 2.6, 0.0),
        Vec3::new(2.0, 0.2, 2.0),
        Quat::IDENTITY,
    );
    let id = add_character(&mut scene, Vec3::ZERO);
    let mut physics = PhysicsWorld::from_scene(&scene);
    let wall = walk_for(&mut physics, &mut scene, id, Vec3::X * 4.0, 40);
    assert!(wall.flags & 2 != 0, "the wall is on the side: {wall:?}");
    assert!(pos_of(&scene, id).x < 1.5, "and it blocked the move");
    scene.world.transform_mut(id).unwrap().position.x = -3.0;
    physics.step(&mut scene, DT);
    let up = physics::move_character(Some(&physics), &mut scene, id, Vec3::Y).unwrap();
    assert!(up.flags & 4 != 0, "the ceiling is above: {up:?}");
    assert!(pos_of(&scene, id).y < 1.5, "and it stopped the jump");
}

#[test]
fn edit_mode_moves_freely_and_tiny_moves_do_nothing() {
    let mut scene = Scene::new();
    let id = add_character(&mut scene, Vec3::ZERO);
    let start = pos_of(&scene, id);
    physics::move_character(None, &mut scene, id, Vec3::new(1.0, -2.0, 3.0)).unwrap();
    assert!((pos_of(&scene, id) - start - Vec3::new(1.0, -2.0, 3.0)).length() < 1e-5);
    let before = pos_of(&scene, id);
    physics::move_character(None, &mut scene, id, Vec3::X * 0.0005).unwrap();
    assert_eq!(pos_of(&scene, id), before, "under min_move_distance");
    let none = physics::move_character(None, &mut scene, 9999, Vec3::X);
    assert!(none.is_none(), "no controller, no move");
}

/// Walking a flat floor covers nearly the full stride every tick: rapier 0.36 can read float noise
/// in the downward move as a slip and drop the slide (the sweep reruns it flat); the 10% allowance
/// is parry's contact-normal noise.
#[test]
fn walking_a_flat_floor_never_stalls() {
    for velocity in [
        Vec3::new(0.0, 0.0, 2.5),
        Vec3::new(1.5, 0.0, -2.0),
        Vec3::X * 5.0,
    ] {
        let mut scene = Scene::new();
        add_floor(&mut scene);
        let id = add_character(&mut scene, Vec3::Y * 0.5);
        let mut physics = PhysicsWorld::from_scene(&scene);
        walk_for(&mut physics, &mut scene, id, Vec3::ZERO, 30);
        let mut vy = 0.0;
        for tick in 0..120 {
            let before = pos_of(&scene, id);
            super::walk(&mut physics, &mut scene, id, velocity, &mut vy);
            let stride = (pos_of(&scene, id) - before) * Vec3::new(1.0, 0.0, 1.0);
            let wanted = velocity.length() * DT;
            assert!(
                stride.length() > wanted * 0.9,
                "tick {tick} at {velocity}: moved {} of {wanted}",
                stride.length()
            );
        }
    }
}
