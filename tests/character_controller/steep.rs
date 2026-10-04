//! Slopes over the limit at every angle, and the ground normal of a move that
//! never touched the floor — the two cases rapier's controller leaves to
//! `physics::character::sweep` (#793): a push into a steep slope must not hold
//! the character on it, and a walkable ramp's normal is read under the capsule.

use glam::Vec3;
use rusty::physics::{self, PhysicsWorld};
use rusty::scene::Scene;

use super::stairs::ramp_scene;
use super::{feet_of, walk_for, DT};

/// Ramp angles over the default 45° slope limit, up to near-vertical.
const STEEP: [f32; 8] = [46.0, 50.0, 55.0, 60.0, 65.0, 70.0, 75.0, 80.0];

/// Put the character on a `degrees` ramp from [`ramp_scene`], touching its
/// surface `along` metres up the slope.
fn on_ramp(degrees: f32, along: f32) -> (Scene, u32) {
    let (mut scene, id) = ramp_scene(degrees);
    let r = degrees.to_radians();
    let surface = Vec3::new(1.0 + along * r.cos(), along * r.sin(), 0.0);
    let normal = Vec3::new(-r.sin(), r.cos(), 0.0);
    // The capsule's lower cap centre sits one radius (plus skin) off the slope.
    let center = surface + normal * 0.58 + Vec3::Y * 0.5;
    scene.world.transform_mut(id).unwrap().position = center;
    (scene, id)
}

/// Unity treats a slope over `slopeLimit` as a wall at every angle: walking
/// into one from the floor never gets up it.
#[test]
fn every_slope_over_the_limit_blocks_like_a_wall() {
    for degrees in STEEP {
        let (mut scene, id) = ramp_scene(degrees);
        let mut physics = PhysicsWorld::from_scene(&scene);
        let mut high = 0.0_f32;
        for _ in 0..120 {
            walk_for(&mut physics, &mut scene, id, Vec3::X * 4.0, 1);
            high = high.max(feet_of(&scene, id).y);
        }
        assert!(high < 0.2, "{degrees}°: climbed to feet y={high}");
    }
}

/// Standing on a steep slope and pushing up it, the character still slides
/// down: the push blocks like a wall, it does not hold the character in place.
/// rapier alone reads the push as an intent to climb and cancels the slide.
#[test]
fn pushing_up_a_steep_slope_still_slides_down_it() {
    for degrees in STEEP {
        let (mut scene, id) = on_ramp(degrees, 2.0);
        let mut physics = PhysicsWorld::from_scene(&scene);
        let start = feet_of(&scene, id).y;
        walk_for(&mut physics, &mut scene, id, Vec3::X * 6.0, 60);
        let end = feet_of(&scene, id).y;
        assert!(end < start - 0.1, "{degrees}°: held at {end}, from {start}");
    }
}

/// A level move across or down a walkable ramp never touches it (snap-to-ground
/// keeps the capsule on), yet still reports the ramp's normal and stays grounded.
#[test]
fn a_move_that_never_hits_the_ramp_still_reads_its_normal() {
    let normal = Vec3::new(-0.5, 3f32.sqrt() * 0.5, 0.0);
    for dir in [Vec3::Z, -Vec3::X] {
        let (mut scene, id) = on_ramp(30.0, 3.0);
        let mut physics = PhysicsWorld::from_scene(&scene);
        for tick in 0..20 {
            let motion = dir * 3.0 * DT;
            let hit = physics::move_character(Some(&physics), &mut scene, id, motion);
            let hit = hit.unwrap();
            physics.step(&mut scene, DT);
            assert!(hit.grounded, "{dir} tick {tick}: airborne");
            let off = (hit.ground_normal - normal).length();
            assert!(off < 0.05, "{dir} tick {tick}: {}", hit.ground_normal);
        }
    }
}
