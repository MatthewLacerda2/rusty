//! Voronoi / Worley cellular noise: F1, F2, their difference (cell edges) and flat
//! per-cell colour (#406). The cell lattice wraps at a whole count, so it tiles.

use super::{count, gray, Sampler};
use crate::procgen::hash;
use crate::procgen::recipe::{VoronoiMetric, VoronoiOutput};

/// The `scale`-cells-per-tile Voronoi sampler. Each cell's feature point sits at the
/// cell centre pushed by `randomness` (clamped to `[0, 1]`) of a full-cell jitter, so
/// 0 is a regular grid and 1 is anywhere in the cell; `metric` measures distance.
pub fn sampler(
    scale: f32,
    output: VoronoiOutput,
    metric: VoronoiMetric,
    randomness: f32,
    seed: u64,
) -> Sampler {
    let cells = count(scale);
    let per = cells as i64;
    let jitter = randomness.clamp(0.0, 1.0);
    let reach = reach(output, metric);
    Box::new(move |u, v| {
        let (px, py) = (u * cells, v * cells);
        let (gx, gy) = (px.floor() as i64, py.floor() as i64);
        let (mut f1, mut f2, mut best_cell) = (f32::MAX, f32::MAX, (0, 0));
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (cx, cy) = (gx + dx, gy + dy);
                let cell = (cx.rem_euclid(per), cy.rem_euclid(per));
                let at = |ch| 0.5 + jitter * (hash::unit2(cell.0, cell.1, ch, seed) - 0.5);
                let dist = measure(metric, cx as f32 + at(1) - px, cy as f32 + at(2) - py);
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
/// farthest the point can be. F1 is at most the own cell's point: `1` (Chebyshev)
/// or `2` (Manhattan) away. F2 is at most the farther of the own cell's point and
/// the nearer neighbour's on one axis (`≤ 1.5` across, `≤ 1` along): `1.5`
/// (Chebyshev), `√3.25 ≈ 1.8` (Euclidean) or `2.5` (Manhattan) away. Euclidean F1
/// keeps the 3×3 search it always had, though its bound is `√2`: only when all nine
/// nearby points are pushed away at full jitter can one two rings out be nearer.
fn reach(output: VoronoiOutput, metric: VoronoiMetric) -> i64 {
    let second = matches!(output, VoronoiOutput::F2 | VoronoiOutput::Edges);
    match (metric, second) {
        (VoronoiMetric::Euclidean | VoronoiMetric::Chebyshev, false) => 1,
        (VoronoiMetric::Euclidean | VoronoiMetric::Chebyshev, true) => 2,
        (VoronoiMetric::Manhattan, false) => 2,
        (VoronoiMetric::Manhattan, true) => 3,
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
