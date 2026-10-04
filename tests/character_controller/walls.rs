//! Sliding along a wall under gravity: the wall takes only the push into it,
//! never the stride along it (#793).

use glam::{Quat, Vec3};
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

use super::{add_box, add_character, add_floor, pos_of, walk_for, DT};

/// Walking at an angle into a long wall slides along it at the full along-wall
/// speed, Unity's collide-and-slide. rapier 0.36 reads the gravity in such a
/// move as a slip off the floor, as it does on open ground, and drops part of
/// the slide (~3% of it before #793); the sweep's flat rerun recovers it here
/// too. The 1.5% allowance is the approach and the skin.
#[test]
fn a_wall_slide_keeps_its_along_wall_speed() {
    for velocity in [Vec3::new(4.0, 0.0, 2.0), Vec3::new(2.0, 0.0, 8.0)] {
        let mut scene = Scene::new();
        add_floor(&mut scene);
        // Its face at x = 1.5, running 40 m along z.
        let size = Vec3::new(1.0, 2.0, 40.0);
        add_box(&mut scene, Vec3::new(2.0, 1.0, 0.0), size, Quat::IDENTITY);
        let id = add_character(&mut scene, Vec3::new(0.0, 0.0, -15.0));
        let mut physics = PhysicsWorld::from_scene(&scene);
        let ticks = 120;
        walk_for(&mut physics, &mut scene, id, velocity, ticks);
        let p = pos_of(&scene, id);
        assert!(
            p.x < 1.0 + 1e-3,
            "{velocity}: through the wall at x={}",
            p.x
        );
        let wanted = velocity.z * DT * ticks as f32;
        let along = p.z + 15.0;
        assert!(
            along > wanted * 0.985,
            "{velocity}: slid {along} of {wanted}"
        );
    }
}
