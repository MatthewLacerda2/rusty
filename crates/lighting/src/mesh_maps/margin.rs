use glam::Vec4;

use super::{Baked, Margin, MeshMap, MARGIN};
use crate::lightmap::{run_pool, BakeProgress, TexelPoint};
use rusty_procgen::image_buf::{Image, Rgba, Size};

/// Points evaluated per block, 16 bytes each: what a map buffers before its image.
pub(super) const BLOCK: usize = 1 << 18;

pub(super) fn image(
    map: MeshMap,
    baked: &Baked,
    f: impl Fn(usize, &TexelPoint) -> Vec4 + Sync,
) -> Image {
    let size = baked.settings.size;
    let row = map.row();
    let mut out = Image::filled(size, row.off.to_array());
    let mut filled = vec![false; size.pixels()];
    let px = out.pixels_mut();
    for (b, block) in baked.points.chunks(BLOCK).enumerate() {
        let values = per_texel(block, |k, p| f(b * BLOCK + k, p));
        for (p, v) in block.iter().zip(values) {
            px[p.at()] = v.to_array();
            filled[p.at()] = true;
        }
    }
    match row.margin {
        Margin::Average => grow(px, &mut filled, size, MARGIN, true),
        Margin::Copy => grow(px, &mut filled, size, MARGIN, false),
        Margin::None => {}
    }
    out
}

pub(super) fn per_texel(
    points: &[TexelPoint],
    f: impl Fn(usize, &TexelPoint) -> Vec4 + Sync,
) -> Vec<Vec4> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let trace = |_, first: usize, batch: &[()]| {
        let at = first..first + batch.len();
        at.map(|k| f(k, &points[k])).collect()
    };
    run_pool(
        vec![vec![(); points.len()]],
        threads,
        &BakeProgress::default(),
        trace,
        |_, _, values| values,
    )
    .and_then(|mut charts| charts.pop())
    .expect("a pool nobody cancels finishes its one chart")
}

const NEIGHBOURS: [(i64, i64); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

pub(super) fn grow(px: &mut [Rgba], filled: &mut [bool], size: Size, passes: u32, average: bool) {
    let (w, h) = (size.width as i64, size.height as i64);
    for _ in 0..passes {
        let mut ring: Vec<(usize, Vec4)> = Vec::new();
        for (i, _) in filled.iter().enumerate().filter(|(_, f)| !**f) {
            let (x, y) = (i as i64 % w, i as i64 / w);
            let near = NEIGHBOURS
                .iter()
                .map(|&(dx, dy)| (x + dx, y + dy))
                .filter(|&(nx, ny)| (0..w).contains(&nx) && (0..h).contains(&ny))
                .map(|(nx, ny)| (ny * w + nx) as usize)
                .filter(|&j| filled[j]);
            let (mut sum, mut n) = (Vec4::ZERO, 0.0f32);
            for j in near {
                sum += Vec4::from(px[j]);
                n += 1.0;
                if !average {
                    break;
                }
            }
            if n > 0.0 {
                ring.push((i, sum / n));
            }
        }
        if ring.is_empty() {
            return;
        }
        for (i, v) in ring {
            px[i] = v.to_array();
            filled[i] = true;
        }
    }
}

pub(super) fn id_colour(key: usize) -> Vec4 {
    // The golden ratio's fraction spreads successive ids' hues far apart.
    const GOLDEN: f32 = 0.618_034;
    let hue = (key as f32 * GOLDEN).fract() * 6.0;
    let x = 1.0 - (hue % 2.0 - 1.0).abs();
    let (r, g, b) = match hue as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    Vec4::new(r, g, b, 1.0)
}
