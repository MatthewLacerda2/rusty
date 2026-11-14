//! A baked map's texels into an image (#1849): the covered texels, [`MARGIN`]
//! texels of fill around every UV island, and the map's neutral value beyond.
//!
//! The texels are written straight into the image a block of points at a time, and
//! the margin grows in place one ring at a time, so a map holds one image and one
//! flag per texel at its peak, never a second copy (at 8192² an `Image` is 1 GiB).

use glam::Vec4;

use super::{Baked, MeshMap, MARGIN};
use crate::lightmap::{run_pool, BakeProgress, TexelPoint};
use rusty_procgen::image_buf::{Image, Rgba};

/// Points evaluated per block: the values buffer a map holds before they are
/// written into its image (16 bytes each).
const BLOCK: usize = 1 << 18;

/// What a texel no island reaches holds: the value that says "nothing here" for
/// each map (flat, open, no normal, the bounds' corner, thick, no material).
fn background(map: MeshMap) -> Vec4 {
    match map {
        MeshMap::Curvature | MeshMap::WorldNormal => Vec4::new(0.5, 0.5, 0.5, 1.0),
        MeshMap::Ao | MeshMap::Thickness => Vec4::ONE,
        MeshMap::Position | MeshMap::Id => Vec4::W,
    }
}

/// `f(i, point)` of every covered texel (`i` its place in `baked.points`) as a
/// `size`² image. A continuous map's margin averages its filled neighbours, ring by
/// ring, as the lightmap's does; the ID map's copies one neighbour instead, so a
/// margin never holds a colour between two materials' that no material has.
pub(super) fn image(
    map: MeshMap,
    baked: &Baked,
    f: impl Fn(usize, &TexelPoint) -> Vec4 + Sync,
) -> Image {
    let size = baked.size;
    let mut out = Image::filled(size, background(map).to_array());
    let mut filled = vec![false; (size as usize) * (size as usize)];
    let px = out.pixels_mut();
    for (b, block) in baked.points.chunks(BLOCK).enumerate() {
        let values = per_texel(block, |k, p| f(b * BLOCK + k, p));
        for (p, v) in block.iter().zip(values) {
            px[p.at()] = v.to_array();
            filled[p.at()] = true;
        }
    }
    grow(px, &mut filled, size, MARGIN, map != MeshMap::Id);
    out
}

/// `f(k, point)` for every point, on the lightmapper's work pool (#1656): small
/// batches pulled off one counter, so a ray-heavy corner never holds up a thread's
/// whole share, and joined back in order: which worker ran a texel never shows.
/// The pool's items are unit placeholders (no memory); a batch's `first` and
/// length say which points it covers.
fn per_texel(points: &[TexelPoint], f: impl Fn(usize, &TexelPoint) -> Vec4 + Sync) -> Vec<Vec4> {
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

/// The eight neighbours a margin texel reads, in the lightmap dilation's order.
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

/// Fill each empty texel bordering filled ones, `passes` rings deep: with the mean
/// of its filled neighbours when `average`, else with the first of them. A ring is
/// read whole before it is written, so a texel never reads one filled in its pass.
fn grow(px: &mut [Rgba], filled: &mut [bool], size: u32, passes: u32, average: bool) {
    let s = size as i64;
    for _ in 0..passes {
        let mut ring: Vec<(usize, Vec4)> = Vec::new();
        for (i, _) in filled.iter().enumerate().filter(|(_, f)| !**f) {
            let (x, y) = (i as i64 % s, i as i64 / s);
            let near = NEIGHBOURS
                .iter()
                .map(|&(dx, dy)| (x + dx, y + dy))
                .filter(|&(nx, ny)| (0..s).contains(&nx) && (0..s).contains(&ny))
                .map(|(nx, ny)| (ny * s + nx) as usize)
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

/// The ID colour of material key `key` (`MeshSource::material_key`): fully
/// saturated hues a golden-ratio step apart, so neighbouring keys never look alike
/// and a material's colour never depends on how many the mesh has.
pub(super) fn id_colour(key: usize) -> Vec4 {
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
