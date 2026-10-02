//! The grounded state and collision flags a move reports, and edit mode.

use glam::{Quat, Vec3};
use rusty::physics::{self, PhysicsWorld};
use rusty::scene::Scene;

use super::{add_box, add_character, add_floor, pos_of, walk_for, DT};

/// Standing on the floor under scripted gravity: grounded, the flags say below,
/// and the ground normal is straight up.
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
    // Walking flat (no downward motion) still reads as grounded.
    let flat = physics::move_character(Some(&physics), &mut scene, id, Vec3::X * 0.05).unwrap();
    assert!(flat.grounded, "a flat move on the floor stays grounded");
}

/// With nothing below, a falling character is airborne and actually falls.
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

/// Walking into a wall reports sides; jumping into a ceiling reports above.
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
    // Under the ceiling (its underside at 2.5, the head at ~2.08): jump.
    scene.world.transform_mut(id).unwrap().position.x = -3.0;
    physics.step(&mut scene, DT);
    let up = physics::move_character(Some(&physics), &mut scene, id, Vec3::Y).unwrap();
    assert!(up.flags & 4 != 0, "the ceiling is above: {up:?}");
    assert!(pos_of(&scene, id).y < 1.5, "and it stopped the jump");
}

/// Without a physics world (edit mode) a move goes exactly where asked; a move
/// under the minimum distance does nothing.
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
