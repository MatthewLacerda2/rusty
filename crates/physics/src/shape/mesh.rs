//! crates/physics/src/shape/mesh.rs — a mesh collider's rapier shape (#77).
//!
//! The one shape [`super::build::try_build_body_shape`] builds from geometry rather than
//! dimensions, so the one that a mirrored world scale must reflect (#1124). This file
//! also owns the validation of that geometry: every way a mesh can fail to become a
//! collider is a [`Skip`], which the caller logs once, naming the entity (#1141).

use std::fmt;

use glam::Vec3;
use rapier3d::prelude::*;

/// Why a mesh collider was not built.
#[derive(Debug, PartialEq)]
pub(crate) enum Skip {
    /// The entity has a Mesh collider but no Mesh component to take geometry from.
    NoMesh,
    /// Fewer vertices than a hull (4) or a trimesh (3) needs.
    TooFewVertices { have: usize, need: usize },
    /// A vertex has a NaN or infinite coordinate.
    NonFinite { vertex: usize },
    /// A triangle index points past the end of the vertex list.
    IndexOutOfRange { index: u32, vertices: usize },
    /// A trimesh without a whole triangle.
    NoTriangles,
    /// Every triangle of the trimesh has (nearly) zero area, so rays would pass through.
    ZeroArea,
    /// The points are collinear or coincident: parry finds no hull.
    NoHull,
    /// Parry refused the trimesh for another reason.
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

/// What a mesh collider is built as.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MeshFit {
    /// One convex hull of every vertex (`convex: true`).
    Hull,
    /// The exact triangle mesh: a concave collider on a static or kinematic body.
    Exact,
    /// Convex pieces from V-HACD (`decompose`, #1886): a concave collider on a
    /// dynamic body. A mesh with no usable decomposition stays [`Self::Exact`].
    Decomposed,
}

impl MeshFit {
    /// The fit for a collider's `convex` flag on a body that is `dynamic` or not.
    pub(crate) fn of(convex: bool, dynamic: bool) -> Self {
        match (convex, dynamic) {
            (true, _) => Self::Hull,
            (false, false) => Self::Exact,
            (false, true) => Self::Decomposed,
        }
    }
}

/// Build a trimesh, decomposed or convex-hull collider from rest-pose mesh geometry (#77),
/// baking the signed world scale into each point. A mirror (odd number of negative
/// axes) would turn every triangle inside out, so it swaps two indices of each
/// (#1124). The [`Skip`] says why not, when the geometry cannot form a valid shape:
/// a hull needs 4 vertices and some volume to span, a trimesh one triangle of
/// non-zero area whose indices all point at a vertex (#1132, #1141). Either
/// shape needs every coordinate finite (#1880). A hull reads no
/// indices, so a stale one never drops it. A decomposition is held to a trimesh's
/// checks, made from the unscaled mesh and scaled after (#1940), and falls back to
/// the trimesh when V-HACD finds no piece with volume.
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
        // A convex hull is the cheaper, dynamic-body-friendly option.
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

/// Every trimesh collider's flags (#1880). `FIX_INTERNAL_EDGES` corrects a contact
/// on the shared edge of two triangles to the neighbours' normal, so a body sliding
/// over a floor of tiles does not snag or hop at the seams ("ghost collisions").
/// It implies `MERGE_DUPLICATE_VERTICES`, which lets two triangles that repeat a
/// vertex share the edge; it removes no triangle, so the checks below, made on the
/// unmerged geometry, still decide every [`Skip`].
pub(crate) const TRIMESH_FLAGS: TriMeshFlags = TriMeshFlags::FIX_INTERNAL_EDGES;

/// A mesh's triangles, checked: what an exact trimesh (static geometry, a tight
/// fit) and a decomposition are built from. Mirrors the winding for a reflecting
/// `scale`; refuses a bad index, an empty or an all-degenerate mesh.
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
    // The mesh's own size (bounding-box diagonal), not its distance from the origin.
    let (min, max) = points.iter().fold((points[0], points[0]), |(lo, hi), &p| {
        (lo.min(p), hi.max(p))
    });
    let epsilon = 1e-6 * (max - min).length_squared();
    if triangles.iter().all(|t| is_degenerate(points, t, epsilon)) {
        return Err(Skip::ZeroArea);
    }
    Ok(triangles)
}

/// A triangle whose edge cross product is no larger than `epsilon` (#1141).
fn is_degenerate(points: &[Vector], t: &[u32; 3], epsilon: f32) -> bool {
    let p = |i: u32| points[i as usize];
    let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
    (b - a).cross(c - a).length() <= epsilon
}
