//! src/physics/build.rs — body/collider construction helpers.
//!
//! Pure mapping from the engine's component model to rapier builders, plus the
//! body-class decision and the per-tick body-state snapshot. Kept out of
//! `world.rs` so that file stays focused on the step/sync lifecycle (and under
//! the size cap).

use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use crate::components::{
    CapsuleAxis, ColliderShape, CollisionDetection, PhysicsMaterial, RigidBodyComponent,
};
use crate::ecs::World;
use crate::scene::Scene;

use super::compound::world_pose;

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

/// Owner `id`'s body class. A CharacterController's body is always kinematic
/// (#451): it moves only where `CharacterController.Move` puts its Transform.
pub(super) fn class_of(world: &World, id: u32) -> BodyClass {
    if world.has_character_controller(id) {
        return BodyClass::Kinematic;
    }
    classify(world.is_static(id), world.rigidbody(id).as_deref())
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

/// Whether rapier's continuous collision detection is switched on for a body's
/// `collision_detection` mode (#321): `Continuous` sweeps the body's motion and
/// stops it at the time of impact (anti-tunnelling), `Discrete` (default) tests
/// overlap only at the tick's final pose. rapier already builds a `CCDSolver`
/// into every step — this flag is the per-body switch that was dormant.
pub(super) fn ccd_enabled(mode: CollisionDetection) -> bool {
    matches!(mode, CollisionDetection::Continuous)
}

/// The per-entity inputs needed to build one rapier collider, captured in a
/// single borrow of the entity so `assemble` can release it before touching
/// `self`. The pose is not here: it comes from the hierarchy (`compound`).
pub(super) struct ColliderInputs {
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
pub(super) fn collider_inputs(world: &World, id: u32) -> Option<ColliderInputs> {
    if let Some(inputs) = super::character::character_collider_inputs(world, id) {
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
pub(super) struct BodyInputs {
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
pub(super) fn body_inputs(world: &World, id: u32) -> BodyInputs {
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
pub(super) struct EntityBodyState {
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
pub(super) fn body_state(scene: &Scene, id: u32) -> Option<EntityBodyState> {
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
        ColliderShape::Capsule {
            radius,
            height,
            axis,
        } => capsule_builder(*radius, *height, *axis, scale).build(),
        ColliderShape::Mesh { convex, .. } => return build_mesh_shape(*convex, scale, mesh?),
    };
    Some(collider)
}

/// A capsule's rapier builder with the world scale baked in, Unity-style: the
/// full length scales with the axis' scale, the radius with the larger of the
/// two cross-axis scales. rapier takes the half-length of the inner segment
/// (caps excluded), so a full length under `2r` collapses to a sphere.
fn capsule_builder(radius: f32, height: f32, axis: CapsuleAxis, scale: Vec3) -> ColliderBuilder {
    let (half_segment, r) = capsule_dims(radius, height, axis, scale);
    match axis {
        CapsuleAxis::X => ColliderBuilder::capsule_x(half_segment, r),
        CapsuleAxis::Y => ColliderBuilder::capsule_y(half_segment, r),
        CapsuleAxis::Z => ColliderBuilder::capsule_z(half_segment, r),
    }
}

/// A capsule's `(half_segment, radius)` with the world scale baked in, as
/// [`capsule_builder`] builds it — shared with the CharacterController's sweep so
/// the shape it moves is the shape others collide with.
pub(super) fn capsule_dims(radius: f32, height: f32, axis: CapsuleAxis, scale: Vec3) -> (f32, f32) {
    let (along, cross) = match axis {
        CapsuleAxis::X => (scale.x, scale.y.max(scale.z)),
        CapsuleAxis::Y => (scale.y, scale.x.max(scale.z)),
        CapsuleAxis::Z => (scale.z, scale.x.max(scale.y)),
    };
    let r = (radius * cross).max(1e-4);
    (((height * along * 0.5) - r).max(0.0), r)
}

/// Build a trimesh or convex-hull collider from rest-pose mesh geometry (#77),
/// baking the world scale into each point. `None` if the geometry is too sparse to
/// form a valid shape.
fn build_mesh_shape(convex: bool, scale: Vec3, mesh: (&[[f32; 3]], &[u32])) -> Option<Collider> {
    let (positions, indices) = mesh;
    if positions.len() < 4 {
        return None;
    }
    let points: Vec<Vector> = positions
        .iter()
        .map(|p| Vector::new(p[0] * scale.x, p[1] * scale.y, p[2] * scale.z))
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
        ColliderBuilder::trimesh(points, triangles)
            .ok()
            .map(|b| b.build())
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
        InteractionTestMode::And,
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
