//! src/physics/shape/build.rs — body/collider construction helpers.
//!
//! Pure mapping from the engine's component model to rapier builders, plus the
//! body-class decision and the per-tick body-state snapshot. Kept out of
//! `world.rs` so that file stays focused on the step/sync lifecycle.

use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use crate::components::{
    CapsuleAxis, ColliderShape, CollisionDetection, PhysicsMaterial, RigidBodyComponent,
    ScaledShape,
};
use crate::ecs::World;
use crate::scene::Scene;

use super::mesh::{build_mesh_shape, Skip};
use crate::physics::compound::world_pose;

/// Body class derived from the entity flags. Mirrors the legacy solver's encoding:
/// static (fixed), kinematic (position-driven), or dynamic (gravity + solver).
pub(in crate::physics) enum BodyClass {
    Static,
    Kinematic,
    Dynamic,
}

pub(in crate::physics) fn classify(is_static: bool, rb: Option<&RigidBodyComponent>) -> BodyClass {
    if is_static {
        BodyClass::Static
    } else if rb.is_none_or(|r| r.is_kinematic) {
        BodyClass::Kinematic
    } else {
        BodyClass::Dynamic
    }
}

/// Owner `id`'s body class. A CharacterController's body is always kinematic
/// (#451): it moves only where `CharacterController.Move` puts its Transform.
pub(in crate::physics) fn class_of(world: &World, id: u32) -> BodyClass {
    if world.has_character_controller(id) {
        return BodyClass::Kinematic;
    }
    classify(world.is_static(id), world.rigidbody(id).as_deref())
}

/// rapier `gravity_scale` for a body's `use_gravity` flag: `1.0` keeps the body
/// under world gravity, `0.0` exempts it (Unity: `Rigidbody.useGravity`). Only
/// dynamic bodies integrate gravity, so it's a no-op on static/kinematic ones.
pub(in crate::physics) fn gravity_scale(use_gravity: bool) -> f32 {
    if use_gravity {
        1.0
    } else {
        0.0
    }
}

/// Whether rapier's continuous collision detection is switched on for a body's
/// `collision_detection` mode (#321): `Continuous` sweeps the body's motion and
/// stops it at the time of impact (anti-tunnelling), `Discrete` (default) tests
/// overlap only at the tick's final pose. rapier already builds a `CCDSolver`
/// into every step — this flag is the per-body switch that was dormant.
pub(in crate::physics) fn ccd_enabled(mode: CollisionDetection) -> bool {
    matches!(mode, CollisionDetection::Continuous)
}

/// The per-entity inputs needed to build one rapier collider, captured in a
/// single borrow of the entity so `assemble` can release it before touching
/// `self`. The pose is not here: it comes from the hierarchy (`compound`).
pub(in crate::physics) struct ColliderInputs {
    pub shape: ColliderShape,
    /// The shape's centre in the entity's local space, before scale: a
    /// CharacterController's `center`, zero for a Collider.
    pub center: Vec3,
    /// An upright shape ignores the entity's rotation — a CharacterController's
    /// capsule stays vertical however the entity turns (#451).
    pub upright: bool,
    /// Live rest-pose geometry for a mesh collider (positions + triangle indices),
    /// captured here so it never enters the scene document.
    pub mesh_geom: Option<(Vec<[f32; 3]>, Vec<u32>)>,
    pub is_trigger: bool,
    pub material: PhysicsMaterial,
    pub layer: u8,
}

/// Snapshot an entity's active-collider inputs, or `None` if it has no active
/// collider (or is dead). A CharacterController's capsule supersedes a Collider
/// on the same entity (#451). A mesh collider also captures the rest-pose
/// geometry it rebuilds from. Reads through the #344 accessor facade — shared
/// guards only.
pub(in crate::physics) fn collider_inputs(world: &World, id: u32) -> Option<ColliderInputs> {
    if let Some(inputs) = crate::physics::character::character_collider_inputs(world, id) {
        return Some(inputs);
    }
    let collider = world.collider(id).filter(|c| c.active)?;
    let mesh_geom = if matches!(collider.shape, ColliderShape::Mesh { .. }) {
        world.mesh(id).map(|m| {
            (
                m.vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
                m.indices.clone(),
            )
        })
    } else {
        None
    };
    Some(ColliderInputs {
        shape: collider.shape.clone(),
        center: Vec3::ZERO,
        upright: false,
        mesh_geom,
        is_trigger: collider.is_trigger,
        material: collider.material,
        layer: world.layer(id),
    })
}

/// The inputs needed to build one rapier body, read off its owner entity (the
/// entity with the Rigidbody, or the lone collider entity — see `compound`).
pub(in crate::physics) struct BodyInputs {
    pub class: BodyClass,
    pub velocity: Vec3,
    /// Initial angular velocity (radians/sec per axis) seeded onto a dynamic body
    /// at build (#319); rapier integrates rotation from it thereafter.
    pub angular_velocity: Vec3,
    /// Whether a dynamic body is pulled by world gravity (Unity: `useGravity`).
    /// Maps to rapier's `gravity_scale` (1.0 when true, 0.0 when false).
    pub use_gravity: bool,
    /// Discrete vs. Continuous (CCD) contact detection for this body (#321).
    pub collision_detection: CollisionDetection,
}

/// Snapshot the body-level inputs of owner `id`.
pub(in crate::physics) fn body_inputs(world: &World, id: u32) -> BodyInputs {
    let rb = world.rigidbody(id);
    let rb = rb.as_deref();
    BodyInputs {
        class: class_of(world, id),
        velocity: rb.map(|r| r.velocity).unwrap_or(Vec3::ZERO),
        angular_velocity: rb.map(|r| r.angular_velocity).unwrap_or(Vec3::ZERO),
        use_gravity: rb.is_none_or(|r| r.use_gravity),
        collision_detection: rb.map(|r| r.collision_detection).unwrap_or_default(),
    }
}

/// The per-entity component state `sync_to_rapier` pushes into a body each tick.
pub(in crate::physics) struct EntityBodyState {
    /// World-space pose (#445): the owner's local `Transform` resolved through
    /// its parents.
    pub pos: Vec3,
    pub rot: Quat,
    pub vel: Vec3,
    pub angular_velocity: Vec3,
    pub kinematic: bool,
    pub is_static: bool,
    pub use_gravity: bool,
    /// Whether CCD is on for this body — re-applied per tick so flipping the
    /// mode mid-play (`Physics.SetCollisionDetection`) takes effect (#321).
    pub ccd_enabled: bool,
}

/// Snapshot an owner's world pose/velocity/body-class state for one tick
/// (`None` when the entity is dead).
pub(in crate::physics) fn body_state(scene: &Scene, id: u32) -> Option<EntityBodyState> {
    let pose = world_pose(scene, id)?;
    let world = &scene.world;
    let class = class_of(world, id);
    // Only dynamic bodies integrate gravity, and they always have a rigidbody.
    let rb = world.rigidbody(id);
    let rb = rb.as_deref();
    let use_gravity = rb.is_some_and(|r| r.use_gravity);
    let collision_detection = rb.map(|r| r.collision_detection).unwrap_or_default();
    Some(EntityBodyState {
        pos: pose.pos,
        rot: pose.rot,
        vel: rb.map(|r| r.velocity).unwrap_or(Vec3::ZERO),
        angular_velocity: rb.map(|r| r.angular_velocity).unwrap_or(Vec3::ZERO),
        kinematic: matches!(class, BodyClass::Kinematic),
        is_static: matches!(class, BodyClass::Static),
        use_gravity,
        ccd_enabled: ccd_enabled(collision_detection),
    })
}

/// [`try_build_shape`] without the reason.
#[cfg(test)]
pub(in crate::physics) fn build_shape(
    shape: &ColliderShape,
    scale: Vec3,
    mesh: Option<(&[[f32; 3]], &[u32])>,
) -> Option<Collider> {
    try_build_shape(shape, scale, mesh).ok()
}

/// [`try_build_shape`] for collider entity `id`: a skipped mesh collider is logged once,
/// naming the entity and the reason (#1141).
pub(in crate::physics) fn build_shape_for(
    id: u32,
    shape: &ColliderShape,
    scale: Vec3,
    mesh: Option<(&[[f32; 3]], &[u32])>,
) -> Option<Collider> {
    try_build_shape(shape, scale, mesh)
        .map_err(|skip| log::warn!("[Physics] entity {id}: mesh collider skipped: {skip}"))
        .ok()
}

/// Build a parry collider shape from the engine's `ColliderShape`, baking in the
/// transform's (lossy, non-uniform) world scale by [`ColliderShape::scaled`], the
/// sizing rule the collider's world AABB reads too (#1140).
/// A mirror (a negative axis: `scale.x` for a child, as glam splits its world matrix;
/// any axis for an unparented entity, whose own scale arrives as-is) keeps a
/// primitive's size, Unity-style: primitives size from `scale.abs()`, and only a
/// mesh gets the signed scale and is reflected (#1124).
///
/// A mesh collider that cannot be built is a [`Skip`] saying why (#1132, #1141).
/// `mesh` is the entity's rest-pose positions + indices, for [`ColliderShape::Mesh`].
pub(in crate::physics) fn try_build_shape(
    shape: &ColliderShape,
    scale: Vec3,
    mesh: Option<(&[[f32; 3]], &[u32])>,
) -> Result<Collider, Skip> {
    let collider = match shape.scaled(scale) {
        ScaledShape::Cuboid { half } => ColliderBuilder::cuboid(half.x, half.y, half.z),
        ScaledShape::Ball { radius } => ColliderBuilder::ball(radius),
        ScaledShape::Cylinder {
            half_height,
            radius,
        } => ColliderBuilder::cylinder(half_height, radius),
        ScaledShape::Capsule {
            half_segment,
            radius,
            axis,
        } => match axis {
            CapsuleAxis::X => ColliderBuilder::capsule_x(half_segment, radius),
            CapsuleAxis::Y => ColliderBuilder::capsule_y(half_segment, radius),
            CapsuleAxis::Z => ColliderBuilder::capsule_z(half_segment, radius),
        },
        ScaledShape::Mesh { convex, .. } => {
            return build_mesh_shape(convex, scale, mesh.ok_or(Skip::NoMesh)?)
        }
    };
    Ok(collider.build())
}

/// rapier collision + solver groups for a collider on `layer`: it is a member of
/// its own layer bit, and collides only with the layers `filter_mask` permits — the
/// row the collision matrix (#91) provides for `layer`. A symmetric matrix makes the
/// pairwise rapier test reduce to `can_collide(a, b)`.
pub(in crate::physics) fn interaction_groups(layer: u8, filter_mask: u32) -> InteractionGroups {
    let membership = if (layer as usize) < 32 {
        1u32 << layer
    } else {
        0
    };
    InteractionGroups::new(
        Group::from_bits_truncate(membership),
        Group::from_bits_truncate(filter_mask),
        InteractionTestMode::And,
    )
}

/// Canonical (low, high) ordering so trigger-pair dedup is stable.
pub(in crate::physics) fn order_pair(a: u32, b: u32) -> (u32, u32) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}
