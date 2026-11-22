//! 8-connected, wrapping both axes. Each flooded pixel gets a *lifted* coordinate (its step from
//! the first pixel, summed, never wrapped), so an island across an edge measures compact. An island
//! that loops round the tile on an axis has no edge there: its box spans the tile, starting at 0,
//! and its centre reads 0.5.

use std::collections::BTreeMap;

use crate::hash;
use crate::image_buf::{Image, Rgba, Size};

/// Coarse enough that the same island at another resolution mostly keeps its cell.
const IDENTITY_GRID: f32 = 128.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Island {
    /// The mean of its pixel centres, wrapped into `[0, 1)`; 0.5 on an axis it spans.
    pub centroid: [f32; 2],
    pub start: [f32; 2],
    pub size: [f32; 2],
    pub rank: u32,
}

impl Island {
    pub fn centre(&self) -> [f32; 2] {
        [0, 1].map(|a| (self.start[a] + self.size[a] / 2.0).rem_euclid(1.0))
    }

    pub fn random(&self, channel: u64, seed: u64) -> f32 {
        let [x, y] = self.cell();
        // Salted as `rank + 1`: the mixer maps 0 to 0, so an all-zero input is avoided.
        let salt = (channel << 32) ^ (u64::from(self.rank) + 1);
        hash::unit2(x, y, salt, seed)
    }

    fn cell(&self) -> [i64; 2] {
        self.centroid.map(|c| (c * IDENTITY_GRID).floor() as i64)
    }
}

pub struct Islands {
    size: Size,
    labels: Vec<u32>,
    pub islands: Vec<Island>,
}

impl Islands {
    pub fn at(&self, x: usize, y: usize) -> Option<&Island> {
        let label = self.labels[y * self.size.width as usize + x];
        label.checked_sub(1).map(|k| &self.islands[k as usize])
    }
}

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
    let mut seen: BTreeMap<[i64; 2], u32> = BTreeMap::new();
    for island in &mut islands {
        let count = seen.entry(island.cell()).or_default();
        island.rank = *count;
        *count += 1;
    }
    Islands {
        size,
        labels,
        islands,
    }
}

pub fn of_label_image(img: &Image) -> Islands {
    label(img, |p| p[3] > 0.5)
}

pub fn paint<P: Sync>(
    islands: &Islands,
    per: impl Fn(&Island) -> P,
    f: impl Fn(&P, f32, f32) -> f32 + Sync,
) -> Image {
    paint_rgba(islands, [0.0, 0.0, 0.0, 1.0], per, |p, u, v| {
        let g = f(p, u, v);
        [g, g, g, 1.0]
    })
}

pub fn paint_rgba<P: Sync>(
    islands: &Islands,
    off: Rgba,
    per: impl Fn(&Island) -> P,
    f: impl Fn(&P, f32, f32) -> Rgba + Sync,
) -> Image {
    let table: Vec<P> = islands.islands.iter().map(per).collect();
    let size = islands.size;
    let (w, h) = (size.width as f32, size.height as f32);
    let mut out = Image::new(size);
    out.for_each_row(|y, row| {
        let v = (y as f32 + 0.5) / h;
        let labels = &islands.labels[y * size.width as usize..][..row.len()];
        for ((x, p), label) in row.iter_mut().enumerate().zip(labels) {
            *p = label
                .checked_sub(1)
                .map_or(off, |k| f(&table[k as usize], (x as f32 + 0.5) / w, v));
        }
    });
    out
}

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
            rank: 0,
        }
    }
}

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
