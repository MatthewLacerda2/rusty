//! src/physics/contact/roll_tests.rs — a body rolling on a flat floor never
//! speeds up (#2071).
//!
//! Nothing pushes it: no motor, and gravity is square to the floor. So its
//! kinetic energy can only fall, and its speed can never rise above its start.
//! Every round shape the engine builds rolls here, along X and along Z, on a
//! `Box` floor and on the `Plane` primitive, slow and fast, for a minute of
//! fixed ticks. Before #2071 a cylinder broke it within seconds (and a lying
//! capsule went from 0.30 to 0.42 m/s).

use glam::{Quat, Vec3};

use super::fixture::{body, floor, floor_speed, run, Floor};
use crate::components::{CapsuleAxis, ColliderShape};
use crate::scene::Scene;

/// How far above its start a roller's speed may read. A facet of a cylinder's
/// side hull lands with a small bump that turns a little vertical speed
/// into horizontal: 1.6% at most over this matrix at 0.3 m/s (2026-11-11), while
/// a roll that gains energy reads 40% to 700% over.
const TOLERANCE: f32 = 0.03;

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
        3 => (ColliderShape::Sphere { radius: 0.4 }, Quat::IDENTITY, 0.4),
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

/// The highest floor speed over a minute, as a share of the start speed `v`.
fn peak(kind: usize, along: Along, ground: Floor, v: f32) -> f32 {
    let mut scene = Scene::new();
    floor(&mut scene, ground);
    let (shape, rot, radius) = roller(kind, along);
    // Rolling without slipping: spin × (contact − centre) cancels the velocity.
    let spin = Vec3::NEG_Y.cross(along.dir()) * (-v / radius);
    let id = body(
        &mut scene,
        shape,
        rot,
        Vec3::Y * radius,
        along.dir() * v,
        spin,
    );
    let mut top: f32 = 0.0;
    run(&mut scene, 60.0, |scene| {
        top = top.max(floor_speed(scene, id))
    });
    top / v
}

#[test]
fn a_rolling_body_never_speeds_up_on_a_flat_floor() {
    let mut broken = Vec::new();
    for ground in [Floor::Box, Floor::Plane] {
        for along in [Along::X, Along::Z] {
            for kind in 0..5 {
                for v in [0.3, 3.0] {
                    let ratio = peak(kind, along, ground, v);
                    if ratio > 1.0 + TOLERANCE {
                        broken.push(format!(
                            "{:?} {along:?} {ground:?} at {v}: x{ratio:.3}",
                            roller(kind, along).0
                        ));
                    }
                }
            }
        }
    }
    assert!(broken.is_empty(), "sped up rolling:\n{}", broken.join("\n"));
}
