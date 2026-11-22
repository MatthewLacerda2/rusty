use glam::{Quat, Vec3};

use super::fixture::{body, floor, floor_speed, run, Floor};
use rusty_components::{CapsuleAxis, ColliderShape};
use rusty_scene::Scene;

/// Fractions of the start speed. A facet landing bumps 1.5% at most over this matrix at 0.3 m/s;
/// the crate's slow rolls bump less, so 1% still catches rapier recycling a pair's frozen
/// contacts there (1% to 3% over).
fn tolerance(ground: Floor) -> f32 {
    match ground {
        Floor::Crate => 0.01,
        _ => 0.03,
    }
}

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

// `roller`'s two kinds past its three cylinders (0 to 2).
const SPHERE: usize = 3;
const CAPSULE: usize = 4;

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

/// Seconds per roll: enough for every gaining case to show it.
const SECONDS: f32 = 20.0;

/// Metres a roll may run on the 2 m crate, short of its far edge.
const CRATE_RUN: f32 = 1.6;

fn peak(kind: usize, along: Along, ground: Floor, v: f32) -> f32 {
    let mut scene = Scene::new();
    floor(&mut scene, ground);
    let (shape, rot, radius) = roller(kind, along);
    // Rolling without slipping: spin × (contact − centre) cancels the velocity.
    let spin = Vec3::NEG_Y.cross(along.dir()) * (-v / radius);
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
