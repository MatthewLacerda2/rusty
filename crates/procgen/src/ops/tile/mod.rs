mod over;
mod stamp;

pub(super) use over::over;

pub use stamp::{Jitter, Jitters};

use over::Hits;
use stamp::{stretch, Draws, Grid, Stamp, MAX_SIZE};

use super::super::catalog::vector::tile::{TileBlend, TileOutput};
use super::super::image_buf::{Image, Rgba};
use super::super::recipe::Tile;
use super::field::{Field, Links, PerCell};
use super::generators::clamp_or;

#[derive(Clone, Copy, Debug)]
pub struct Settings<'a> {
    pub count: f32,
    pub aspect: [f32; 2],
    pub offset: f32,
    pub jitters: Jitters<'a>,
    pub output: TileOutput,
    pub value_jitter: f32,
    pub probability: f32,
    pub mask: Option<&'a Image>,
    pub size_map: Option<&'a Image>,
    pub blend: TileBlend,
    pub seed: u64,
}

impl<'a> Settings<'a> {
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
            value_jitter: clamp_or(p.value_jitter, 0.0, 0.0, 2.0),
            probability: clamp_or(p.probability, 1.0, 0.0, 1.0),
            aspect: p.aspect,
            offset: p.offset,
            output: p.output,
            blend: p.blend,
            mask,
            size_map,
            ..Self::plain(p.count, jitters, seed)
        }
    }

    pub fn plain(cells: f32, jitters: Jitters<'a>, seed: u64) -> Self {
        Self {
            count: cells,
            aspect: [1.0, 1.0],
            offset: 0.0,
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
        Draws {
            output: self.output,
            value_jitter: self.value_jitter,
            probability: self.probability,
        }
    }
}

fn red(img: Option<&Image>, fallback: f32) -> Field<'_> {
    img.map_or(Field::Const(fallback), |img| Field::Map {
        img,
        min: 0.0,
        max: 1.0,
    })
}

pub fn tile(img: &Image, s: &Settings) -> Image {
    let grid = Grid::new(s.count, s.aspect, s.offset);
    let ((nx, ny), per) = (grid.n, grid.per);
    let (mask, size) = (red(s.mask, 1.0), red(s.size_map, 1.0));
    let reach = s
        .jitters
        .reach(size.max().clamp(0.0, MAX_SIZE), stretch(per));
    let draws = s.draws();
    let stamps = PerCell::for_pixels(per, img.size().pixels(), |cell| {
        cell_stamp(s, &grid, (mask, size), draws, cell)
    });
    let grey = draws.output != TileOutput::Stamp;
    Image::fill_uv(img.size(), |u, v| {
        let (px, py) = (u * nx, v * ny);
        let gy = py.floor() as i64;
        let mut out: Rgba = [0.0, 0.0, 0.0, 1.0];
        let mut hits = (s.blend == TileBlend::Over).then(Hits::default);
        for dy in -reach.1..=reach.1 {
            let cy = gy + dy;
            let shift = grid.shift(cy);
            let gx = (px - shift).floor() as i64;
            for dx in -reach.0..=reach.0 {
                let cx = gx + dx;
                let wrapped = (cx.rem_euclid(per.0), cy.rem_euclid(per.1));
                let Some(stamp) = stamps.get(wrapped) else {
                    continue;
                };
                let (su, sv) = stamp.local(px - cx as f32 - 0.5 - shift, py - cy as f32 - 0.5);
                if (0.0..1.0).contains(&su) && (0.0..1.0).contains(&sv) {
                    let p = img.sample_bilinear_wrapped(su, sv);
                    let colour = fill(p, stamp.value, grey);
                    if let Some(hits) = &mut hits {
                        let order = [wrapped.1, wrapped.0, cy, cx];
                        hits.push(order, colour, coverage(p, grey));
                    } else {
                        fold(s.blend, &mut out, colour);
                    }
                }
            }
        }
        if let Some(hits) = &mut hits {
            hits.composite(&mut out);
        }
        out
    })
}

fn cell_stamp(
    s: &Settings,
    grid: &Grid,
    (mask, size): (Field, Field),
    draws: Draws,
    cell: (i64, i64),
) -> Option<Stamp> {
    let (u, v) = grid.centre(cell);
    let m = mask.at_uv(u, v);
    if m.is_nan() || m < 0.5 {
        return None;
    }
    let j = s.jitters.at(cell, grid);
    Stamp::of(cell, grid.per, j, size.at_uv(u, v), draws, s.seed)
}

#[inline]
pub(super) fn fold(mode: TileBlend, out: &mut Rgba, colour: Rgba) {
    *out = match mode {
        TileBlend::Add => std::array::from_fn(|c| match c {
            3 => out[3].max(colour[3]),
            _ => out[c] + colour[c],
        }),
        _ => std::array::from_fn(|c| out[c].max(colour[c])),
    };
}

pub(super) fn fill(p: Rgba, value: f32, grey: bool) -> Rgba {
    match grey {
        true => [p[0] * value, p[0] * value, p[0] * value, p[3]],
        false => [p[0] * value, p[1] * value, p[2] * value, p[3]],
    }
}

pub(super) fn coverage(p: Rgba, grey: bool) -> f32 {
    let bright = match grey {
        true => p[0],
        false => p[0].max(p[1]).max(p[2]),
    };
    (bright * p[3]).clamp(0.0, 1.0)
}

#[cfg(test)]
#[path = "../tile_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "grid_tests.rs"]
mod grid_tests;

#[cfg(test)]
#[path = "pinned_tests.rs"]
mod pinned_tests;
