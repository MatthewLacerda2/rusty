//! src/physics/shape/decompose/mod.rs — a concave mesh as convex pieces, for a dynamic
//! body (#1886).
//!
//! A triangle mesh has no volume, so on a dynamic body it gets no real mass or
//! inertia, and two of them only touch at their surfaces and tunnel. Unity refuses
//! the case; rusty splits the mesh instead, with parry's V-HACD at its default
//! parameters, into a compound of convex hulls that simulates as one body. Static
//! and kinematic bodies keep the exact trimesh (`mesh`).
//!
//! **Scale.** The mesh is decomposed once, at unit scale, and the body's scale is
//! applied to the hulls (#1940), as PhysX cooks a convex mesh once and scales the
//! cooked hulls (`PxMeshScale`). A linear map keeps every hull convex, so a
//! stretched or mirrored prop gets the stretched hulls, not a fresh decomposition
//! of the stretched shape, and a scaled copy costs no V-HACD run.
//!
//! **Where the pieces come from.** `core::hulls` keeps every decomposition this
//! process knows, keyed by the unscaled mesh: read from the model's hull sidecar
//! when it was imported (`asset::hull_sidecar`), or made here by a V-HACD run and
//! marked fresh, for a dev build to write into that sidecar. So V-HACD runs once
//! per mesh, not once per session.
//!
//! **Determinism.** parry's V-HACD is single-threaded and has no randomness: it
//! voxelises the mesh, cuts the voxels along planes picked by a fixed search, and
//! hulls each part. The same points and triangles give the same hulls, bit for bit
//! (`tests`), so pieces read from a sidecar are the ones a run would make.
//!
//! **Cost.** The built shape is kept too, per mesh and scale: entering Play again,
//! or the twentieth copy of a prop, reuses it. A mesh with no usable
//! decomposition is kept as no pieces, so it is not retried.
//!
//! **Panics.** V-HACD's plane search hulls clipped points through parry's
//! `convex_hull`, which unwraps quickhull and so panics on a degeneracy. A panic
//! inside a decomposition is caught and treated as no decomposition: kept as no
//! pieces (never written to a sidecar), so the caller falls back to the trimesh
//! and warns, naming the entity.

use std::sync::{Arc, Mutex};

use glam::Vec3;
use rapier3d::parry::transformation::vhacd::{VHACDParameters, VHACD};
use rapier3d::prelude::*;

use super::hull::has_volume;
use crate::core::hulls::{self, Hull, Key, Pieces};

#[cfg(test)]
mod body_tests;
#[cfg(test)]
mod panic_tests;
#[cfg(test)]
mod refit_tests;
#[cfg(test)]
mod scale_tests;
#[cfg(test)]
mod sidecar_tests;
#[cfg(test)]
mod tests;

/// How many built shapes are kept: every distinct concave prop of a level at its
/// scales. Past it the oldest is dropped.
const KEPT: usize = 256;

/// A built shape's key: the mesh's and the scale's bits.
type Built = (Key, [u32; 3]);

/// The shapes built this session, oldest first (`None`: no usable pieces).
static KEPT_SHAPES: Mutex<Vec<(Built, Option<SharedShape>)>> = Mutex::new(Vec::new());

/// The convex decomposition of the mesh `positions` and `indices` (unscaled; every
/// index in range, checked by the caller), scaled by `scale`, or `None` when
/// V-HACD finds no piece with volume: a flat or degenerate mesh, which the caller
/// keeps as a trimesh.
pub(in crate::physics) fn decomposition(
    positions: &[[f32; 3]],
    indices: &[u32],
    scale: Vec3,
) -> Option<SharedShape> {
    let mesh = hulls::key(positions, indices);
    let built = (mesh, scale.to_array().map(f32::to_bits));
    let mut kept = KEPT_SHAPES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, shape)) = kept.iter().find(|(k, _)| *k == built) {
        return shape.clone();
    }
    let pieces = hulls::get(mesh).unwrap_or_else(|| run(mesh, positions, indices));
    let shape = assemble(&pieces, scale);
    if kept.len() == KEPT {
        kept.remove(0);
    }
    kept.push((built, shape.clone()));
    shape
}

/// Decompose the mesh `mesh` and keep its pieces: fresh, to be written to its
/// sidecar, unless V-HACD panicked.
fn run(mesh: Key, positions: &[[f32; 3]], indices: &[u32]) -> Pieces {
    let points: Vec<Vector> = positions.iter().map(|&p| Vector::from_array(p)).collect();
    let triangles: Vec<[u32; 3]> = (indices.chunks_exact(3))
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    // AssertUnwindSafe: the closure only reads the two slices; nothing it could
    // leave half-written is read after a panic.
    let attempt = std::panic::AssertUnwindSafe(|| decompose(&points, &triangles));
    match std::panic::catch_unwind(attempt) {
        Ok(pieces) => {
            let pieces = Arc::new(pieces);
            hulls::keep_fresh(mesh, pieces.clone());
            pieces
        }
        Err(_) => {
            let none = Pieces::default();
            hulls::keep(mesh, none.clone());
            none
        }
    }
}

/// Run V-HACD on unit-scale `points` and `triangles`, and keep only the pieces
/// that enclose a volume. Not parry's `SharedShape::convex_decomposition`, which
/// panics on an empty compound.
pub(in crate::physics) fn decompose(points: &[Vector], triangles: &[[u32; 3]]) -> Vec<Hull> {
    #[cfg(test)]
    {
        if PANIC_NEXT.with(|p| p.replace(false)) {
            panic!("forced V-HACD panic (test seam)");
        }
        RUNS.with(|r| r.set(r.get() + 1));
    }
    let params = VHACDParameters::default();
    let vhacd = VHACD::decompose(&params, points, triangles, true);
    (vhacd
        .compute_exact_convex_hulls(points, triangles)
        .into_iter())
    .map(|(vertices, triangles)| Hull {
        points: vertices.iter().map(|v| v.to_array()).collect(),
        triangles,
    })
    .filter(|h| piece(h, Vec3::ONE).is_some())
    .collect()
}

/// The compound of `pieces` scaled by `scale`, or `None` when no piece keeps a
/// volume (none to begin with, or a zero scale axis).
pub(in crate::physics) fn assemble(pieces: &[Hull], scale: Vec3) -> Option<SharedShape> {
    let parts: Vec<(Pose, SharedShape)> = (pieces.iter())
        .filter_map(|h| piece(h, scale))
        .map(|s| (Pose::IDENTITY, s))
        .collect();
    (!parts.is_empty()).then(|| SharedShape::compound(parts))
}

/// One hull as a convex shape with a volume. At unit scale it is the hull V-HACD
/// gave, faces and all; scaled, it is hulled again from the scaled points, which
/// also turns a mirrored hull's faces the right way out.
fn piece(hull: &Hull, scale: Vec3) -> Option<SharedShape> {
    let at = |p: &[f32; 3]| Vector::from_array(*p) * scale;
    let points: Vec<Vector> = hull.points.iter().map(at).collect();
    let in_range = |t: &[u32; 3]| t.iter().all(|&i| (i as usize) < points.len());
    if !hull.triangles.iter().all(in_range) {
        return None;
    }
    let shape = if scale == Vec3::ONE {
        SharedShape::convex_mesh(points, &hull.triangles)
    } else {
        SharedShape::convex_hull(&points)
    };
    shape.filter(|s| s.as_convex_polyhedron().is_some_and(has_volume))
}

/// Forget every shape built from the mesh `mesh`, as a fresh launch has (tests).
#[cfg(test)]
pub(in crate::physics) fn forget_built(mesh: Key) {
    let mut kept = KEPT_SHAPES.lock().unwrap_or_else(|e| e.into_inner());
    kept.retain(|((k, _), _)| *k != mesh);
}

#[cfg(test)]
thread_local! {
    /// Set by a test to make this thread's next [`decompose`] panic, as quickhull
    /// does on a degenerate imported prop.
    pub(in crate::physics) static PANIC_NEXT: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    /// How many times this thread ran V-HACD.
    pub(in crate::physics) static RUNS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}
