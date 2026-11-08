//! Candidate items (#1892): the generator's points, projected onto the navmesh.

use glam::Vec3;

use super::spec::{grid_side, Generator};
use crate::navigation::NavigationGraph;

/// The generator's points in generation order, each moved onto the walkable floor
/// nearest it within `project` metres; a point with none is dropped.
pub fn items(nav: &NavigationGraph, generator: &Generator, project: f32) -> Vec<Vec3> {
    raw_points(generator)
        .into_iter()
        .filter_map(|p| nav.sample_position(p, project))
        .collect()
}

/// The generator's points before projection.
pub(super) fn raw_points(generator: &Generator) -> Vec<Vec3> {
    match generator {
        Generator::Donut {
            center,
            inner,
            outer,
            rings,
            per_ring,
        } => donut(*center, (*inner, *outer), *rings, *per_ring),
        Generator::Grid {
            center,
            half_size,
            spacing,
        } => grid(*center, *half_size, *spacing),
        Generator::Points(points) => points.clone(),
    }
}

/// `rings` rings from `inner` to `outer` (one ring sits midway), `per_ring`
/// evenly spaced points on each, turning from +X toward +Z.
fn donut(center: Vec3, (inner, outer): (f32, f32), rings: u32, per_ring: u32) -> Vec<Vec3> {
    let mut out = Vec::with_capacity((rings as usize).saturating_mul(per_ring as usize));
    for r in 0..rings {
        let radius = match rings {
            1 => 0.5 * (inner + outer),
            _ => inner + (outer - inner) * r as f32 / (rings - 1) as f32,
        };
        for k in 0..per_ring {
            let angle = std::f32::consts::TAU * k as f32 / per_ring as f32;
            out.push(center + Vec3::new(angle.cos(), 0.0, angle.sin()) * radius);
        }
    }
    out
}

/// A square grid of `spacing` over `±half_size` around `center`, row by row
/// from -X -Z.
fn grid(center: Vec3, half_size: f32, spacing: f32) -> Vec<Vec3> {
    let side = grid_side(half_size, spacing);
    let mut out = Vec::with_capacity(side.saturating_mul(side));
    for z in 0..side {
        for x in 0..side {
            let offset = Vec3::new(x as f32, 0.0, z as f32) * spacing;
            out.push(center - Vec3::new(half_size, 0.0, half_size) + offset);
        }
    }
    out
}
