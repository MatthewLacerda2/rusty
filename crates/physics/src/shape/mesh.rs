use std::fmt;

use glam::Vec3;
use rapier3d::prelude::*;

#[derive(Debug, PartialEq)]
pub(crate) enum Skip {
    NoMesh,
    TooFewVertices { have: usize, need: usize },
    NonFinite { vertex: usize },
    IndexOutOfRange { index: u32, vertices: usize },
    NoTriangles,
    ZeroArea,
    NoHull,
    Rejected(String),
}

impl fmt::Display for Skip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoMesh => write!(f, "has a Mesh collider but no mesh"),
            Self::TooFewVertices { have, need } => write!(f, "{have} vertices, {need} needed"),
            Self::NonFinite { vertex } => write!(f, "vertex {vertex} is not finite"),
            Self::IndexOutOfRange { index, vertices } => {
                write!(f, "index {index} is past the {vertices} vertices")
            }
            Self::NoTriangles => write!(f, "a trimesh needs at least one triangle"),
            Self::ZeroArea => write!(f, "every triangle has zero area"),
            Self::NoHull => write!(f, "the points form no convex hull"),
            Self::Rejected(why) => write!(f, "{why}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MeshFit {
    Hull,
    Exact,
    Decomposed,
}

impl MeshFit {
    pub(crate) fn of(convex: bool, dynamic: bool) -> Self {
        match (convex, dynamic) {
            (true, _) => Self::Hull,
            (false, false) => Self::Exact,
            (false, true) => Self::Decomposed,
        }
    }
}

pub(crate) fn build_mesh_shape(
    fit: MeshFit,
    scale: Vec3,
    mesh: (&[[f32; 3]], &[u32]),
) -> Result<Collider, Skip> {
    let (positions, indices) = mesh;
    let need = if fit == MeshFit::Hull { 4 } else { 3 };
    if positions.len() < need {
        return Err(Skip::TooFewVertices {
            have: positions.len(),
            need,
        });
    }
    let points: Vec<Vector> = positions
        .iter()
        .map(|p| Vector::new(p[0] * scale.x, p[1] * scale.y, p[2] * scale.z))
        .collect();
    if let Some(vertex) = points.iter().position(|p| !p.is_finite()) {
        return Err(Skip::NonFinite { vertex });
    }

    if fit == MeshFit::Hull {
        return ColliderBuilder::convex_hull(&points)
            .map(|b| b.build())
            .ok_or(Skip::NoHull);
    }
    let triangles = triangles(&points, indices, scale)?;
    let pieces = (fit == MeshFit::Decomposed)
        .then(|| super::decompose::decomposition(positions, indices, scale))
        .flatten();
    match pieces {
        Some(shape) => Ok(ColliderBuilder::new(shape).build()),
        None => ColliderBuilder::trimesh_with_flags(points, triangles, TRIMESH_FLAGS)
            .map(|b| b.build())
            .map_err(|e| Skip::Rejected(e.to_string())),
    }
}

/// Every trimesh collider's flags. `FIX_INTERNAL_EDGES` corrects a contact on two
/// triangles' shared edge to their normal, so a body sliding over tiles does not hop at a
/// seam. It implies merging duplicate vertices, which drops no triangle, so the
/// unmerged checks in `triangles` still decide every `Skip`.
pub(crate) const TRIMESH_FLAGS: TriMeshFlags = TriMeshFlags::FIX_INTERNAL_EDGES;

fn triangles(points: &[Vector], indices: &[u32], scale: Vec3) -> Result<Vec<[u32; 3]>, Skip> {
    if let Some(&index) = indices.iter().find(|&&i| i as usize >= points.len()) {
        return Err(Skip::IndexOutOfRange {
            index,
            vertices: points.len(),
        });
    }
    // Counted, not multiplied: a product of tiny axes can underflow to -0.0.
    let mirrored = (scale.x < 0.0) ^ (scale.y < 0.0) ^ (scale.z < 0.0);
    let triangles: Vec<[u32; 3]> = indices
        .chunks_exact(3)
        .map(|c| {
            if mirrored {
                [c[0], c[2], c[1]]
            } else {
                [c[0], c[1], c[2]]
            }
        })
        .collect();
    if triangles.is_empty() {
        return Err(Skip::NoTriangles);
    }
    // A cross product (twice the area) under a millionth of the squared bounding-box
    // diagonal is zero: the mesh's own size, not its distance from the origin.
    let (min, max) = points.iter().fold((points[0], points[0]), |(lo, hi), &p| {
        (lo.min(p), hi.max(p))
    });
    let epsilon = 1e-6 * (max - min).length_squared();
    if triangles.iter().all(|t| is_degenerate(points, t, epsilon)) {
        return Err(Skip::ZeroArea);
    }
    Ok(triangles)
}

fn is_degenerate(points: &[Vector], t: &[u32; 3], epsilon: f32) -> bool {
    let p = |i: u32| points[i as usize];
    let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
    (b - a).cross(c - a).length() <= epsilon
}
