//! In Play a CharacterController moved ahead on a flat floor covers its whole stride every tick
//! and stays grounded (Unity's `Move`); floors are built as a script builds them (primitive plus
//! `AddComponent("Collider")`, kinematic, no Rigidbody).

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::edit::authoring::Primitive;
use rusty::scene::Scene;

use super::{pos_of, DT};
use crate::play::{playing, run};

mod build;

use build::{add_floor, add_lane, capsules};

const TICKS: usize = 300;
/// Metres along the walk: 30 m/s for `TICKS`, with room to spare.
pub(super) const LENGTH: f32 = 180.0;
pub(super) const START_Z: f32 = -80.0;
const SPEEDS: [f32; 4] = [1.0, 4.0, 10.0, 30.0];
/// A script's ground stick per tick (m).
const STICKS: [f32; 2] = [0.0, -0.02];
/// The share of a stride a tick may lose: the skin settling, never a stall.
const SLACK: f32 = 0.01;

/// A floor: name, primitive, scale, centre height (top face at y = 0), static flag, plane
/// subdivisions.
pub(super) type Floor = (&'static str, Primitive, Vec3, f32, bool, u32);

const BOXES: [Floor; 4] = [
    (
        "box",
        Primitive::Box,
        Vec3::new(52.0, 1.0, LENGTH),
        -0.5,
        false,
        1,
    ),
    (
        "tall box",
        Primitive::Box,
        Vec3::new(52.0, 4.0, LENGTH),
        -2.0,
        false,
        1,
    ),
    (
        "thin box",
        Primitive::Box,
        Vec3::new(52.0, 0.1, LENGTH),
        -0.05,
        false,
        1,
    ),
    (
        "static box",
        Primitive::Box,
        Vec3::new(52.0, 1.0, LENGTH),
        -0.5,
        true,
        1,
    ),
];

fn plane(name: &'static str, subdivisions: u32) -> Floor {
    let scale = Vec3::new(52.0 / 15.0, 1.0, LENGTH / 15.0);
    (name, Primitive::Plane, scale, 0.0, false, subdivisions)
}

fn walk_floor(floor: Floor) {
    let mut scene = Scene::new();
    add_floor(&mut scene, floor);
    let mut lanes = Vec::new();
    for cc in capsules() {
        for speed in SPEEDS {
            for stick in STICKS {
                let x = -24.0 + 3.0 * lanes.len() as f32;
                lanes.push(add_lane(&mut scene, x, cc.clone(), speed, stick));
            }
        }
    }
    let scene = Rc::new(RefCell::new(scene));
    let mut game = playing(scene.clone(), 100.0);
    let mut last: Vec<f32> = lanes
        .iter()
        .map(|l| pos_of(&scene.borrow(), l.id).z)
        .collect();
    for tick in 0..TICKS {
        run(&mut game, 1);
        let s = scene.borrow();
        for (lane, z) in lanes.iter().zip(last.iter_mut()) {
            let (p, name) = (pos_of(&s, lane.id), floor.0);
            let (stride, moved) = (lane.speed * DT, p.z - *z);
            assert!(
                moved > stride * (1.0 - SLACK),
                "{name}, {}: tick {tick} moved {moved} of {stride} (at {p})",
                lane.label
            );
            let grounded = s.world.character_controller(lane.id).unwrap().is_grounded;
            assert!(grounded, "{name}, {}: airborne at tick {tick}", lane.label);
            *z = p.z;
        }
    }
}

#[test]
fn a_walk_across_flat_boxes_covers_every_stride() {
    for floor in BOXES {
        walk_floor(floor);
    }
}

// Each mesh floor runs in its own test, so its own process (nextest).

#[test]
fn a_walk_across_a_mesh_plane_covers_every_stride() {
    walk_floor(plane("plane", 1));
}

#[test]
fn a_walk_across_a_plane_of_7_subdivisions_covers_every_stride() {
    walk_floor(plane("plane, 7 subdivisions", 7));
}

#[test]
fn a_walk_across_a_plane_of_32_subdivisions_covers_every_stride() {
    walk_floor(plane("plane, 32 subdivisions", 32));
}
