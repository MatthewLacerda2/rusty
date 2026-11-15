//! The `scatter` op's math (#2187): stamps placed freely over the torus, off any
//! grid. Each stamp's place and draws come from its index and the node seed, each
//! from its own hash stream (`place` for the place, the channels below for the
//! rest), so raising `amount` adds stamps without moving the others and a param
//! added later reshuffles nothing.
//!
//! A stamp input is read as `tile` reads its one: the unit square, nothing outside
//! it. `density`, `scale` and `rotation` are read once per stamp at its centre,
//! wrapped, so the map tiles and looks the same at any resolution.
//!
//! **Cost.** The drawn stamps are filed into buckets (`buckets`) about half as
//! wide as the widest stamp, so a pixel visits only the stamps that can cover it;
//! the rows are split across threads by [`Image::fill_uv`]'s bands.

mod buckets;
mod place;

use std::f32::consts::TAU;

use buckets::Buckets;

use super::super::catalog::vector::scatter::AMOUNT;
use super::super::catalog::vector::tile::{TileBlend, TileOutput};
use super::super::hash;
use super::super::image_buf::{Image, Rgba};
use super::super::recipe::Scatter as Params;
use super::field::{Field, Links};
use super::generators::{clamp_or, finite};
use super::tile::{coverage, fill, fold, over};

/// The most stamps a tile holds: a pebble a few texels wide over a 2048² map.
const MAX_STAMPS: f32 = 65_536.0;

/// The largest a stamp's size reads (one tile across) before its jitter.
const MAX_SCALE: f32 = 1.0;

/// The hash channels of a stamp's draws past its place (`place` takes 40–41).
const DRAWS: u64 = 42;
const PICK: u64 = DRAWS;
const SIZE: u64 = DRAWS + 1;
const TURN: u64 = DRAWS + 2;
const VALUE: u64 = DRAWS + 3;
const GREY: u64 = DRAWS + 4;
const CHANCE: u64 = DRAWS + 5;

/// Everything a scatter reads besides its stamp images, resolved and clamped.
#[derive(Clone, Copy, Debug)]
pub struct Settings<'a> {
    /// Stamps per tile, a whole count in `0..=65 536`.
    pub amount: u32,
    /// A stamp's size in tiles, read at its centre (up to 1).
    pub scale: Field<'a>,
    /// Each stamp's size times `1 ± scale_jitter / 2`; 0–1.
    pub scale_jitter: f32,
    /// A stamp's turn in turns, read at its centre.
    pub rotation: Field<'a>,
    /// Each stamp turned up to `± rotation_jitter / 2` turns more; 0–1.
    pub rotation_jitter: f32,
    /// Each stamp's value times `1 ± value_jitter / 2`; 0–2.
    pub value_jitter: f32,
    /// 0 places stamps purely at random; toward 1 they keep apart (`place::gap`).
    pub spacing: f32,
    /// The chance a stamp is drawn, read at its centre.
    pub density: Field<'a>,
    pub output: TileOutput,
    pub blend: TileBlend,
    pub seed: u64,
}

impl<'a> Settings<'a> {
    /// The op's params resolved against its `links`, under its node seed. A NaN
    /// number reads its default; one out of range is clamped.
    pub fn of(p: &Params, links: &Links<'a>, seed: u64) -> Self {
        Self {
            amount: clamp_or(p.amount, AMOUNT, 0.0, MAX_STAMPS).round() as u32,
            scale: links.field(&p.scale),
            scale_jitter: clamp_or(p.scale_jitter, 0.0, 0.0, 1.0),
            rotation: links.field(&p.rotation),
            rotation_jitter: clamp_or(p.rotation_jitter, 0.0, 0.0, 1.0),
            value_jitter: clamp_or(p.value_jitter, 0.0, 0.0, 2.0),
            spacing: clamp_or(p.spacing, 0.0, 0.0, 1.0),
            density: links.field(&p.density),
            output: p.output,
            blend: p.blend,
            seed,
        }
    }
}

/// One drawn stamp: its centre (tiles), `(cos, sin)` of its turn, `1 / size`,
/// which input it stamps and the value its colour is multiplied by.
#[derive(Clone, Copy, Debug)]
struct Stamp {
    centre: [f32; 2],
    turn: (f32, f32),
    inv_size: f32,
    pick: usize,
    value: f32,
}

impl Stamp {
    /// Stamp `i` at `centre`, or `None` when its density draw or its size leaves
    /// it out. A draw whose setting can't change the stamp (a jitter of 0, a
    /// density of 1, a pick from one input) is not hashed.
    fn of(i: u32, centre: [f32; 2], inputs: usize, s: &Settings) -> Option<Self> {
        let unit = |ch| hash::unit2(i64::from(i), 0, ch, s.seed);
        let [u, v] = centre;
        let density = s.density.at_uv(u, v);
        if density.is_nan() || (density < 1.0 && unit(CHANCE) >= density) {
            return None;
        }
        let scale = s.scale.at_uv(u, v);
        if scale.is_nan() || scale <= 0.0 {
            return None;
        }
        // A jitter of 0 multiplies its draw away, so the draw is not hashed.
        let jitter = |amount: f32, ch| match amount > 0.0 {
            true => amount * (unit(ch) - 0.5),
            false => 0.0,
        };
        let size = scale.min(MAX_SCALE) * (1.0 + jitter(s.scale_jitter, SIZE));
        let turn = finite(s.rotation.at_uv(u, v), 0.0) + jitter(s.rotation_jitter, TURN);
        let angle = turn * TAU;
        let pick = match inputs > 1 {
            true => ((unit(PICK) * inputs as f32) as usize).min(inputs - 1),
            false => 0,
        };
        let base = match s.output {
            TileOutput::Stamp => 1.0,
            TileOutput::Random => unit(GREY),
            TileOutput::Index => ((f64::from(i) + 1.0) / f64::from(s.amount)) as f32,
        };
        let value = base * (1.0 + jitter(s.value_jitter, VALUE));
        Some(Self {
            centre,
            turn: (angle.cos(), angle.sin()),
            inv_size: 1.0 / size,
            pick,
            value,
        })
    }

    /// How far the stamp's turned square reaches from its centre on either axis.
    fn half_width(&self) -> f32 {
        let (c, s) = self.turn;
        0.5 * (c.abs() + s.abs()) / self.inv_size
    }

    /// The stamp-space point for the tile point `(x, y)` (relative to the copy
    /// whose centre is this stamp's, already shifted).
    #[inline]
    fn local(&self, x: f32, y: f32) -> (f32, f32) {
        let (x, y) = (x - self.centre[0], y - self.centre[1]);
        let (c, s) = self.turn;
        let (rx, ry) = (x * c + y * s, -x * s + y * c);
        (rx * self.inv_size + 0.5, ry * self.inv_size + 0.5)
    }
}

/// The stamps `s` draws, in index order, over `inputs` stamp images.
fn stamps(s: &Settings, inputs: usize) -> Vec<Stamp> {
    let size = s.scale.max();
    let gap = place::gap(s.spacing, s.amount, if size.is_nan() { 0.0 } else { size });
    place::place(s.amount, gap, s.seed)
        .into_iter()
        .zip(0..)
        .filter_map(|(centre, i)| Stamp::of(i, centre?, inputs, s))
        .collect()
}

/// Scatter `inputs` (1–8 stamp images, all one size) under `s`.
pub fn scatter(inputs: &[&Image], s: &Settings) -> Image {
    draw(inputs, &stamps(s, inputs.len()), s)
}

/// Draw `stamps` (in index order) over the tile.
fn draw(inputs: &[&Image], stamps: &[Stamp], s: &Settings) -> Image {
    let boxes: Vec<_> = stamps
        .iter()
        .map(|st| (st.centre, st.half_width()))
        .collect();
    let buckets = Buckets::new(&boxes);
    let grey = s.output != TileOutput::Stamp;
    Image::fill_uv(inputs[0].size(), |u, v| {
        let mut out: Rgba = [0.0, 0.0, 0.0, 1.0];
        for e in buckets.at(u, v) {
            let st = &stamps[e.stamp as usize];
            let (x, y) = (u + f32::from(e.shift[0]), v + f32::from(e.shift[1]));
            let (su, sv) = st.local(x, y);
            if (0.0..1.0).contains(&su) && (0.0..1.0).contains(&sv) {
                let p = inputs[st.pick].sample_bilinear_wrapped(su, sv);
                let colour = fill(p, st.value, grey);
                match s.blend {
                    TileBlend::Over => over(&mut out, colour, coverage(p, grey)),
                    _ => fold(s.blend, &mut out, colour),
                }
            }
        }
        out
    })
}

#[cfg(test)]
mod tests;
