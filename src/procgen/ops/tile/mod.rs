//! The `tile` scatter op (#407): stamp the input once per cell of a whole-count grid,
//! each cell nudged, turned and scaled by values hashed from its wrapped index and
//! the recipe seed. Rivets are `shape circle` → `tile`. A linked jitter varies by
//! cell (#1494): an ordered grid that grows chaotic over a damage mask. #1742 adds
//! the rest of Substance Designer's *Tile Sampler* essentials: a seeded grey or an
//! index per stamp, a value jitter, a fill probability, a mask and a size map read
//! at each cell's centre, and the overlap blend.
//!
//! The input is read as **one stamp**: the unit square, nothing outside it (no
//! wrap), so a stamp pushed across its cell's border is drawn in the neighbour cell
//! too rather than cut off. Overlapping stamps keep the brighter value per channel
//! by default — the right blend for masks and heights.
//!
//! **Draw order.** `over` lets the later stamp cover: later is row-major order of
//! the stamps' wrapped cells (row 0 first, left to right), a property of the two
//! stamps alone. So neither the band split nor which pixel looks decides what
//! covers what, and a stamp crossing the tile edge covers the same stamps on both
//! sides of it.

mod stamp;

pub use stamp::{Jitter, Jitters};

use stamp::{centre, Base, Draws, Stamp, MAX_SIZE};

use super::super::catalog::vector::tile::{TileBlend, TileOutput};
use super::super::image_buf::{Image, Rgba};
use super::super::recipe::Tile;
use super::field::{Field, Links, PerCell};
use super::generators::count;

/// Everything a scatter reads besides its stamp image.
#[derive(Clone, Copy, Debug)]
pub struct Scatter<'a> {
    pub count: f32,
    pub jitters: Jitters<'a>,
    pub output: TileOutput,
    /// Each stamp's value is multiplied by `1 ± value_jitter / 2`; 0–2.
    pub value_jitter: f32,
    /// The chance a cell gets a stamp; 0–1.
    pub probability: f32,
    /// No stamp in a cell whose centre reads below 0.5 on the mask's red.
    pub mask: Option<&'a Image>,
    /// The red at the cell's centre scales that cell's stamp, up to 2.
    pub size_map: Option<&'a Image>,
    pub blend: TileBlend,
    pub seed: u64,
}

impl<'a> Scatter<'a> {
    /// The `tile` op's scatter: its params resolved against its `links`, its
    /// optional mask and size map, and its node seed. A NaN `probability` or
    /// `value_jitter` is ignored (the default); any other value is clamped.
    pub fn of(
        p: &Tile,
        links: &Links<'a>,
        mask: Option<&'a Image>,
        size_map: Option<&'a Image>,
        seed: u64,
    ) -> Self {
        let jitters = Jitters {
            offset: links.field(&p.jitter),
            rotation: links.field(&p.rotation_jitter),
            scale: links.field(&p.scale_jitter),
        };
        Self {
            value_jitter: finite_or(p.value_jitter, 0.0).clamp(0.0, 2.0),
            probability: finite_or(p.probability, 1.0).clamp(0.0, 1.0),
            output: p.output,
            blend: p.blend,
            mask,
            size_map,
            ..Self::plain(p.count, jitters, seed)
        }
    }

    /// A `cells`×`cells` scatter under `jitters`, every other setting at its default.
    pub fn plain(cells: f32, jitters: Jitters<'a>, seed: u64) -> Self {
        Self {
            count: cells,
            jitters,
            output: TileOutput::Stamp,
            value_jitter: 0.0,
            probability: 1.0,
            mask: None,
            size_map: None,
            blend: TileBlend::Max,
            seed,
        }
    }

    fn draws(&self) -> Draws {
        let base = match self.output {
            TileOutput::Stamp => Base::One,
            TileOutput::Random => Base::Random,
            TileOutput::Index => Base::Index,
        };
        Draws {
            base,
            value_jitter: self.value_jitter,
            probability: self.probability,
        }
    }
}

/// `v`, or `default` when it is NaN (an infinity is left for the clamp).
fn finite_or(v: f32, default: f32) -> f32 {
    if v.is_nan() {
        default
    } else {
        v
    }
}

/// A mask or size map as a field of its red, or `fallback` everywhere without one.
fn red(img: Option<&Image>, fallback: f32) -> Field<'_> {
    img.map_or(Field::Const(fallback), |img| Field::Map {
        img,
        min: 0.0,
        max: 1.0,
    })
}

/// Scatter `img` over `s`'s `count`×`count` grid (rounded to a whole count ≥ 1).
pub fn tile(img: &Image, s: &Scatter) -> Image {
    let n = count(s.count);
    let per = n as i64;
    let (mask, size) = (red(s.mask, 1.0), red(s.size_map, 1.0));
    let reach = s.jitters.reach(size.max().clamp(0.0, MAX_SIZE));
    let draws = s.draws();
    let j = s.jitters;
    let fields = [j.offset, j.rotation, j.scale, mask, size];
    let stamps = PerCell::new(per, &fields, |cell| {
        let (u, v) = centre(cell, n);
        if !(mask.at_uv(u, v) >= 0.5) {
            return None;
        }
        Stamp::of(cell, per, j.at(cell, n), size.at_uv(u, v), draws, s.seed)
    });
    let grey = draws.base != Base::One;
    Image::fill_uv(img.size(), |u, v| {
        let (px, py) = (u * n, v * n);
        let (gx, gy) = (px.floor() as i64, py.floor() as i64);
        let mut out: Rgba = [0.0, 0.0, 0.0, 1.0];
        let mut top: Option<(i64, i64)> = None;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (cx, cy) = (gx + dx, gy + dy);
                let Some(stamp) = stamps.get((cx.rem_euclid(per), cy.rem_euclid(per))) else {
                    continue;
                };
                let (su, sv) = stamp.local(px - cx as f32 - 0.5, py - cy as f32 - 0.5);
                if (0.0..1.0).contains(&su) && (0.0..1.0).contains(&sv) {
                    let p = img.sample_bilinear_wrapped(su, sv);
                    let colour = fill(p, stamp.value, grey);
                    blend(s.blend, &mut out, &mut top, p, colour, stamp.cell);
                }
            }
        }
        out
    })
}

/// A stamp's colour: the input times `value`, or its red times `value` as a grey;
/// alpha is the input's.
fn fill(p: Rgba, value: f32, grey: bool) -> Rgba {
    match grey {
        true => [p[0] * value, p[0] * value, p[0] * value, p[3]],
        false => [p[0] * value, p[1] * value, p[2] * value, p[3]],
    }
}

/// Lay `colour` (sampled as `p` from the stamp of wrapped cell `cell`) onto `out`.
/// `top` is the latest cell `over` has drawn here.
fn blend(
    mode: TileBlend,
    out: &mut Rgba,
    top: &mut Option<(i64, i64)>,
    p: Rgba,
    colour: Rgba,
    cell: (i64, i64),
) {
    match mode {
        TileBlend::Max => *out = std::array::from_fn(|c| out[c].max(colour[c])),
        TileBlend::Add => {
            *out = std::array::from_fn(|c| match c {
                3 => out[3].max(colour[3]),
                _ => out[c] + colour[c],
            })
        }
        TileBlend::Over => {
            // Row-major: compare (row, column).
            let order = (cell.1, cell.0);
            let covers = p[..3].iter().any(|&c| c > 0.0);
            if covers && top.is_none_or(|t| order > t) {
                *out = colour;
                *top = Some(order);
            }
        }
    }
}

#[cfg(test)]
#[path = "../tile_tests.rs"]
mod tests;
