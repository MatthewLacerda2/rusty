//! Voronoi / Worley cellular noise: F1, F2, their difference (cell edges) and flat
//! per-cell colour (#406). The cell lattice wraps at a whole count, so it tiles.

use super::{count, gray, Sampler};
use crate::procgen::hash;
use crate::procgen::recipe::VoronoiOutput;

/// The `scale`-cells-per-tile Voronoi sampler. Each cell's feature point sits at the
/// cell centre pushed by `randomness` (clamped to `[0, 1]`) of a full-cell jitter, so
/// 0 is a regular grid and 1 is anywhere in the cell.
pub fn sampler(scale: f32, output: VoronoiOutput, randomness: f32, seed: u64) -> Sampler {
    let cells = count(scale);
    let per = cells as i64;
    let jitter = randomness.clamp(0.0, 1.0);
    // F1 lives in the 3×3 neighbourhood; the second-nearest point can sit one ring out.
    let reach = match output {
        VoronoiOutput::Distance | VoronoiOutput::Cells => 1,
        VoronoiOutput::F2 | VoronoiOutput::Edges => 2,
    };
    Box::new(move |u, v| {
        let (px, py) = (u * cells, v * cells);
        let (gx, gy) = (px.floor() as i64, py.floor() as i64);
        let (mut f1, mut f2, mut best_cell) = (f32::MAX, f32::MAX, (0, 0));
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (cx, cy) = (gx + dx, gy + dy);
                let cell = (cx.rem_euclid(per), cy.rem_euclid(per));
                let at = |ch| 0.5 + jitter * (hash::unit2(cell.0, cell.1, ch, seed) - 0.5);
                let dist = (cx as f32 + at(1) - px).hypot(cy as f32 + at(2) - py);
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
