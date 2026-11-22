use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use rusty_components::{
    CapsuleAxis, ColliderShape, CollisionDetection, PhysicsMaterial, RigidBodyComponent,
    ScaledShape,
};
use rusty_ecs::World;
use rusty_scene::Scene;

use super::mesh::{build_mesh_shape, MeshFit, Skip};
use crate::compound::world_pose;
use crate::contact;
use crate::writes::Damping;

pub(crate) enum BodyClass {
    Static,
    Kinematic,
    Dynamic,
}

pub(crate) fn classify(is_static: bool, rb: Option<&RigidBodyComponent>) -> BodyClass {
    if is_static {
        BodyClass::Static
    } else if rb.is_none_or(|r| r.is_kinematic) {
        BodyClass::Kinematic
    } else {
        BodyClass::Dynamic
    }
}

pub(crate) fn class_of(world: &World, id: u32) -> BodyClass {
    if world.has_character_controller(id) {
        return BodyClass::Kinematic;
    }
    classify(world.is_static(id), world.rigidbody(id).as_deref())
}

pub(crate) fn gravity_scale(use_gravity: bool) -> f32 {
    if use_gravity {
        1.0
    } else {
        0.0
    }
}

pub(crate) fn ccd_enabled(mode: CollisionDetection) -> bool {
    matches!(mode, CollisionDetection::Continuous)
}

pub(crate) struct ColliderInputs {
    pub shape: ColliderShape,
    pub center: Vec3,
    /// An upright shape ignores the entity's rotation — a CharacterController's
    /// capsule stays vertical however the entity turns.
    pub upright: bool,
    pub mesh_geom: Option<(Vec<[f32; 3]>, Vec<u32>)>,
    pub is_trigger: bool,
    pub material: PhysicsMaterial,
    pub layer: u8,
}

pub(crate) fn collider_inputs(world: &World, id: u32) -> Option<ColliderInputs> {
    if let Some(inputs) = crate::character::character_collider_inputs(world, id) {
        return Some(inputs);
    }
    let collider = world.collider(id).filter(|c| c.active)?;
    // The collision mesh wins while it has triangles; else the entity's own
    // Mesh; else the copy kept when that Mesh was removed.
    let mesh_geom = if matches!(collider.shape, ColliderShape::Mesh { .. }) {
        let own_copy = |s: &rusty_components::SharedMesh| (s.positions.clone(), s.indices.to_vec());
        collider
            .built_collision_mesh()
            .map(own_copy)
            .or_else(|| {
                world.mesh(id).map(|m| {
                    (
                        m.vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
                        m.indices.to_vec(),
                    )
                })
            })
            .or_else(|| collider.shared_mesh.as_ref().map(own_copy))
    } else {
        None
    };
    Some(ColliderInputs {
        shape: collider.shape.clone(),
        center: Vec3::ZERO,
        upright: false,
        mesh_geom,
        is_trigger: collider.is_trigger,
        material: rusty_scene::physics_material::effective(&collider),
        layer: world.layer(id),
    })
}

pub(crate) struct BodyInputs {
    pub class: BodyClass,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub use_gravity: bool,
    pub collision_detection: CollisionDetection,
    pub damping: Damping,
}

pub(crate) fn body_inputs(world: &World, id: u32) -> BodyInputs {
    let rb = world.rigidbody(id);
    let rb = rb.as_deref();
    BodyInputs {
        class: class_of(world, id),
        velocity: rb.map(|r| r.velocity).unwrap_or(Vec3::ZERO),
        angular_velocity: rb.map(|r| r.angular_velocity).unwrap_or(Vec3::ZERO),
        use_gravity: rb.is_none_or(|r| r.use_gravity),
        collision_detection: rb.map(|r| r.collision_detection).unwrap_or_default(),
        damping: Damping::of(rb),
    }
}

pub(crate) struct EntityBodyState {
    pub pos: Vec3,
    pub rot: Quat,
    pub vel: Vec3,
    pub angular_velocity: Vec3,
    pub kinematic: bool,
    pub is_static: bool,
    pub use_gravity: bool,
    pub ccd_enabled: bool,
    pub damping: Damping,
}

pub(crate) fn body_state(scene: &Scene, id: u32) -> Option<EntityBodyState> {
    let pose = world_pose(scene, id)?;
    let world = &scene.world;
    let class = class_of(world, id);
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
        damping: Damping::of(rb),
    })
}

#[cfg(test)]
pub(crate) fn build_shape(
    shape: &ColliderShape,
    scale: Vec3,
    mesh: Option<(&[[f32; 3]], &[u32])>,
) -> Option<Collider> {
    try_build_shape(shape, scale, mesh).ok()
}

/// A mirror (a negative axis: `scale.x` for a child, as glam splits its world matrix;
/// any axis for an unparented entity, whose own scale arrives as-is) keeps a
/// primitive's size, Unity-style: primitives size from `scale.abs()`, and only a
/// mesh gets the signed scale and is reflected.
#[cfg(test)]
pub(crate) fn try_build_shape(
    shape: &ColliderShape,
    scale: Vec3,
    mesh: Option<(&[[f32; 3]], &[u32])>,
) -> Result<Collider, Skip> {
    try_build_body_shape(shape, scale, mesh, false)
}

pub(crate) fn try_build_body_shape(
    shape: &ColliderShape,
    scale: Vec3,
    mesh: Option<(&[[f32; 3]], &[u32])>,
    dynamic: bool,
) -> Result<Collider, Skip> {
    let collider = match shape.scaled(scale) {
        ScaledShape::Cuboid { half } => ColliderBuilder::cuboid(half.x, half.y, half.z),
        ScaledShape::Ball { radius } => ColliderBuilder::ball(radius),
        ScaledShape::Cylinder {
            half_height,
            radius,
        } => contact::cylinder(half_height, radius, dynamic),
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
            let fit = MeshFit::of(convex, dynamic);
            return build_mesh_shape(fit, scale, mesh.ok_or(Skip::NoMesh)?);
        }
    };
    Ok(contact::rolling(collider, shape, dynamic).build())
}

pub(crate) fn interaction_groups(layer: u8, filter_mask: u32) -> InteractionGroups {
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

pub(crate) fn order_pair(a: u32, b: u32) -> (u32, u32) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}
