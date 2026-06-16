//! src/physics/build.rs — body/collider construction helpers.
//!
//! Pure mapping from the engine's component model to rapier builders, plus the
//! body-class decision. Kept out of `world.rs` so that file stays focused on the
//! step/sync lifecycle (and under the size cap).

use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use crate::components::{ColliderShape, Entity, RigidBodyComponent};

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
    pub layer: u8,
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
        layer: entity.layer,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ColliderShape;

    /// A unit cube (half-extent 0.5) as rest-pose positions + triangle indices.
    fn unit_cube() -> (Vec<[f32; 3]>, Vec<u32>) {
        let p = vec![
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
        ];
        #[rustfmt::skip]
        let i = vec![
            0, 1, 2, 0, 2, 3, 4, 6, 5, 4, 7, 6, 0, 4, 5,
            0, 5, 1, 3, 2, 6, 3, 6, 7, 0, 3, 7, 0, 7, 4, 1, 5, 6, 1, 6, 2,
        ];
        (p, i)
    }

    fn mesh_shape(convex: bool) -> ColliderShape {
        ColliderShape::Mesh {
            convex,
            local_min: Vec3::splat(-0.5),
            local_max: Vec3::splat(0.5),
        }
    }

    fn assert_cube_extent(collider: &Collider, half: f32) {
        let aabb = collider.shape().compute_local_aabb();
        for axis in 0..3 {
            assert!((aabb.maxs[axis] - half).abs() < 1e-4, "max {:?}", aabb.maxs);
            assert!((aabb.mins[axis] + half).abs() < 1e-4, "min {:?}", aabb.mins);
        }
    }

    #[test]
    fn convex_hull_from_mesh_matches_extents() {
        let (p, i) = unit_cube();
        let c = build_shape(&mesh_shape(true), Vec3::ONE, Some((&p, &i))).unwrap();
        assert_cube_extent(&c, 0.5);
    }

    #[test]
    fn trimesh_from_mesh_matches_extents() {
        let (p, i) = unit_cube();
        let c = build_shape(&mesh_shape(false), Vec3::ONE, Some((&p, &i))).unwrap();
        assert_cube_extent(&c, 0.5);
    }

    #[test]
    fn scale_is_baked_into_mesh_collider() {
        let (p, i) = unit_cube();
        let c = build_shape(&mesh_shape(false), Vec3::splat(2.0), Some((&p, &i))).unwrap();
        assert_cube_extent(&c, 1.0);
    }

    #[test]
    fn degenerate_mesh_yields_no_collider() {
        // Too few points for any shape.
        let sparse = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert!(build_shape(&mesh_shape(true), Vec3::ONE, Some((&sparse, &[0, 1, 2]))).is_none());
        // Enough points, but a trimesh needs triangles.
        let (p, _) = unit_cube();
        assert!(build_shape(&mesh_shape(false), Vec3::ONE, Some((&p, &[]))).is_none());
        // No geometry supplied at all.
        assert!(build_shape(&mesh_shape(true), Vec3::ONE, None).is_none());
    }

    #[test]
    fn analytic_shapes_still_build_without_mesh() {
        assert!(build_shape(&ColliderShape::Sphere { radius: 1.0 }, Vec3::ONE, None).is_some());
    }
}
