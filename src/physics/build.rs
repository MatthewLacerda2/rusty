//! src/physics/build.rs — body/collider construction helpers.
//!
//! Pure mapping from the engine's component model to rapier builders, plus the
//! body-class decision. Kept out of `world.rs` so that file stays focused on the
//! step/sync lifecycle (and under the size cap).

use glam::Vec3;
use rapier3d::prelude::*;

use crate::components::{ColliderShape, RigidBodyComponent};

/// Body class derived from the entity flags. Mirrors the legacy solver's encoding:
/// static (fixed), kinematic (position-driven), or dynamic (gravity + solver).
pub(super) enum BodyClass {
    Static,
    Kinematic,
    Dynamic,
}

pub(super) fn classify(is_static: bool, rb: Option<&RigidBodyComponent>) -> BodyClass {
    if is_static {
        BodyClass::Static
    } else if rb.is_none_or(|r| r.is_kinematic) {
        BodyClass::Kinematic
    } else {
        BodyClass::Dynamic
    }
}

pub(super) fn is_kinematic(is_static: bool, rb: Option<&RigidBodyComponent>) -> bool {
    matches!(classify(is_static, rb), BodyClass::Kinematic)
}

/// Build a parry collider shape from the engine's `ColliderShape`, baking in the
/// transform's (lossy, non-uniform) world scale the same way the legacy AABB did.
pub(super) fn build_shape(shape: &ColliderShape, scale: Vec3) -> Collider {
    match shape {
        ColliderShape::Box { size } => {
            let h = *size * scale * 0.5;
            ColliderBuilder::cuboid(h.x.max(1e-4), h.y.max(1e-4), h.z.max(1e-4)).build()
        }
        ColliderShape::Sphere { radius } => {
            let r = (*radius * scale.max_element()).max(1e-4);
            ColliderBuilder::ball(r).build()
        }
        ColliderShape::Cylinder { radius, height } => {
            let r = (*radius * scale.x).max(1e-4);
            let half_h = (*height * scale.y * 0.5).max(1e-4);
            ColliderBuilder::cylinder(half_h, r).build()
        }
    }
}

/// Canonical (low, high) ordering so trigger-pair dedup is stable.
pub(super) fn order_pair(a: u32, b: u32) -> (u32, u32) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}
