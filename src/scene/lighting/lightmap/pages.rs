//! A packed atlas page as the GPU stores it (#1694): its mip chain, then BC6H for
//! irradiance (`asset::lightmap_page`); a direction page's and a shadowmask's
//! levels stay RGBA8 ([`direction_levels`], [`mask_levels`]), never block-compressed:
//! BC7 turned a lamp's direction up to 5° off a texel, which a glossy surface's
//! baked highlight showed as a sawtooth (#1996). The levels are made
//! here, after the bake's chart dilation and fill (`raster::fill_rest`), so a mip
//! averages a chart's light and its own edge, never the empty background, and
//! the atlas's cells (`atlas`) keep every level of [`MIP_LEVELS`] inside one map.
//!
//! A level halves its parent with a 2×2 box filter in linear light. A direction
//! texel's vector is a direction scaled by its directionality, so averaging four of
//! them shortens the result where they disagree: a mip of mixed directions is,
//! rightly, less directional. Its lamp share (`encode`) averages as plainly.
//!
//! The RGBM PNG (`encode`) stays beside each KTX2 page, as the fallback for a GPU
//! without BC compression and as what an agent can open; [`levels`] makes that
//! fallback's mips too, so both paths sample the same chain.

use glam::{Vec3, Vec4};

use super::atlas::MIP_LEVELS;
use super::encode::{encode_direction, encode_mask};
use super::raster::Texel;
use crate::asset::lightmap_page::{self, encode_bc6h, Page};

/// How many levels an `edge`² page is stored with: [`MIP_LEVELS`], fewer when a
/// level would not be whole blocks.
pub fn level_count(edge: u32) -> u32 {
    MIP_LEVELS.min(lightmap_page::max_levels(edge)).max(1)
}

/// The mip chain of an `edge`² page of linear values, base first.
pub fn levels(page: &[Vec3], edge: u32) -> Vec<Vec<Vec3>> {
    chain(page, edge)
}

/// [`levels`] of any page: a colour one or a direction one.
fn chain<T: Texel>(page: &[T], edge: u32) -> Vec<Vec<T>> {
    let mut chain = vec![page.to_vec()];
    for l in 1..level_count(edge) {
        let parent = &chain[l as usize - 1];
        chain.push(half(parent, edge >> (l - 1)));
    }
    chain
}

/// An irradiance page, every level, as BC6H; `None` when `stop` answers true
/// before a level.
pub fn colour_page(
    page: &[Vec3],
    edge: u32,
    threads: usize,
    stop: &dyn Fn() -> bool,
) -> Option<Page> {
    encode_bc6h(edge, &levels(page, edge), threads, stop)
}

/// A direction page's levels as their RGBA8 texels (`encode::encode_direction`):
/// what the loader uploads (`render::gpu::lightmaps`).
pub fn direction_levels(page: &[Vec4], edge: u32) -> Vec<Vec<[u8; 4]>> {
    chain(page, edge)
        .iter()
        .map(|level| level.iter().map(|&d| encode_direction(d)).collect())
        .collect()
}

/// A shadowmask page's levels as RGBA8 texels (#1695, `encode::encode_mask`): a
/// mip averages occlusion as it averages light, so a far shadow edge softens.
pub fn mask_levels(page: &[Vec4], edge: u32) -> Vec<Vec<[u8; 4]>> {
    chain(page, edge)
        .iter()
        .map(|level| level.iter().map(|&m| encode_mask(m)).collect())
        .collect()
}

/// A `edge`² level halved by a 2×2 box.
fn half<T: Texel>(level: &[T], edge: u32) -> Vec<T> {
    let (edge, out) = (edge as usize, (edge / 2) as usize);
    let mut halved = Vec::with_capacity(out * out);
    for y in 0..out {
        for x in 0..out {
            let at = |dx: usize, dy: usize| level[(2 * y + dy) * edge + 2 * x + dx];
            halved.push((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) * 0.25);
        }
    }
    halved
}
