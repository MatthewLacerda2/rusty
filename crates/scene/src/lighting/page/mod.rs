//! crates/scene/src/lighting/page/mod.rs — a lightmap page as it is stored and read
//! back (#1694): its mip chain ([`levels`]), and the 8-bit encodings of its PNG
//! fallback, its direction page and its shadowmask (`encode`).
//!
//! The bake (`rusty-lighting`'s `lightmap::pages`) writes pages with these, and the
//! renderer's loader reads them back with the same functions, so a page decoded
//! from its PNG has the same chain the bake stored. A level halves its parent with
//! a 2×2 box filter in linear light, which also averages a direction texel's
//! vector (a mip of mixed directions is, rightly, less directional) and a
//! shadowmask's occlusion.
//!
//! stored-format: the `.lightmaps` pages' bytes (their mip chain and 8-bit encodings)

use std::ops::{Add, Div, Mul};

use glam::{Vec3, Vec4};

use rusty_asset::lightmap_page;

pub mod encode;

use encode::{encode_direction, encode_mask};

/// The mip levels a page keeps: the base and two halvings. The atlas
/// (`rusty-lighting`'s `lightmap::atlas`) keeps every one of them inside one map.
pub const MIP_LEVELS: u32 = 3;

/// A texel a level averages: linear light, a direction, an occlusion.
pub trait Texel:
    Copy + Default + Add<Output = Self> + Mul<f32, Output = Self> + Div<f32, Output = Self>
{
}

impl<T> Texel for T where
    T: Copy + Default + Add<Output = T> + Mul<f32, Output = T> + Div<f32, Output = T>
{
}

/// How many levels an `edge`² page is stored with: [`MIP_LEVELS`], fewer when a
/// level would not be whole blocks.
pub fn level_count(edge: u32) -> u32 {
    MIP_LEVELS.min(lightmap_page::max_levels(edge)).max(1)
}

/// The mip chain of an `edge`² page of linear values, base first.
pub fn levels(page: &[Vec3], edge: u32) -> Vec<Vec<Vec3>> {
    chain(page, edge)
}

/// [`levels`] of any page: a colour one, a direction one or a shadowmask.
fn chain<T: Texel>(page: &[T], edge: u32) -> Vec<Vec<T>> {
    let mut chain = vec![page.to_vec()];
    for l in 1..level_count(edge) {
        let parent = &chain[l as usize - 1];
        chain.push(half(parent, edge >> (l - 1)));
    }
    chain
}

/// A direction page's levels as their RGBA8 texels (`encode::encode_direction`):
/// what `rusty-lighting`'s `direction_page` stores, and what the loader makes of a progressive
/// preview's direction PNG (`render::gpu::lightmaps`).
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
