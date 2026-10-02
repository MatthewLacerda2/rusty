//! Crouching: a shorter capsule fits under an overhang the standing one cannot,
//! and `can_stand` says whether there is room to stand back up.

use glam::{Quat, Vec3};
use rusty::physics::{self, PhysicsWorld};
use rusty::scene::Scene;

use super::{add_box, add_character, add_floor, pos_of, walk_for, DT};

/// A floor with an overhang (underside at 1.5 m) spanning x in [2, 6].
fn overhang_scene() -> (Scene, u32) {
    let mut scene = Scene::new();
    add_floor(&mut scene);
    let size = Vec3::new(4.0, 1.0, 6.0);
    add_box(&mut scene, Vec3::new(4.0, 2.0, 0.0), size, Quat::IDENTITY);
    let id = add_character(&mut scene, Vec3::ZERO);
    (scene, id)
}

/// Crouch to 1 m with the feet kept planted.
fn crouch(scene: &mut Scene, id: u32) {
    let mut cc = scene.world.character_controller_mut(id).unwrap();
    cc.height = 1.0;
    cc.center = Vec3::new(0.0, -0.5, 0.0);
}

/// Standing, the overhang blocks the walk; crouched, the character passes under.
#[test]
fn only_a_crouched_character_fits_under_an_overhang() {
    let (mut scene, id) = overhang_scene();
    let mut physics = PhysicsWorld::from_scene(&scene);
    walk_for(&mut physics, &mut scene, id, Vec3::X * 4.0, 60);
    assert!(pos_of(&scene, id).x < 1.6, "standing, it is blocked");
    crouch(&mut scene, id);
    physics.step(&mut scene, DT);
    walk_for(&mut physics, &mut scene, id, Vec3::X * 4.0, 45);
    assert!(pos_of(&scene, id).x > 3.0, "crouched, it walks under");
}

/// Under the overhang the full height does not fit; out in the open it does.
#[test]
fn can_stand_only_where_there_is_room() {
    let (mut scene, id) = overhang_scene();
    crouch(&mut scene, id);
    let mut physics = PhysicsWorld::from_scene(&scene);
    assert!(
        physics::can_stand(Some(&physics), &scene, id, 2.0),
        "in the open"
    );
    walk_for(&mut physics, &mut scene, id, Vec3::X * 4.0, 60);
    assert!(pos_of(&scene, id).x > 3.0, "now under the overhang");
    assert!(
        !physics::can_stand(Some(&physics), &scene, id, 2.0),
        "no room to stand"
    );
    assert!(
        physics::can_stand(Some(&physics), &scene, id, 1.0),
        "the crouch fits"
    );
    assert!(
        !physics::can_stand(None, &scene, 9999, 2.0),
        "no controller"
    );
}
