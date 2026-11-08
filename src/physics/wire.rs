//! src/physics/wire.rs — a collider's shape as world-space line segments, for the
//! collider gizmo (#1882): Unity's green wireframe of the shape physics collides
//! with, not of its bounds.
//!
//! Primitives are drawn as Unity outlines them, from the same scaled sizes the
//! rapier shape is built from ([`ColliderShape::scaled`]): a box's twelve edges, a
//! sphere's three great circles, a cylinder's two rims and four sides, a capsule's
//! two rims, four sides and its caps' arcs. A convex mesh draws its hull's edges
//! (parry merges coplanar faces, so a box hull shows twelve, not its diagonals); a
//! triangle mesh draws each edge of its triangles once. A CharacterController's
//! capsule draws too, upright at its centre, as physics places it.
//!
//! **Bounded.** A level's collision mesh can hold far more edges than a frame
//! should upload, so a caller passes a `budget` of segments: a triangle mesh stops
//! reading triangles once it has that many edges, and draws only the first
//! `budget`. Pure geometry: nothing here touches the physics world.

use glam::{Quat, Vec3};

use super::shape::build::{build_shape_for, collider_inputs};
use crate::components::{CapsuleAxis, ScaledShape};
use crate::scene::Scene;

/// Segments in a full circle.
const CIRCLE: usize = 32;

/// One collider's wireframe.
#[derive(Debug, Default)]
pub struct ColliderWire {
    /// World-space segments, each its two ends.
    pub segments: Vec<[Vec3; 2]>,
    /// The collider is a trigger (Unity draws it the same, paler here).
    pub trigger: bool,
}

/// `id`'s active collider (or CharacterController) as at most `budget` world-space
/// segments, or `None` when it has neither, or a mesh that builds no shape.
pub fn collider_wire(scene: &Scene, id: u32, budget: usize) -> Option<ColliderWire> {
    let inp = collider_inputs(&scene.world, id)?;
    let pose = scene.world.world_pose(id)?;
    let (pos, rot) = match inp.upright {
        true => (
            pose.pos + pose.rot * (inp.center * pose.scale),
            Quat::IDENTITY,
        ),
        false => (pose.pos, pose.rot),
    };
    let mut local = match inp.shape.scaled(pose.scale) {
        ScaledShape::Mesh { convex: true, .. } => {
            let mesh = inp.mesh_geom.as_ref();
            let mesh = mesh.map(|(p, i)| (p.as_slice(), i.as_slice()));
            hull_edges(&build_shape_for(id, &inp.shape, pose.scale, mesh)?)?
        }
        ScaledShape::Mesh { convex: false, .. } => {
            let (points, indices) = inp.mesh_geom.as_ref()?;
            mesh_edges(points, indices, pose.scale, budget)
        }
        primitive => primitive_edges(primitive),
    };
    local.truncate(budget);
    for seg in &mut local {
        *seg = seg.map(|p| pos + rot * p);
    }
    Some(ColliderWire {
        segments: local,
        trigger: inp.is_trigger,
    })
}

/// A primitive's outline in its own space (centred, unrotated).
fn primitive_edges(shape: ScaledShape) -> Vec<[Vec3; 2]> {
    match shape {
        ScaledShape::Cuboid { half } => box_edges(half),
        ScaledShape::Ball { radius } => {
            let mut out = circle(Vec3::ZERO, Vec3::X, Vec3::Y, radius, 1.0);
            out.extend(circle(Vec3::ZERO, Vec3::Y, Vec3::Z, radius, 1.0));
            out.extend(circle(Vec3::ZERO, Vec3::Z, Vec3::X, radius, 1.0));
            out
        }
        ScaledShape::Cylinder {
            half_height,
            radius,
        } => tube(CapsuleAxis::Y, half_height, radius),
        ScaledShape::Capsule {
            half_segment,
            radius,
            axis,
        } => {
            let mut out = tube(axis, half_segment, radius);
            let (u, v) = across(axis);
            let along = axis.unit();
            for sign in [1.0, -1.0] {
                let centre = along * half_segment * sign;
                // Half circles from one side to the other, over the cap's pole.
                out.extend(circle(centre, u, along * sign, radius, 0.5));
                out.extend(circle(centre, v, along * sign, radius, 0.5));
            }
            out
        }
        // A mesh is read from its geometry, never as a primitive.
        ScaledShape::Mesh { .. } => Vec::new(),
    }
}

/// Two rims `half` along `axis` from the centre, `radius` round, and four sides.
fn tube(axis: CapsuleAxis, half: f32, radius: f32) -> Vec<[Vec3; 2]> {
    let (u, v) = across(axis);
    let along = axis.unit() * half;
    let mut out = circle(along, u, v, radius, 1.0);
    out.extend(circle(-along, u, v, radius, 1.0));
    for side in [u, -u, v, -v] {
        out.push([along + side * radius, -along + side * radius]);
    }
    out
}

/// Two unit vectors across `axis`, completing a right-handed frame.
fn across(axis: CapsuleAxis) -> (Vec3, Vec3) {
    match axis {
        CapsuleAxis::X => (Vec3::Y, Vec3::Z),
        CapsuleAxis::Y => (Vec3::Z, Vec3::X),
        CapsuleAxis::Z => (Vec3::X, Vec3::Y),
    }
}

/// `turns` of a circle round `centre` in the plane of `u` and `v`, from `u`.
fn circle(centre: Vec3, u: Vec3, v: Vec3, radius: f32, turns: f32) -> Vec<[Vec3; 2]> {
    let steps = ((CIRCLE as f32 * turns).ceil() as usize).max(1);
    let step = std::f32::consts::TAU * turns / steps as f32;
    let at = |i: usize| {
        let (s, c) = (step * i as f32).sin_cos();
        centre + (u * c + v * s) * radius
    };
    (0..steps).map(|i| [at(i), at(i + 1)]).collect()
}

/// A box's twelve edges.
fn box_edges(h: Vec3) -> Vec<[Vec3; 2]> {
    let corner = |i: usize| {
        let pick = |bit: usize, x: f32| if i & bit == 0 { -x } else { x };
        Vec3::new(pick(1, h.x), pick(2, h.y), pick(4, h.z))
    };
    let mut out = Vec::with_capacity(12);
    for i in 0..8 {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                out.push([corner(i), corner(i | bit)]);
            }
        }
    }
    out
}

/// A built convex mesh's hull edges, in its own space.
fn hull_edges(collider: &rapier3d::prelude::Collider) -> Option<Vec<[Vec3; 2]>> {
    let hull = collider.shape().as_convex_polyhedron()?;
    let points = hull.points();
    let edges = hull.edges().iter().map(|e| {
        let [a, b] = e.vertices.map(|i| points[i as usize]);
        [a, b]
    });
    Some(edges.collect())
}

/// A triangle mesh's edges, each once, its points times the signed `scale` as
/// physics builds it; read until `budget` edges are found.
fn mesh_edges(points: &[[f32; 3]], indices: &[u32], scale: Vec3, budget: usize) -> Vec<[Vec3; 2]> {
    let mut pairs = Vec::new();
    for tri in indices.chunks_exact(3) {
        if pairs.len() >= budget.saturating_mul(3) {
            break;
        }
        for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            pairs.push((a.min(b), a.max(b)));
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    let point = |i: u32| points.get(i as usize).map(|p| Vec3::from(*p) * scale);
    pairs
        .into_iter()
        .filter_map(|(a, b)| Some([point(a)?, point(b)?]))
        .take(budget)
        .collect()
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
