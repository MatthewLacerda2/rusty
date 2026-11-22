use super::super::super::catalog::vector::tile::TileOutput;
use super::super::super::hash;
use super::super::field::Field;
use super::super::generators::count;

const CHANNEL: u64 = 20;
// Past the four placement channels, so adding `probability` or `value_jitter` never moves
// where a stamp lands.
const GREY: u64 = CHANNEL + 4;
const VALUE: u64 = CHANNEL + 5;
const CHANCE: u64 = CHANNEL + 6;

/// Past it a stamp would reach cells far enough away to multiply the neighbours a pixel visits.
pub(super) const MAX_SIZE: f32 = 2.0;

pub(super) const MAX_REACH: i64 = 3;

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

    pub(super) fn reach(self, size: f32, stretch: f32) -> (i64, i64) {
        let k = 0.5 * (1.0 + 0.5 * self.scale) * size;
        let (a, b) = (k, k * stretch);
        let most = self.rotation * std::f32::consts::PI;
        // A turned bound is taken a hair over, so a libm's last bit cannot cut it.
        let along = |a: f32, b: f32| {
            if most <= 0.0 {
                a
            } else if most >= b.atan2(a) {
                a.hypot(b) * (1.0 + 1e-4)
            } else {
                (a * most.cos() + b * most.sin()) * (1.0 + 1e-4)
            }
        };
        let cells = |extent: f32| {
            let extent = 0.5 * self.offset + extent;
            ((extent + 0.5).ceil() as i64 - 1).max(0)
        };
        (cells(along(a, b)), cells(along(b, a) / stretch))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Jitters<'a> {
    pub offset: Field<'a>,
    pub rotation: Field<'a>,
    pub scale: Field<'a>,
}

impl<'a> Jitters<'a> {
    /// A test hook: the crate's tile tests build plain jitters with it.
    pub fn uniform(offset: f32, rotation: f32, scale: f32) -> Self {
        Self {
            offset: Field::Const(offset),
            rotation: Field::Const(rotation),
            scale: Field::Const(scale),
        }
    }

    pub(super) fn at(&self, cell: (i64, i64), grid: &Grid) -> Jitter {
        let (u, v) = grid.centre(cell);
        Jitter::new(
            self.offset.at_uv(u, v),
            self.rotation.at_uv(u, v),
            self.scale.at_uv(u, v),
        )
    }

    pub(super) fn reach(&self, size: f32, stretch: f32) -> (i64, i64) {
        Jitter::new(self.offset.max(), self.rotation.max(), self.scale.max()).reach(size, stretch)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Stamp {
    offset: [f32; 2],
    turn: (f32, f32),
    inv_scale: f32,
    stretch: f32,
    pub value: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Draws {
    pub output: TileOutput,
    pub value_jitter: f32,
    pub probability: f32,
}

impl Stamp {
    pub fn of(
        cell: (i64, i64),
        per: (i64, i64),
        j: Jitter,
        size: f32,
        d: Draws,
        seed: u64,
    ) -> Option<Self> {
        let unit = |ch| hash::unit2(cell.0, cell.1, ch, seed);
        if size.is_nan() || size <= 0.0 {
            return None;
        }
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
            stretch: stretch(per),
            value,
        })
    }

    pub fn local(&self, x: f32, y: f32) -> (f32, f32) {
        let (x, y) = (x - self.offset[0], (y - self.offset[1]) * self.stretch);
        let (c, s) = self.turn;
        let (rx, ry) = (x * c + y * s, (-x * s + y * c) / self.stretch);
        (rx * self.inv_scale + 0.5, ry * self.inv_scale + 0.5)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Grid {
    pub n: (f32, f32),
    pub per: (i64, i64),
    pub shift: f32,
}

impl Grid {
    pub fn new(cells: f32, aspect: [f32; 2], offset: f32) -> Self {
        let shift = if offset.is_nan() {
            0.0
        } else {
            offset.clamp(0.0, 1.0)
        };
        let across = count(cells * aspect[0]);
        let down = match shift > 0.0 {
            true => count(cells * aspect[1] / 2.0) * 2.0,
            false => count(cells * aspect[1]),
        };
        Self {
            n: (across, down),
            per: (across as i64, down as i64),
            shift,
        }
    }

    #[inline]
    pub fn shift(&self, row: i64) -> f32 {
        match row.rem_euclid(2) {
            1 => self.shift,
            _ => 0.0,
        }
    }

    pub fn centre(&self, cell: (i64, i64)) -> (f32, f32) {
        let x = cell.0 as f32 + 0.5 + self.shift(cell.1);
        (x / self.n.0, (cell.1 as f32 + 0.5) / self.n.1)
    }
}

pub(super) fn stretch(per: (i64, i64)) -> f32 {
    per.0 as f32 / per.1 as f32
}

/// In `f64`, so a large grid neither overflows nor collapses neighbouring indices.
fn index((x, y): (i64, i64), per: (i64, i64)) -> f32 {
    let (w, h) = (per.0 as f64, per.1 as f64);
    ((y as f64 * w + x as f64 + 1.0) / (w * h)) as f32
}

#[cfg(test)]
#[path = "stamp_tests.rs"]
mod tests;
