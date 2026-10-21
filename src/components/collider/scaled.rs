//! src/components/collider/scaled.rs — a collider shape with its world scale
//! baked in: the one sizing rule physics builds the rapier shape from and the
//! world AABB is drawn around (#1140), so the two never drift apart (#1131).

use glam::Vec3;

use super::{CapsuleAxis, ColliderShape};

/// The smallest size a primitive keeps on any axis, so a zero scale still
/// builds a valid (flat) rapier shape.
const MIN_SIZE: f32 = 1e-4;

/// A [`ColliderShape`] sized by a world scale. Primitives size from the scale's
/// magnitude, Unity-style, so a mirror keeps their size; only a mesh is
/// reflected by the signed scale (#1124), so its corners carry the signs.
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
    /// A mesh's local bounds times the signed scale: two opposite corners `a`
    /// and `b`, unordered under a negative scale (not a min and a max). The
    /// rapier shape is built from the vertices.
    Mesh {
        convex: bool,
        a: Vec3,
        b: Vec3,
    },
}

impl ScaledShape {
    /// The two opposite corners of the shape's box in its own (unrotated)
    /// space: what the world AABB turns and moves.
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
    /// This shape under the world `scale` ([`ScaledShape`]).
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

/// A capsule's `(half_segment, radius)` under `scale`, Unity-style: the full
/// length (caps included) scales with the axis' scale, the radius by
/// [`round_radius`]. rapier takes the half-length of the inner segment, so a
/// full length under `2r` collapses to a sphere. Shared with the
/// CharacterController's sweep, whose capsule is not a [`ColliderShape`].
pub fn capsule_dims(radius: f32, height: f32, axis: CapsuleAxis, scale: Vec3) -> (f32, f32) {
    let along = scale.abs().dot(axis.unit());
    let r = round_radius(radius, scale, axis).max(MIN_SIZE);
    (((height * along * 0.5) - r).max(0.0), r)
}

/// A round cross-section's radius under `scale`: `radius` times the larger of
/// the two scales across `axis`, so the shape never sits inside its drawn
/// surface. The one rule a cylinder (axis Y) and a capsule share (#1131). A
/// mirrored axis keeps its size.
pub fn round_radius(radius: f32, scale: Vec3, axis: CapsuleAxis) -> f32 {
    let s = scale.abs();
    let cross = match axis {
        CapsuleAxis::X => s.y.max(s.z),
        CapsuleAxis::Y => s.x.max(s.z),
        CapsuleAxis::Z => s.x.max(s.y),
    };
    radius * cross
}
