//! A baked map's texels into an image (#1849): the covered texels, [`MARGIN`]
//! texels of fill around every UV island, and the map's neutral value beyond.

use glam::Vec4;

use super::{Baked, MeshMap, MARGIN};
use crate::procgen::image_buf::Image;
use crate::scene::lighting::lightmap::dilate;

/// What a texel no island reaches holds: the value that says "nothing here" for
/// each map (flat, open, no normal, the bounds' corner, thick, no slot).
fn background(map: MeshMap) -> Vec4 {
    match map {
        MeshMap::Curvature | MeshMap::WorldNormal => Vec4::new(0.5, 0.5, 0.5, 1.0),
        MeshMap::Ao | MeshMap::Thickness => Vec4::ONE,
        MeshMap::Position | MeshMap::Id => Vec4::W,
    }
}

/// `values` (one per covered texel, in `baked.points` order) as a `size`² image.
/// A continuous map's margin averages its filled neighbours, ring by ring, as the
/// lightmap's does; the ID map's copies one neighbour instead, so a margin never
/// holds a colour between two slots' that no slot has.
pub(super) fn image(map: MeshMap, baked: &Baked, values: &[Vec4]) -> Image {
    let size = baked.size;
    let n = (size as usize) * (size as usize);
    let mut texels = vec![Vec4::ZERO; n];
    let mut filled = vec![false; n];
    for (p, v) in baked.points.iter().zip(values) {
        texels[p.index] = *v;
        filled[p.index] = true;
    }
    match map {
        MeshMap::Id => copy_nearest(&mut texels, &mut filled, size, MARGIN),
        _ => dilate(&mut texels, &mut filled, size, MARGIN),
    }
    let empty = background(map);
    let mut out = Image::new(size);
    for ((px, v), f) in out.pixels_mut().iter_mut().zip(&texels).zip(&filled) {
        *px = if *f { *v } else { empty }.to_array();
    }
    out
}

/// Fill each empty texel bordering filled ones with the first filled neighbour (left,
/// right, up, down), `passes` rings deep.
fn copy_nearest(texels: &mut [Vec4], filled: &mut [bool], size: u32, passes: u32) {
    let size = size as usize;
    for _ in 0..passes {
        let before = filled.to_vec();
        for i in 0..texels.len() {
            if before[i] {
                continue;
            }
            let (x, y) = (i % size, i / size);
            let near = [
                (x > 0).then(|| i - 1),
                (x + 1 < size).then(|| i + 1),
                (y > 0).then(|| i - size),
                (y + 1 < size).then(|| i + size),
            ];
            if let Some(j) = near.into_iter().flatten().find(|&j| before[j]) {
                texels[i] = texels[j];
                filled[i] = true;
            }
        }
    }
}

/// Material slot `slot`'s ID colour: fully saturated hues a golden-ratio step apart,
/// so neighbouring slots never look alike and a slot's colour never depends on how
/// many slots the mesh has.
pub(super) fn id_colour(slot: usize) -> Vec4 {
    const GOLDEN: f32 = 0.618_034;
    let hue = (slot as f32 * GOLDEN).fract() * 6.0;
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
