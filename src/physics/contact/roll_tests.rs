//! src/physics/contact/roll_tests.rs — a body rolling on a flat floor never
//! speeds up (#2071).
//!
//! Nothing pushes it: no motor, and gravity is square to the floor. So its
//! kinetic energy can only fall, and its speed can never rise above its start.
//! Every round shape the engine builds rolls here ([`cases`]): along X and Z, on
//! a big `Box` floor, the `Plane` primitive and a crate-sized box, for up to
//! [`SECONDS`] of fixed ticks. Before #2071 a cylinder broke it within seconds
//! (and a lying capsule went from 0.30 to 0.42 m/s).

use glam::{Quat, Vec3};

use super::fixture::{body, floor, floor_speed, run, Floor};
use crate::components::{CapsuleAxis, ColliderShape};
use crate::scene::Scene;

/// How far above its start a roller's speed may read on `ground`. A facet of a
/// cylinder's side hull lands with a small bump that turns a little vertical
/// speed into horizontal: 1.5% at most over this matrix at 0.3 m/s (2026-11-11),
/// while a roll on rapier's own contacts reads 40% to 700% over. The crate's
/// rolls are slow, so their bump is 0.2% at most; there the gain to catch is
/// rapier recycling a pair's frozen contacts, which the hull's own losses keep
/// down to 1% to 3% over.
fn tolerance(ground: Floor) -> f32 {
    match ground {
        Floor::Crate => 0.01,
        _ => 0.03,
    }
}

/// Which way a roller sets off along the floor.
#[derive(Clone, Copy, Debug)]
enum Along {
    X,
    Z,
}

impl Along {
    fn dir(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Z => Vec3::Z,
        }
    }
}

/// [`roller`]'s sphere and capsule; below them are the three cylinders.
const SPHERE: usize = 3;
const CAPSULE: usize = 4;

/// A round shape lying ready to roll `along`: its shape, its turn, and its radius.
fn roller(kind: usize, along: Along) -> (ColliderShape, Quat, f32) {
    // A lying cylinder's axis is square to the roll: Z for a roll along X.
    let lie = match along {
        Along::X => Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        Along::Z => Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
    };
    let cylinder = |radius, height| (ColliderShape::Cylinder { radius, height }, lie, radius);
    match kind {
        0 => cylinder(0.4, 1.0),
        1 => cylinder(1.0, 0.2),
        2 => cylinder(0.1, 2.0),
        SPHERE => (ColliderShape::Sphere { radius: 0.4 }, Quat::IDENTITY, 0.4),
        _ => {
            let axis = match along {
                Along::X => CapsuleAxis::Z,
                Along::Z => CapsuleAxis::X,
            };
            let capsule = ColliderShape::Capsule {
                radius: 0.4,
                height: 2.0,
                axis,
            };
            (capsule, Quat::IDENTITY, 0.4)
        }
    }
}

/// How long each roll runs: long enough that a gaining roll shows it on every
/// case that gains (the slowest, the 0.3 m/s cylinders on a `Box`, are past ×1.4
/// within 15 s on rapier's own contacts).
const SECONDS: f32 = 20.0;

/// How far a roll may run on the 2 m crate before it would reach the far edge.
const CRATE_RUN: f32 = 1.6;

/// The highest floor speed over [`SECONDS`], as a share of the start speed `v`.
fn peak(kind: usize, along: Along, ground: Floor, v: f32) -> f32 {
    let mut scene = Scene::new();
    floor(&mut scene, ground);
    let (shape, rot, radius) = roller(kind, along);
    // Rolling without slipping: spin × (contact − centre) cancels the velocity.
    let spin = Vec3::NEG_Y.cross(along.dir()) * (-v / radius);
    // On the crate it sets off from one edge and is stopped before the other.
    let (start, seconds) = match ground {
        Floor::Crate => (-CRATE_RUN / 2.0, SECONDS.min(CRATE_RUN / v)),
        _ => (0.0, SECONDS),
    };
    let pos = Vec3::Y * radius + along.dir() * start;
    let id = body(&mut scene, shape, rot, pos, along.dir() * v, spin);
    let mut top: f32 = 0.0;
    run(&mut scene, seconds, |scene| {
        top = top.max(floor_speed(scene, id))
    });
    top / v
}

/// Every case the property covers: each lying cylinder and the capsule, along X
/// and Z, slow and fast on the big floors, and slowly across the crate, where
/// rapier's contact recycling has its widest window. A sphere never reaches the
/// `Rolling` dispatcher, so one case of it is the passing control.
fn cases() -> Vec<(usize, Along, Floor, f32)> {
    let mut all = vec![(SPHERE, Along::X, Floor::Box, 0.3)];
    for kind in [0, 1, 2, CAPSULE] {
        for along in [Along::X, Along::Z] {
            for ground in [Floor::Box, Floor::Plane] {
                all.extend([0.3, 3.0].map(|v| (kind, along, ground, v)));
            }
            all.extend([0.1, 0.3].map(|v| (kind, along, Floor::Crate, v)));
        }
    }
    all
}

#[test]
fn a_rolling_body_never_speeds_up_on_a_flat_floor() {
    let broken: Vec<String> = cases()
        .into_iter()
        .filter_map(|(kind, along, ground, v)| {
            let ratio = peak(kind, along, ground, v);
            (ratio > 1.0 + tolerance(ground)).then(|| {
                format!(
                    "{:?} {along:?} {ground:?} at {v}: x{ratio:.3}",
                    roller(kind, along).0
                )
            })
        })
        .collect();
    assert!(broken.is_empty(), "sped up rolling:\n{}", broken.join("\n"));
}
