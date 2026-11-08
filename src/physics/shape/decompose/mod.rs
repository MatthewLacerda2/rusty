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
//!
//! **Panics.** V-HACD's plane search hulls clipped points through parry's
//! `convex_hull`, which unwraps quickhull and so panics on a degeneracy. A panic
//! inside a decomposition is caught and treated as no decomposition: kept as
//! `None`, so the caller falls back to the trimesh and warns, naming the entity.

use std::sync::Mutex;

use rapier3d::parry::transformation::vhacd::{VHACDParameters, VHACD};
use rapier3d::prelude::*;

use super::hull::has_volume;

#[cfg(test)]
mod body_tests;
#[cfg(test)]
mod panic_tests;
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
    let shape = decompose_or_none(points, triangles);
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

/// [`decompose`], with a panic inside V-HACD caught and read as `None`.
fn decompose_or_none(points: &[Vector], triangles: &[[u32; 3]]) -> Option<SharedShape> {
    // AssertUnwindSafe: the closure only reads the two slices; nothing it could
    // leave half-written is read after a panic.
    let run = std::panic::AssertUnwindSafe(|| decompose(points, triangles));
    std::panic::catch_unwind(run).ok().flatten()
}

/// Run V-HACD and keep only the pieces that enclose a volume. Not parry's
/// `SharedShape::convex_decomposition`, which panics on an empty compound.
pub(in crate::physics) fn decompose(
    points: &[Vector],
    triangles: &[[u32; 3]],
) -> Option<SharedShape> {
    #[cfg(test)]
    if PANIC_NEXT.with(|p| p.replace(false)) {
        panic!("forced V-HACD panic (test seam)");
    }
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

#[cfg(test)]
thread_local! {
    /// Set by a test to make this thread's next [`decompose`] panic, as quickhull
    /// does on a degenerate imported prop.
    pub(in crate::physics) static PANIC_NEXT: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}
