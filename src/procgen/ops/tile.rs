//! The `tile` scatter op (#407): stamp the input once per cell of a whole-count grid,
//! each cell nudged, turned and scaled by values hashed from its wrapped index and
//! the recipe seed. Rivets are `shape circle` → `tile`.
//!
//! The input is read as **one stamp**: the unit square, nothing outside it (no
//! wrap), so a stamp pushed across its cell's border is drawn in the neighbour cell
//! too rather than cut off. Overlapping stamps keep the brighter value per channel —
//! the right blend for masks and heights.

use super::super::hash;
use super::super::image_buf::{Image, Rgba};
use super::generators::count;

/// The hash channels a cell's offset x/y, rotation and scale draw from (clear of
/// Voronoi's 1–5 and brick's 16).
const CHANNEL: u64 = 20;

/// The per-cell randomness ranges, clamped: `jitter` moves a stamp up to half a cell
/// either way at 1, `rotation` turns it up to half a turn either way at 1, `scale`
/// sizes it in `[1 − s/2, 1 + s/2]`.
#[derive(Clone, Copy, Debug)]
pub struct Jitter {
    pub offset: f32,
    pub rotation: f32,
    pub scale: f32,
}

impl Jitter {
    pub fn new(offset: f32, rotation: f32, scale: f32) -> Self {
        Self {
            offset: offset.clamp(0.0, 1.0),
            rotation: rotation.clamp(0.0, 1.0),
            scale: scale.clamp(0.0, 1.0),
        }
    }

    /// How many neighbour cells out a stamp can reach a pixel from: its farthest
    /// point from its cell centre, against a pixel's ≤ ½ cell from its own.
    fn reach(self) -> i64 {
        let corner = if self.rotation > 0.0 {
            std::f32::consts::FRAC_1_SQRT_2
        } else {
            0.5
        };
        let extent = 0.5 * self.offset + corner * (1.0 + 0.5 * self.scale);
        ((extent + 0.5).ceil() as i64 - 1).max(0)
    }
}

/// One cell's stamp transform: centre offset (cells), `(cos, sin)` of its turn, and
/// `1 / scale`.
struct Stamp {
    offset: [f32; 2],
    turn: (f32, f32),
    inv_scale: f32,
}

impl Stamp {
    fn of(cell: (i64, i64), j: Jitter, seed: u64) -> Self {
        let r = |ch| hash::unit2(cell.0, cell.1, CHANNEL + ch, seed) - 0.5;
        let angle = j.rotation * r(2) * std::f32::consts::TAU;
        Self {
            offset: [j.offset * r(0), j.offset * r(1)],
            turn: (angle.cos(), angle.sin()),
            inv_scale: 1.0 / (1.0 + j.scale * r(3)),
        }
    }

    /// The stamp-space point for `(x, y)`, relative to this stamp's cell centre.
    fn local(&self, x: f32, y: f32) -> (f32, f32) {
        let (x, y) = (x - self.offset[0], y - self.offset[1]);
        let (c, s) = self.turn;
        let (rx, ry) = (x * c + y * s, -x * s + y * c);
        (rx * self.inv_scale + 0.5, ry * self.inv_scale + 0.5)
    }
}

/// Scatter `img` over a `count`×`count` grid (rounded to a whole count ≥ 1).
pub fn tile(img: &Image, cells: f32, jitter: Jitter, seed: u64) -> Image {
    let n = count(cells);
    let per = n as i64;
    let reach = jitter.reach();
    Image::fill_uv(img.resolution(), |u, v| {
        let (px, py) = (u * n, v * n);
        let (gx, gy) = (px.floor() as i64, py.floor() as i64);
        let mut out: Rgba = [0.0, 0.0, 0.0, 1.0];
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (cx, cy) = (gx + dx, gy + dy);
                let stamp = Stamp::of((cx.rem_euclid(per), cy.rem_euclid(per)), jitter, seed);
                let (su, sv) = stamp.local(px - cx as f32 - 0.5, py - cy as f32 - 0.5);
                if (0.0..1.0).contains(&su) && (0.0..1.0).contains(&sv) {
                    let p = img.sample_bilinear_wrapped(su, sv);
                    out = std::array::from_fn(|c| out[c].max(p[c]));
                }
            }
        }
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reach_grows_with_the_jitter() {
        assert_eq!(Jitter::new(0.0, 0.0, 0.0).reach(), 0);
        assert_eq!(Jitter::new(1.0, 0.0, 0.0).reach(), 1);
        assert_eq!(Jitter::new(1.0, 1.0, 1.0).reach(), 2);
    }

    #[test]
    fn no_jitter_repeats_the_input_exactly() {
        let src = Image::fill_uv(8, |u, v| [u, v, 0.0, 1.0]);
        let out = tile(&src, 2.0, Jitter::new(0.0, 0.0, 0.0), 1);
        assert_eq!(out.resolution(), 8);
        for y in 0..8 {
            for x in 0..8 {
                let expect = src.sample_bilinear_wrapped(
                    ((x % 4) as f32 + 0.5) / 4.0,
                    ((y % 4) as f32 + 0.5) / 4.0,
                );
                assert_eq!(out.get_wrapped(x, y), expect, "({x},{y})");
            }
        }
    }
}
