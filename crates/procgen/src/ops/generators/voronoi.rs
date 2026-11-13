//! Voronoi / Worley cellular noise: F1, F2, their difference (cell edges) and flat
//! per-cell colour (#406), and Blender's smooth F1 (#1740). The cell lattice wraps
//! at a whole count, so it tiles.

use super::{count, gray, Sampler};
use crate::hash;
use crate::ops::field::{Field, PerCell};
use crate::recipe::{VoronoiMetric, VoronoiOutput};

/// The Voronoi sampler, `scale` cells per tile across and down (each a whole count). Each cell's feature point sits at the
/// cell centre pushed by `randomness` (clamped to `[0, 1]`) of a full-cell jitter, so
/// 0 is a regular grid and 1 is anywhere in the cell; `metric` measures distance.
/// A linked `randomness` is read at each cell's centre, wrapped, so the cell keeps
/// one point and the pattern still tiles (#1494). `smoothness` (clamped to
/// `[0, 1]`, NaN reading 0) is `SmoothF1`'s.
pub fn sampler(
    scale: [f32; 2],
    (output, metric): (VoronoiOutput, VoronoiMetric),
    randomness: Field<'_>,
    smoothness: f32,
    seed: u64,
) -> Sampler<'_> {
    let smooth = SmoothMin::new(smoothness);
    let cells = scale.map(count);
    let per = cells.map(|c| c as i64);
    let reach = reach(output, metric);
    let centre = move |c: i64, axis: usize| (c as f32 + 0.5) / cells[axis];
    let jitters = PerCell::grid((per[0], per[1]), &[randomness], move |cell: (i64, i64)| {
        randomness
            .at_uv(centre(cell.0, 0), centre(cell.1, 1))
            .clamp(0.0, 1.0)
    });
    Box::new(move |u, v| {
        let (px, py) = (u * cells[0], v * cells[1]);
        let (gx, gy) = (px.floor() as i64, py.floor() as i64);
        let (mut f1, mut f2, mut best_cell) = (f32::MAX, f32::MAX, (0, 0));
        let mut soft = None;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (cx, cy) = (gx + dx, gy + dy);
                let cell = (cx.rem_euclid(per[0]), cy.rem_euclid(per[1]));
                let jitter = jitters.get(cell);
                let at = |ch| 0.5 + jitter * (hash::unit2(cell.0, cell.1, ch, seed) - 0.5);
                let dist = measure(metric, cx as f32 + at(1) - px, cy as f32 + at(2) - py);
                if output == VoronoiOutput::SmoothF1 {
                    soft = Some(soft.map_or(dist, |s| smooth.fold(s, dist)));
                }
                if dist < f1 {
                    (f2, f1, best_cell) = (f1, dist, cell);
                } else if dist < f2 {
                    f2 = dist;
                }
            }
        }
        match output {
            VoronoiOutput::Distance => gray(f1.min(1.0)),
            VoronoiOutput::F2 => gray(f2.min(1.0)),
            VoronoiOutput::Edges => gray((f2 - f1).min(1.0)),
            VoronoiOutput::SmoothF1 => gray(soft.unwrap_or(f1).clamp(0.0, 1.0)),
            VoronoiOutput::Cells => {
                let c = |ch| hash::unit2(best_cell.0, best_cell.1, ch, seed);
                [c(3), c(4), c(5), 1.0]
            }
        }
    })
}

/// How many cell rings around the sample's own cell hold the point `output` needs.
/// A point `R + 1` rings out is more than `R` cells away along one axis, so more
/// than `R` under every metric; a ring reaches far enough once that bounds the
/// farthest the point can be. `Distance` and `F2` clamp to 1, so one ring is exact
/// for both: any point within 1 is in it. Under Chebyshev one ring is exact for
/// every output: a point `k ≥ 2` cells out along x is more than `k − 1 ≥ 1` away,
/// and the own cell's point and the neighbour's one cell out along x are nearer
/// (less than 1 across, less than `k − 1` along), so F1 and F2 both lie in ring 1.
/// Otherwise `Cells` needs the true nearest point, at most the own cell's: `√2`
/// (Euclidean) or `2` (Manhattan) away; `Edges` the true second-nearest, at most
/// the farther of the own cell's point and the nearer neighbour's on one axis
/// (`≤ 1.5` across, `≤ 1` along): `√3.25 ≈ 1.8` (Euclidean) or `2.5` (Manhattan).
fn reach(output: VoronoiOutput, metric: VoronoiMetric) -> i64 {
    match (output, metric) {
        // Blender's reach: points two rings out still pull the smooth minimum.
        (VoronoiOutput::SmoothF1, _) => 2,
        (VoronoiOutput::Distance | VoronoiOutput::F2, _) | (_, VoronoiMetric::Chebyshev) => 1,
        (VoronoiOutput::Edges, VoronoiMetric::Manhattan) => 3,
        (VoronoiOutput::Cells | VoronoiOutput::Edges, _) => 2,
    }
}

/// Blender's smooth minimum for Smooth F1: each nearer point blends the running
/// distance toward it over a window of `smoothness / 2`, minus a correction that
/// rounds the crease. At 0 it is the hard minimum.
#[derive(Clone, Copy)]
struct SmoothMin {
    k: f32,
}

impl SmoothMin {
    fn new(smoothness: f32) -> Self {
        let s = super::finite(smoothness, 0.0).clamp(0.0, 1.0);
        Self {
            k: (s * 0.5).max(f32::MIN_POSITIVE),
        }
    }

    fn fold(self, running: f32, dist: f32) -> f32 {
        let t = (0.5 + 0.5 * (running - dist) / self.k).clamp(0.0, 1.0);
        let h = t * t * (3.0 - 2.0 * t);
        running + (dist - running) * h - self.k * h * (1.0 - h)
    }
}

/// The length of `(dx, dy)` under `metric`.
fn measure(metric: VoronoiMetric, dx: f32, dy: f32) -> f32 {
    match metric {
        VoronoiMetric::Euclidean => dx.hypot(dy),
        VoronoiMetric::Manhattan => dx.abs() + dy.abs(),
        VoronoiMetric::Chebyshev => dx.abs().max(dy.abs()),
    }
}

#[cfg(test)]
#[path = "voronoi_tests.rs"]
mod tests;
