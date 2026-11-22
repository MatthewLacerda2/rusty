//! The one sizing rule both a collider's rapier shape and its world AABB follow.

use glam::Vec3;

use super::{CapsuleAxis, ColliderShape};

/// Smallest half-extent or radius in metres a primitive keeps, so a zero scale builds a flat shape.
const MIN_SIZE: f32 = 1e-4;

/// Primitives take the scale's magnitude, so a mirror keeps their size; only a mesh takes its sign.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScaledShape {
    Cuboid {
        half: Vec3,
    },
    Ball {
        radius: f32,
    },
    Cylinder {
        half_height: f32,
        radius: f32,
    },
    Capsule {
        half_segment: f32,
        radius: f32,
        axis: CapsuleAxis,
    },
    /// Opposite corners, unordered under a negative scale: not a min and a max.
    Mesh {
        convex: bool,
        a: Vec3,
        b: Vec3,
    },
}

impl ScaledShape {
    pub fn local_box(self) -> (Vec3, Vec3) {
        let half = match self {
            Self::Cuboid { half } => half,
            Self::Ball { radius } => Vec3::splat(radius),
            Self::Cylinder {
                half_height,
                radius,
            } => Vec3::new(radius, half_height, radius),
            Self::Capsule {
                half_segment,
                radius,
                axis,
            } => Vec3::splat(radius) + axis.unit() * half_segment,
            Self::Mesh { a, b, .. } => return (a, b),
        };
        (-half, half)
    }
}

impl ColliderShape {
    pub fn scaled(&self, scale: Vec3) -> ScaledShape {
        let abs = scale.abs();
        match *self {
            Self::Box { size } => ScaledShape::Cuboid {
                half: (size * abs * 0.5).max(Vec3::splat(MIN_SIZE)),
            },
            Self::Sphere { radius } => ScaledShape::Ball {
                radius: (radius * abs.max_element()).max(MIN_SIZE),
            },
            Self::Cylinder { radius, height } => ScaledShape::Cylinder {
                half_height: (height * abs.y * 0.5).max(MIN_SIZE),
                radius: round_radius(radius, abs, CapsuleAxis::Y).max(MIN_SIZE),
            },
            Self::Capsule {
                radius,
                height,
                axis,
            } => {
                let (half_segment, radius) = capsule_dims(radius, height, axis, scale);
                ScaledShape::Capsule {
                    half_segment,
                    radius,
                    axis,
                }
            }
            Self::Mesh {
                convex,
                local_min,
                local_max,
            } => ScaledShape::Mesh {
                convex,
                a: local_min * scale,
                b: local_max * scale,
            },
        }
    }
}

/// rapier's inner half-segment, so a full length under `2r` is a sphere; the CharacterController
/// sizes here too.
pub fn capsule_dims(radius: f32, height: f32, axis: CapsuleAxis, scale: Vec3) -> (f32, f32) {
    let along = scale.abs().dot(axis.unit());
    let r = round_radius(radius, scale, axis).max(MIN_SIZE);
    (((height * along * 0.5) - r).max(0.0), r)
}

/// The larger scale across `axis`, so the shape never sits inside its drawn surface.
pub fn round_radius(radius: f32, scale: Vec3, axis: CapsuleAxis) -> f32 {
    let s = scale.abs();
    let cross = match axis {
        CapsuleAxis::X => s.y.max(s.z),
        CapsuleAxis::Y => s.x.max(s.z),
        CapsuleAxis::Z => s.x.max(s.y),
    };
    radius * cross
}
