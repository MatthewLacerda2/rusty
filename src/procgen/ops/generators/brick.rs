//! Running-bond brick (#392, #407): the face mask, a seeded value per brick, or a
//! bevel ramp. `rows` rounds to an **even** count so the half-brick offset closes at
//! the wrap, and brick indices wrap at the counts, so every output tiles.

use super::{count, gray, Sampler};
use crate::procgen::hash;
use crate::procgen::recipe::BrickOutput;

/// The hash channel per-brick values draw from (clear of Voronoi's 1–5).
const BRICK_CHANNEL: u64 = 16;

/// `cols`×`rows` bricks, every other row offset half a brick. `mortar` is the gap
/// as a fraction of one brick, on its left and bottom sides; mortar reads 0.
pub fn sampler(rows: f32, cols: f32, mortar: f32, output: BrickOutput, seed: u64) -> Sampler {
    let rows = count(rows / 2.0) * 2.0;
    let cols = count(cols);
    let mortar = mortar.clamp(0.0, 1.0);
    Box::new(move |u, v| {
        let ry = v * rows;
        let row = (ry.floor() as i64).rem_euclid(rows as i64);
        let offset = if row % 2 == 0 { 0.0 } else { 0.5 };
        let rx = u * cols + offset;
        let (cx, cy) = (rx.rem_euclid(1.0), ry.rem_euclid(1.0));
        if cx < mortar || cy < mortar {
            return gray(0.0);
        }
        gray(match output {
            BrickOutput::Mask => 1.0,
            BrickOutput::Random => {
                let col = (rx.floor() as i64).rem_euclid(cols as i64);
                hash::unit2(col, row, BRICK_CHANNEL, seed)
            }
            BrickOutput::Bevel => bevel(cx, cy, mortar, cols, rows),
        })
    })
}

/// Distance from `(cx, cy)` (cell units) to the face's nearest edge, in tile units,
/// over the farthest any face point gets: 0 at the mortar, 1 on the centre line.
fn bevel(cx: f32, cy: f32, mortar: f32, cols: f32, rows: f32) -> f32 {
    let face = 1.0 - mortar;
    let dx = (cx - mortar).min(1.0 - cx) / cols;
    let dy = (cy - mortar).min(1.0 - cy) / rows;
    let reach = (face / cols).min(face / rows) * 0.5;
    if reach > 0.0 {
        (dx.min(dy) / reach).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brick(output: BrickOutput, seed: u64) -> Sampler {
        sampler(2.0, 2.0, 0.1, output, seed)
    }

    #[test]
    fn random_is_one_value_per_brick_and_follows_the_seed() {
        let f = brick(BrickOutput::Random, 3);
        // Two points on row 0's first brick, one on its second.
        let (a, b, other) = (f(0.1, 0.1)[0], f(0.4, 0.4)[0], f(0.7, 0.1)[0]);
        assert_eq!(a, b, "one brick, one value");
        assert_ne!(a, other, "neighbouring bricks differ");
        assert_ne!(a, brick(BrickOutput::Random, 4)(0.1, 0.1)[0], "seeded");
        assert_eq!(f(0.02, 0.3)[0], 0.0, "mortar reads 0");
    }

    #[test]
    fn bevel_rises_from_the_mortar_to_the_centre_line() {
        let f = brick(BrickOutput::Bevel, 0);
        // Row 0's first brick spans u ∈ [0.05, 0.5), v ∈ [0.05, 0.5).
        let (edge, centre) = (f(0.06, 0.275)[0], f(0.275, 0.275)[0]);
        assert!(edge < 0.1, "{edge}");
        assert!((centre - 1.0).abs() < 1e-4, "{centre}");
        assert_eq!(f(0.02, 0.3)[0], 0.0, "mortar reads 0");
    }
}
