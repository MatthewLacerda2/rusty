//! The `tile` scatter op (#407): stamp the input once per cell of a whole-count grid,
//! each cell nudged, turned and scaled by values hashed from its wrapped index and
//! the recipe seed. Rivets are `shape circle` → `tile`. A linked jitter varies by
//! cell (#1494): an ordered grid that grows chaotic over a damage mask.
//!
//! The input is read as **one stamp**: the unit square, nothing outside it (no
//! wrap), so a stamp pushed across its cell's border is drawn in the neighbour cell
//! too rather than cut off. Overlapping stamps keep the brighter value per channel —
//! the right blend for masks and heights.

use super::super::hash;
use super::super::image_buf::{Image, Rgba};
use super::super::recipe::Tile;
use super::field::{Field, Links, PerCell};
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
    /// The `tile` op's three jitter params, resolved against its `links`.
    pub fn of(p: &Tile, links: &Links<'a>) -> Self {
        Self {
            offset: links.field(&p.jitter),
            rotation: links.field(&p.rotation_jitter),
            scale: links.field(&p.scale_jitter),
        }
    }

    /// The same jitter everywhere.
    pub fn uniform(offset: f32, rotation: f32, scale: f32) -> Self {
        Self {
            offset: Field::Const(offset),
            rotation: Field::Const(rotation),
            scale: Field::Const(scale),
        }
    }

    /// The jitter of the (wrapped) cell `cell` of an `n`-cell grid.
    fn at(&self, cell: (i64, i64), n: f32) -> Jitter {
        let (u, v) = ((cell.0 as f32 + 0.5) / n, (cell.1 as f32 + 0.5) / n);
        Jitter::new(
            self.offset.at_uv(u, v),
            self.rotation.at_uv(u, v),
            self.scale.at_uv(u, v),
        )
    }

    /// The reach of the widest jitter any cell takes.
    fn reach(&self) -> i64 {
        Jitter::new(self.offset.max(), self.rotation.max(), self.scale.max()).reach()
    }
}

/// One cell's stamp transform: centre offset (cells), `(cos, sin)` of its turn, and
/// `1 / scale`.
#[derive(Clone, Copy)]
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
pub fn tile(img: &Image, cells: f32, jitter: Jitters, seed: u64) -> Image {
    let n = count(cells);
    let per = n as i64;
    let reach = jitter.reach();
    let fields = [jitter.offset, jitter.rotation, jitter.scale];
    let stamps = PerCell::new(per, &fields, |cell| {
        Stamp::of(cell, jitter.at(cell, n), seed)
    });
    Image::fill_uv(img.size(), |u, v| {
        let (px, py) = (u * n, v * n);
        let (gx, gy) = (px.floor() as i64, py.floor() as i64);
        let mut out: Rgba = [0.0, 0.0, 0.0, 1.0];
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (cx, cy) = (gx + dx, gy + dy);
                let cell = (cx.rem_euclid(per), cy.rem_euclid(per));
                let stamp = stamps.get(cell);
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
    fn each_cell_reads_its_own_jitter_from_a_link() {
        // A 4×4 grid over a 16×16 link: cell (cx, cy) is a 4×4 block of red
        // (cx + 4cy) / 16, so a read anywhere but that cell's centre is off.
        let link = Image::fill_uv(16, |u, v| {
            let (cx, cy) = ((u * 4.0).floor(), (v * 4.0).floor());
            [(cx + 4.0 * cy) / 16.0, 0.0, 0.0, 1.0]
        });
        let field = Field::Map {
            img: &link,
            min: 0.0,
            max: 1.0,
        };
        let jitters = Jitters {
            offset: field,
            ..Jitters::uniform(0.0, 0.0, 0.0)
        };
        for (cx, cy) in (0..4).flat_map(|y| (0..4).map(move |x| (x, y))) {
            let want = (cx + 4 * cy) as f32 / 16.0;
            assert_eq!(jitters.at((cx, cy), 4.0).offset, want, "cell ({cx}, {cy})");
        }
    }

    #[test]
    fn no_jitter_repeats_the_input_exactly() {
        let src = Image::fill_uv(8, |u, v| [u, v, 0.0, 1.0]);
        let out = tile(&src, 2.0, Jitters::uniform(0.0, 0.0, 0.0), 1);
        assert_eq!(out.size(), 8.into());
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
