//! One cell's stamp (#407, #1742): the jitter ranges, and what a cell draws from
//! its hash — where its stamp sits, how it is turned and sized, the value it is
//! filled with, and whether it is drawn at all.

use super::super::super::catalog::vector::tile::TileOutput;
use super::super::super::hash;
use super::super::field::Field;

/// The hash channels a cell's offset x/y, rotation and scale draw from (clear of
/// Voronoi's 1–5 and brick's 16).
const CHANNEL: u64 = 20;
/// The channels #1742's draws take, past the four placement ones, so adding
/// `probability` or `value_jitter` to a recipe never moves where a stamp lands.
const GREY: u64 = CHANNEL + 4;
const VALUE: u64 = CHANNEL + 5;
const CHANCE: u64 = CHANNEL + 6;

/// The largest a size map scales a stamp: past it a stamp would cover cells far
/// enough away to multiply the neighbours each pixel looks at.
pub(super) const MAX_SIZE: f32 = 2.0;

/// The widest [`Jitter::reach`] any clamped jitter takes at [`MAX_SIZE`].
pub(super) const MAX_REACH: i64 = 3;

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
    /// point from its cell centre, against a pixel's ≤ ½ cell from its own, with a
    /// size map scaling it by up to `size`.
    pub(super) fn reach(self, size: f32) -> i64 {
        let corner = if self.rotation > 0.0 {
            std::f32::consts::FRAC_1_SQRT_2
        } else {
            0.5
        };
        let extent = 0.5 * self.offset + corner * (1.0 + 0.5 * self.scale) * size;
        ((extent + 0.5).ceil() as i64 - 1).max(0)
    }
}

/// The three jitter params as [`Field`]s (#1494). A linked one is read once per
/// cell, at the cell's centre, wrapped: each cell keeps one stamp, and a cell met
/// across the wrap is the same cell, so the result still tiles.
#[derive(Clone, Copy, Debug)]
pub struct Jitters<'a> {
    pub offset: Field<'a>,
    pub rotation: Field<'a>,
    pub scale: Field<'a>,
}

impl<'a> Jitters<'a> {
    /// The same jitter everywhere.
    pub fn uniform(offset: f32, rotation: f32, scale: f32) -> Self {
        Self {
            offset: Field::Const(offset),
            rotation: Field::Const(rotation),
            scale: Field::Const(scale),
        }
    }

    /// The jitter of the (wrapped) cell `cell` of an `n`-cell grid.
    pub(super) fn at(&self, cell: (i64, i64), n: f32) -> Jitter {
        let (u, v) = centre(cell, n);
        Jitter::new(
            self.offset.at_uv(u, v),
            self.rotation.at_uv(u, v),
            self.scale.at_uv(u, v),
        )
    }

    /// The reach of the widest jitter any cell takes, at a size of up to `size`.
    pub(super) fn reach(&self, size: f32) -> i64 {
        Jitter::new(self.offset.max(), self.rotation.max(), self.scale.max()).reach(size)
    }
}

/// One cell's stamp: centre offset (cells), `(cos, sin)` of its turn, `1 / scale`,
/// and the value its colour is multiplied by.
#[derive(Clone, Copy, Debug)]
pub(super) struct Stamp {
    offset: [f32; 2],
    turn: (f32, f32),
    inv_scale: f32,
    pub value: f32,
}

/// The per-cell draws besides placement: what a stamp is filled with (its base
/// value: 1, a grey or an index), the `value_jitter` range, and the fill probability.
#[derive(Clone, Copy, Debug)]
pub(super) struct Draws {
    pub output: TileOutput,
    pub value_jitter: f32,
    pub probability: f32,
}

impl Stamp {
    /// Cell `cell`'s stamp under jitter `j` at size `size` (the size map's value,
    /// 1 without one), or `None` when the cell's draw or its size leaves it empty.
    pub fn of(
        cell: (i64, i64),
        per: i64,
        j: Jitter,
        size: f32,
        d: Draws,
        seed: u64,
    ) -> Option<Self> {
        let unit = |ch| hash::unit2(cell.0, cell.1, ch, seed);
        if size.is_nan() || size <= 0.0 {
            return None;
        }
        // At the defaults neither draw can change the stamp (`unit2` stays below
        // 1), so neither is hashed: every plain scatter would pay for both.
        if d.probability < 1.0 && unit(CHANCE) >= d.probability {
            return None;
        }
        let size = size.min(MAX_SIZE);
        let r = |ch| unit(CHANNEL + ch) - 0.5;
        let angle = j.rotation * r(2) * std::f32::consts::TAU;
        let base = match d.output {
            TileOutput::Stamp => 1.0,
            TileOutput::Random => unit(GREY),
            TileOutput::Index => index(cell, per),
        };
        let value = match d.value_jitter > 0.0 {
            true => base * (1.0 + d.value_jitter * (unit(VALUE) - 0.5)),
            false => base,
        };
        Some(Self {
            offset: [j.offset * r(0), j.offset * r(1)],
            turn: (angle.cos(), angle.sin()),
            inv_scale: 1.0 / ((1.0 + j.scale * r(3)) * size),
            value,
        })
    }

    /// The stamp-space point for `(x, y)`, relative to this stamp's cell centre.
    pub fn local(&self, x: f32, y: f32) -> (f32, f32) {
        let (x, y) = (x - self.offset[0], y - self.offset[1]);
        let (c, s) = self.turn;
        let (rx, ry) = (x * c + y * s, -x * s + y * c);
        (rx * self.inv_scale + 0.5, ry * self.inv_scale + 0.5)
    }
}

/// The `(u, v)` centre of cell `cell` of an `n`-cell grid: where a linked param,
/// a mask and a size map are read.
pub(super) fn centre(cell: (i64, i64), n: f32) -> (f32, f32) {
    ((cell.0 as f32 + 0.5) / n, (cell.1 as f32 + 0.5) / n)
}

/// Cell `cell`'s row-major index in a `per`×`per` grid as `(i + 1) / per²`, so the
/// first cell isn't lost against the black between stamps. In `f64`, so a large
/// grid neither overflows nor collapses neighbouring indices.
fn index((x, y): (i64, i64), per: i64) -> f32 {
    let per = per as f64;
    ((y as f64 * per + x as f64 + 1.0) / (per * per)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reach_grows_with_the_jitter_and_the_size() {
        assert_eq!(Jitter::new(0.0, 0.0, 0.0).reach(1.0), 0);
        assert_eq!(Jitter::new(1.0, 0.0, 0.0).reach(1.0), 1);
        assert_eq!(Jitter::new(1.0, 1.0, 1.0).reach(1.0), 2);
        assert_eq!(Jitter::new(0.0, 0.0, 0.0).reach(2.0), 1);
        assert_eq!(Jitter::new(1.0, 1.0, 1.0).reach(MAX_SIZE), MAX_REACH);
    }

    #[test]
    fn the_index_counts_row_major_from_one() {
        assert_eq!(index((0, 0), 2), 0.25);
        assert_eq!(index((1, 0), 2), 0.5);
        assert_eq!(index((1, 1), 2), 1.0);
    }
}
