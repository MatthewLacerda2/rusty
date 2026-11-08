//! The segments of each shape kind, in the shape's own space (centred, unrotated).

use glam::Vec3;

use crate::components::{CapsuleAxis, ScaledShape};
use crate::core::collections::Set;

/// Segments in a full circle.
const CIRCLE: usize = 32;

/// A primitive's outline.
pub(super) fn primitive(shape: ScaledShape) -> Vec<[Vec3; 2]> {
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
        // A mesh is read from the shape physics built, never as a primitive.
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

/// A mesh collider's edges, read off the shape physics built: a hull's edges, each
/// convex piece's of a decomposed concave mesh on a dynamic body (#1886), or a
/// triangle mesh's, each once.
pub(super) fn built(collider: &rapier3d::prelude::Collider) -> Option<Vec<[Vec3; 2]>> {
    let shape = collider.shape();
    if let Some(hull) = shape.as_convex_polyhedron() {
        return Some(hull_edges(hull));
    }
    if let Some(compound) = shape.as_compound() {
        let pieces = compound.shapes().iter().filter_map(|(at, part)| {
            let edges = hull_edges(part.as_convex_polyhedron()?);
            Some(edges.into_iter().map(|s| s.map(|p| at.transform_point(p))))
        });
        return Some(pieces.flatten().take(super::MAX_SEGMENTS).collect());
    }
    let mesh = shape.as_trimesh()?;
    let tris = mesh
        .indices()
        .iter()
        .map(|t| t.map(|i| mesh.vertices()[i as usize]));
    Some(unique_edges(tris))
}

/// A convex hull's edges (parry merges coplanar faces: a box hull has twelve).
fn hull_edges(hull: &rapier3d::parry::shape::ConvexPolyhedron) -> Vec<[Vec3; 2]> {
    let points = hull.points();
    let edges = hull.edges().iter();
    edges
        .map(|e| e.vertices.map(|i| points[i as usize]))
        .collect()
}

/// Each edge of `triangles` once, by its ends' positions (not their indices: a
/// seam split for its normals repeats a position under two indices), in the order
/// first met.
pub(super) fn unique_edges(triangles: impl Iterator<Item = [Vec3; 3]>) -> Vec<[Vec3; 2]> {
    let key = |p: Vec3| p.to_array().map(f32::to_bits);
    let mut seen = Set::default();
    let mut out = Vec::new();
    for [a, b, c] in triangles {
        for [p, q] in [[a, b], [b, c], [c, a]] {
            let (kp, kq) = (key(p), key(q));
            if seen.insert(if kp <= kq { (kp, kq) } else { (kq, kp) }) {
                out.push([p, q]);
            }
        }
        if out.len() >= super::MAX_SEGMENTS {
            break;
        }
    }
    out
}
