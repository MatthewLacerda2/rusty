//! **Scale.** The mesh is decomposed once, at unit scale, and the body's scale is
//! applied to the hulls, as PhysX cooks a convex mesh once and scales the
//! cooked hulls (`PxMeshScale`). A linear map keeps every hull convex, so a
//! stretched or mirrored prop gets the stretched hulls, not a fresh decomposition
//! of the stretched shape, and a scaled copy costs no V-HACD run.

use std::sync::{Arc, Mutex};

use glam::Vec3;
use rapier3d::parry::transformation::vhacd::{VHACDParameters, VHACD};
use rapier3d::prelude::*;

use super::hull::has_volume;
use rusty_core::hulls::{self, Hull, Key, Pieces};

#[cfg(test)]
mod body_tests;
#[cfg(test)]
mod fingerprint_tests;
#[cfg(test)]
mod panic_tests;
#[cfg(test)]
mod refit_tests;
#[cfg(test)]
mod scale_tests;
#[cfg(all(test, any(feature = "editor", feature = "dev")))]
mod sidecar_tests;
#[cfg(test)]
mod tests;

const KEPT: usize = 256;

type Built = (Key, [u32; 3]);

static KEPT_SHAPES: Mutex<Vec<(Built, Option<SharedShape>)>> = Mutex::new(Vec::new());

pub(crate) fn decomposition(
    positions: &[[f32; 3]],
    indices: &[u32],
    scale: Vec3,
) -> Option<SharedShape> {
    let mesh = hulls::key(positions.iter().copied(), indices);
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

pub(crate) fn decompose(points: &[Vector], triangles: &[[u32; 3]]) -> Vec<Hull> {
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

pub(crate) fn assemble(pieces: &[Hull], scale: Vec3) -> Option<SharedShape> {
    let parts: Vec<(Pose, SharedShape)> = (pieces.iter())
        .filter_map(|h| piece(h, scale))
        .map(|s| (Pose::IDENTITY, s))
        .collect();
    (!parts.is_empty()).then(|| SharedShape::compound(parts))
}

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

#[cfg(all(test, any(feature = "editor", feature = "dev")))]
pub(crate) fn forget_built(mesh: Key) {
    let mut kept = KEPT_SHAPES.lock().unwrap_or_else(|e| e.into_inner());
    kept.retain(|((k, _), _)| *k != mesh);
}

#[cfg(test)]
thread_local! {
    pub(crate) static PANIC_NEXT: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    pub(crate) static RUNS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}
