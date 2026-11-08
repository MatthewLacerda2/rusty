//! src/physics/shape/decompose/mod.rs — a concave mesh as convex pieces, for a dynamic
//! body (#1886).
//!
//! A triangle mesh has no volume, so on a dynamic body it gets no real mass or
//! inertia, and two of them only touch at their surfaces and tunnel. Unity refuses
//! the case; rusty splits the mesh instead, with parry's V-HACD at its default
//! parameters, into a compound of convex hulls that simulates as one body. Static
//! and kinematic bodies keep the exact trimesh (`mesh`).
//!
//! **Determinism.** parry's V-HACD is single-threaded and has no randomness: it
//! voxelises the mesh, cuts the voxels along planes picked by a fixed search, and
//! hulls each part. The same points and triangles give the same hulls, bit for bit
//! (`tests`).
//!
//! **Cost.** A decomposition takes milliseconds to seconds, so the result is kept
//! for the session, keyed by the scaled points (the world scale is baked in, and a
//! non-uniform one changes the hulls) and the triangles: entering Play again, or
//! the twentieth copy of a prop, reuses it. A mesh with no usable decomposition is
//! kept too, as `None`, so it is not retried.

use std::sync::Mutex;

use rapier3d::parry::transformation::vhacd::{VHACDParameters, VHACD};
use rapier3d::prelude::*;

use super::hull::has_volume;

#[cfg(test)]
mod body_tests;
#[cfg(test)]
mod refit_tests;
#[cfg(test)]
mod tests;

/// How many decompositions are kept: every distinct concave prop of a level at its
/// scales. Past it the oldest is dropped.
const KEPT: usize = 256;

/// What a decomposition is keyed by: the content hash of the scaled points and the
/// triangles, and their counts.
type Key = (u64, usize, usize);

/// The decompositions made this session, oldest first (`None`: none usable).
static KEPT_SHAPES: Mutex<Vec<(Key, Option<SharedShape>)>> = Mutex::new(Vec::new());

/// The convex decomposition of the mesh `points` (scale already baked in) and
/// `triangles` (every index in range), or `None` when V-HACD finds no piece with
/// volume: a flat or degenerate mesh, which the caller keeps as a trimesh.
pub(in crate::physics) fn decomposition(
    points: &[Vector],
    triangles: &[[u32; 3]],
) -> Option<SharedShape> {
    let key = key(points, triangles);
    let mut kept = KEPT_SHAPES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, shape)) = kept.iter().find(|(k, _)| *k == key) {
        return shape.clone();
    }
    let shape = decompose(points, triangles);
    if kept.len() == KEPT {
        kept.remove(0);
    }
    kept.push((key, shape.clone()));
    shape
}

/// The cache key of a mesh: its bytes' content hash and its counts.
fn key(points: &[Vector], triangles: &[[u32; 3]]) -> Key {
    let mut bytes = Vec::with_capacity(points.len() * 12 + triangles.len() * 12);
    for p in points {
        bytes.extend(p.to_array().iter().flat_map(|c| c.to_bits().to_le_bytes()));
    }
    for t in triangles {
        bytes.extend(t.iter().flat_map(|i| i.to_le_bytes()));
    }
    (
        crate::core::hash::content(&bytes),
        points.len(),
        triangles.len(),
    )
}

/// Run V-HACD (parry's `SharedShape::convex_decomposition`, which panics on a mesh
/// that yields no hull) and keep only the pieces that enclose a volume.
pub(in crate::physics) fn decompose(
    points: &[Vector],
    triangles: &[[u32; 3]],
) -> Option<SharedShape> {
    #[cfg(test)]
    RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let params = VHACDParameters::default();
    let vhacd = VHACD::decompose(&params, points, triangles, true);
    let parts: Vec<(Pose, SharedShape)> = vhacd
        .compute_exact_convex_hulls(points, triangles)
        .into_iter()
        .filter_map(|(vertices, indices)| SharedShape::convex_mesh(vertices, &indices))
        .filter(|s| s.as_convex_polyhedron().is_some_and(has_volume))
        .map(|s| (Pose::IDENTITY, s))
        .collect();
    (!parts.is_empty()).then(|| SharedShape::compound(parts))
}

/// How many decompositions this process ran: a test's handle on the cache.
#[cfg(test)]
pub(in crate::physics) static RUNS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
