//! src/physics/build.rs — body/collider construction helpers.
//!
//! Pure mapping from the engine's component model to rapier builders, plus the
//! body-class decision and the per-tick body-state snapshot. Kept out of
//! `world.rs` so that file stays focused on the step/sync lifecycle (and under
//! the size cap).

use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use super::convert::{to_iso, to_na_vec};
use crate::components::{ColliderShape, CollisionDetection, Entity, RigidBodyComponent};

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

/// rapier `gravity_scale` for a body's `use_gravity` flag: `1.0` keeps the body
/// under world gravity, `0.0` exempts it (Unity: `Rigidbody.useGravity`). Only
/// dynamic bodies integrate gravity, so it's a no-op on static/kinematic ones.
pub(super) fn gravity_scale(use_gravity: bool) -> f32 {
    if use_gravity {
        1.0
    } else {
        0.0
    }
}

/// The per-entity inputs needed to build one rapier body + collider, captured in a
/// single borrow of the entity so `world.rs` can release it before touching `self`.
pub(super) struct ColliderInputs {
    pub pos: Vec3,
    pub rot: Quat,
    pub scale: Vec3,
    pub shape: ColliderShape,
    /// Live rest-pose geometry for a mesh collider (positions + triangle indices),
    /// captured here so it never enters the scene document.
    pub mesh_geom: Option<(Vec<[f32; 3]>, Vec<u32>)>,
    pub is_trigger: bool,
    pub is_static: bool,
    pub class: BodyClass,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    /// Whether a dynamic body is pulled by world gravity (Unity: `useGravity`).
    /// Maps to rapier's `gravity_scale` (1.0 when true, 0.0 when false).
    pub use_gravity: bool,
    pub layer: u8,
    /// Discrete vs. continuous (CCD) collision testing (#321), read off the
    /// rigidbody; a body with no `RigidBodyComponent` (a bare static collider)
    /// has nothing to sweep, so it defaults to `Discrete`.
    pub collision_detection: CollisionDetection,
}

/// Snapshot an entity's active-collider inputs, or `None` if it has no active
/// collider. A mesh collider also captures the rest-pose geometry it rebuilds from.
pub(super) fn collider_inputs(entity: &Entity) -> Option<ColliderInputs> {
    let collider = match &entity.collider {
        Some(c) if c.active => c,
        _ => return None,
    };
    let mesh_geom = if matches!(collider.shape, ColliderShape::Mesh { .. }) {
        entity.mesh.as_ref().map(|m| {
            (
                m.vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
                m.indices.clone(),
            )
        })
    } else {
        None
    };
    Some(ColliderInputs {
        pos: entity.transform.position,
        rot: entity.transform.rotation,
        scale: entity.transform.scale,
        shape: collider.shape.clone(),
        mesh_geom,
        is_trigger: collider.is_trigger,
        is_static: entity.is_static,
        class: classify(entity.is_static, entity.rigidbody.as_ref()),
        velocity: entity
            .rigidbody
            .as_ref()
            .map(|r| r.velocity)
            .unwrap_or(Vec3::ZERO),
        angular_velocity: entity
            .rigidbody
            .as_ref()
            .map(|r| r.angular_velocity)
            .unwrap_or(Vec3::ZERO),
        use_gravity: entity.rigidbody.as_ref().is_none_or(|r| r.use_gravity),
        layer: entity.layer,
        collision_detection: entity
            .rigidbody
            .as_ref()
            .map(|r| r.collision_detection)
            .unwrap_or_default(),
    })
}

/// Build the rigid-body half of an entity's physics body: body class (static /
/// kinematic / dynamic), initial pose + velocity, and the CCD flag from its
/// collision-detection mode (#321). Kept alongside [`collider_inputs`] so
/// `world.rs` stays focused on the rapier set/handle bookkeeping.
pub(super) fn body_builder(inp: &ColliderInputs) -> RigidBodyBuilder {
    match inp.class {
        BodyClass::Static => RigidBodyBuilder::fixed(),
        BodyClass::Kinematic => RigidBodyBuilder::kinematic_position_based(),
        BodyClass::Dynamic => RigidBodyBuilder::dynamic()
            .linvel(to_na_vec(inp.velocity))
            .angvel(to_na_vec(inp.angular_velocity))
            .gravity_scale(gravity_scale(inp.use_gravity)),
    }
    .position(to_iso(inp.pos, inp.rot))
    .ccd_enabled(inp.collision_detection == CollisionDetection::Continuous)
}

/// The per-entity component state `sync_to_rapier` pushes into a body each tick.
pub(super) struct EntityBodyState {
    pub pos: Vec3,
    pub rot: Quat,
    pub vel: Vec3,
    pub angular_vel: Vec3,
    pub active: bool,
    pub kinematic: bool,
    pub is_static: bool,
    pub use_gravity: bool,
    /// Discrete vs. continuous (CCD) collision testing (#321), re-applied every
    /// tick so flipping the mode mid-play takes effect live.
    pub collision_detection: CollisionDetection,
}

/// Snapshot an entity's transform/velocity/body-class state for one tick.
pub(super) fn body_state(entity: &Entity) -> EntityBodyState {
    // Gravity needs an authored rigidbody opting in: a collider-only entity
    // (kinematic by default) is script-driven scenery and must never fall (#318),
    // so a missing rigidbody reads as `use_gravity = false` here. Dynamic bodies
    // always have one, so this matches `collider_inputs` for them.
    let use_gravity = entity.rigidbody.as_ref().is_some_and(|r| r.use_gravity);
    EntityBodyState {
        pos: entity.transform.position,
        rot: entity.transform.rotation,
        vel: entity
            .rigidbody
            .as_ref()
            .map(|r| r.velocity)
            .unwrap_or(Vec3::ZERO),
        angular_vel: entity
            .rigidbody
            .as_ref()
            .map(|r| r.angular_velocity)
            .unwrap_or(Vec3::ZERO),
        active: entity.active,
        kinematic: is_kinematic(entity.is_static, entity.rigidbody.as_ref()),
        is_static: entity.is_static,
        use_gravity,
        collision_detection: entity
            .rigidbody
            .as_ref()
            .map(|r| r.collision_detection)
            .unwrap_or_default(),
    }
}

/// Build a parry collider shape from the engine's `ColliderShape`, baking in the
/// transform's (lossy, non-uniform) world scale the same way the legacy AABB did.
///
/// Returns `None` for a mesh collider whose geometry is missing or degenerate
/// (too few vertices for a hull, or no triangles for a trimesh); the caller skips
/// the body entirely in that case. `mesh` carries the entity's live rest-pose
/// positions + triangle indices, supplied only for [`ColliderShape::Mesh`].
pub(super) fn build_shape(
    shape: &ColliderShape,
    scale: Vec3,
    mesh: Option<(&[[f32; 3]], &[u32])>,
) -> Option<Collider> {
    let collider = match shape {
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
        ColliderShape::Mesh { convex, .. } => return build_mesh_shape(*convex, scale, mesh?),
    };
    Some(collider)
}

/// Build a trimesh or convex-hull collider from rest-pose mesh geometry (#77),
/// baking the world scale into each point. `None` if the geometry is too sparse to
/// form a valid shape.
fn build_mesh_shape(convex: bool, scale: Vec3, mesh: (&[[f32; 3]], &[u32])) -> Option<Collider> {
    let (positions, indices) = mesh;
    if positions.len() < 4 {
        return None;
    }
    let points: Vec<Point<f32>> = positions
        .iter()
        .map(|p| Point::new(p[0] * scale.x, p[1] * scale.y, p[2] * scale.z))
        .collect();

    if convex {
        // A convex hull is the cheaper, dynamic-body-friendly option.
        ColliderBuilder::convex_hull(&points).map(|b| b.build())
    } else {
        // An exact triangle mesh — static geometry only, but a tight fit.
        let triangles: Vec<[u32; 3]> = indices
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        if triangles.is_empty() {
            return None;
        }
        Some(ColliderBuilder::trimesh(points, triangles).build())
    }
}

/// rapier collision + solver groups for a collider on `layer`: it is a member of
/// its own layer bit, and collides only with the layers `filter_mask` permits — the
/// row the collision matrix (#91) provides for `layer`. A symmetric matrix makes the
/// pairwise rapier test reduce to `can_collide(a, b)`.
pub(super) fn interaction_groups(layer: u8, filter_mask: u32) -> InteractionGroups {
    let membership = if (layer as usize) < 32 {
        1u32 << layer
    } else {
        0
    };
    InteractionGroups::new(
        Group::from_bits_truncate(membership),
        Group::from_bits_truncate(filter_mask),
    )
}

/// Canonical (low, high) ordering so trigger-pair dedup is stable.
pub(super) fn order_pair(a: u32, b: u32) -> (u32, u32) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}
