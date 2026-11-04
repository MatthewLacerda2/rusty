//! src/procgen/ops/flood.rs — a mask's islands (#1741): connected-component
//! labelling on the torus, and what each island knows of itself (its centroid and
//! its bounding box, in tile units), for the flood-fill ops.
//!
//! An island is a set of "on" pixels connected through their 8 neighbours, wrapping
//! both axes, so a piece crossing a tile edge or a corner is one island. The scan
//! is a serial flood from each unlabelled pixel in row-major order; each pixel it
//! reaches gets a *lifted* coordinate (its step from the flood's first pixel,
//! summed, never wrapped), so an island across an edge measures as one compact
//! piece. An island that loops round the tile on an axis (a band across it) has no
//! edge on that axis: its box spans the tile there, starting at 0, and its centre
//! reads 0.5.
//!
//! Nothing an island reports depends on the order the scan meets it: a box, a
//! centroid and an [`Island::random`] value are the same whichever pixel the flood
//! starts from, so shifting a mask by a wrap shifts every island's box and centroid
//! with it.

use crate::procgen::hash;
use crate::procgen::image_buf::{Image, Rgba, Size};

/// How finely an island's centroid is snapped before it is hashed to its random
/// values, in cells per tile. Coarse enough that the same island at another
/// resolution (its centroid moves by a fraction of a pixel) mostly keeps its cell,
/// fine enough that two islands share a cell only when both are specks a
/// 1/`IDENTITY_GRID`-tile apart.
const IDENTITY_GRID: f32 = 128.0;

/// One island: its centroid, and its bounding box's start and size, in tile units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Island {
    /// The mean of its pixel centres, wrapped into `[0, 1)`; 0.5 on an axis it spans.
    pub centroid: [f32; 2],
    /// The box's first edge, wrapped into `[0, 1)`; 0 on an axis it spans.
    pub start: [f32; 2],
    /// The box's extent, at most 1 (an island that spans the axis).
    pub size: [f32; 2],
}

impl Island {
    /// The box's centre, wrapped into `[0, 1)`.
    pub fn centre(&self) -> [f32; 2] {
        [0, 1].map(|a| (self.start[a] + self.size[a] / 2.0).rem_euclid(1.0))
    }

    /// A uniform `[0, 1)` value for this island on `channel`, from `seed` and the
    /// island's centroid snapped to [`IDENTITY_GRID`]: its identity, not the
    /// number the scan gave it.
    pub fn random(&self, channel: u64, seed: u64) -> f32 {
        let [x, y] = self.centroid.map(|c| (c * IDENTITY_GRID).floor() as i64);
        hash::unit2(x, y, channel, seed)
    }
}

/// A labelled mask: which island each pixel belongs to, and the islands.
pub struct Islands {
    size: Size,
    /// Per pixel, row-major: 0 off, `k + 1` for island `k`.
    labels: Vec<u32>,
    pub islands: Vec<Island>,
}

impl Islands {
    /// The island pixel `(x, y)` belongs to, if it is on.
    pub fn at(&self, x: usize, y: usize) -> Option<&Island> {
        let label = self.labels[y * self.size.width as usize + x];
        label.checked_sub(1).map(|k| &self.islands[k as usize])
    }
}

/// The islands of `img`'s pixels for which `on` holds, 8-connected and wrapping.
///
/// Serial, `O(pixels)`: at 2048² about half a second of one core, holding a
/// label per pixel (16 MB) and, for a mask that is all one island, a flood stack
/// as long as the island (up to 32 MB).
pub fn label(img: &Image, on: impl Fn([f32; 4]) -> bool) -> Islands {
    let size = img.size();
    let (w, h) = (size.width as i64, size.height as i64);
    let on: Vec<bool> = img.pixels().iter().map(|p| on(*p)).collect();
    let mut labels = vec![0u32; on.len()];
    let mut islands = Vec::new();
    let mut stack = Vec::new();
    for first in 0..on.len() {
        if !on[first] || labels[first] != 0 {
            continue;
        }
        islands.push(Island::default());
        let id = islands.len() as u32;
        let (x0, y0) = ((first as i64 % w) as i32, (first as i64 / w) as i32);
        let mut extent = Extent::new();
        labels[first] = id;
        stack.push((x0, y0));
        while let Some((lx, ly)) = stack.pop() {
            extent.add(lx, ly);
            for (dx, dy) in NEIGHBOURS {
                let (nx, ny) = (lx + dx, ly + dy);
                let (wx, wy) = (i64::from(nx).rem_euclid(w), i64::from(ny).rem_euclid(h));
                let i = (wy * w + wx) as usize;
                if on[i] && labels[i] == 0 {
                    labels[i] = id;
                    stack.push((nx, ny));
                }
            }
        }
        islands[id as usize - 1] = extent.island(size);
    }
    Islands {
        size,
        labels,
        islands,
    }
}

/// The islands a reader takes from its input: `flood_fill`'s output carries them
/// in its alpha (1 on an island, 0 off), and relabelling that alpha gives back the
/// same islands, since two of them never touch. Any other image reads by its
/// alpha the same way: an opaque one is one island, the whole tile.
pub fn of_label_image(img: &Image) -> Islands {
    label(img, |p| p[3] > 0.5)
}

/// A grey image of `f(island, u, v)` over every island's pixels (`u`, `v` the
/// pixel centre in tile units), 0 off them; alpha 1.
pub fn paint(islands: &Islands, f: impl Fn(&Island, f32, f32) -> f32 + Sync) -> Image {
    paint_rgba(islands, [0.0, 0.0, 0.0, 1.0], |i, u, v| {
        let g = f(i, u, v);
        [g, g, g, 1.0]
    })
}

/// [`paint`] in colour: `f(island, u, v)` on every island's pixels, `off` off them.
pub fn paint_rgba(
    islands: &Islands,
    off: Rgba,
    f: impl Fn(&Island, f32, f32) -> Rgba + Sync,
) -> Image {
    let size = islands.size;
    let (w, h) = (size.width as f32, size.height as f32);
    let mut out = Image::new(size);
    out.for_each_row(|y, row| {
        let v = (y as f32 + 0.5) / h;
        for (x, p) in row.iter_mut().enumerate() {
            *p = islands
                .at(x, y)
                .map_or(off, |i| f(i, (x as f32 + 0.5) / w, v));
        }
    });
    out
}

/// The 8 neighbours' steps.
const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

impl Default for Island {
    fn default() -> Self {
        Island {
            centroid: [0.5; 2],
            start: [0.0; 2],
            size: [1.0; 2],
        }
    }
}

/// An island's lifted pixels as the flood meets them: per axis, the least and
/// greatest coordinate and their sum.
struct Extent {
    min: [i64; 2],
    max: [i64; 2],
    sum: [i64; 2],
    count: i64,
}

impl Extent {
    fn new() -> Self {
        Extent {
            min: [i64::MAX; 2],
            max: [i64::MIN; 2],
            sum: [0; 2],
            count: 0,
        }
    }

    fn add(&mut self, x: i32, y: i32) {
        for (a, c) in [x, y].map(i64::from).into_iter().enumerate() {
            self.min[a] = self.min[a].min(c);
            self.max[a] = self.max[a].max(c);
            self.sum[a] += c;
        }
        self.count += 1;
    }

    /// The island in tile units on a `size` tile. An axis the lifted pixels cover
    /// a whole period of is one the island loops round.
    fn island(&self, size: Size) -> Island {
        let mut island = Island::default();
        for (a, n) in [size.width, size.height].into_iter().enumerate() {
            let n = i64::from(n);
            let span = self.max[a] - self.min[a] + 1;
            if span >= n {
                continue;
            }
            let n = n as f64;
            let mean = self.sum[a] as f64 / self.count as f64 + 0.5;
            island.centroid[a] = (mean / n).rem_euclid(1.0) as f32;
            island.start[a] = (self.min[a] as f64 / n).rem_euclid(1.0) as f32;
            island.size[a] = (span as f64 / n) as f32;
        }
        island
    }
}

#[cfg(test)]
#[path = "flood_tests.rs"]
mod tests;
