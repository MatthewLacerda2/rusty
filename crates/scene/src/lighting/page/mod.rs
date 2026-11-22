//! stored-format: the `.lightmaps` pages' bytes (their mip chain and 8-bit encodings)
//! The bake writes pages with these and the renderer's loader reads them back, so a page
//! decoded from its PNG has the chain the bake stored.

use std::ops::{Add, Div, Mul};

use glam::{Vec3, Vec4};

use rusty_asset::lightmap_page;

pub mod encode;

use encode::{encode_direction, encode_mask};

/// The base and two halvings; the atlas keeps every level inside one map.
pub const MIP_LEVELS: u32 = 3;

pub trait Texel:
    Copy + Default + Add<Output = Self> + Mul<f32, Output = Self> + Div<f32, Output = Self>
{
}

impl<T> Texel for T where
    T: Copy + Default + Add<Output = T> + Mul<f32, Output = T> + Div<f32, Output = T>
{
}

pub fn level_count(edge: u32) -> u32 {
    MIP_LEVELS.min(lightmap_page::max_levels(edge)).max(1)
}

pub fn levels(page: &[Vec3], edge: u32) -> Vec<Vec<Vec3>> {
    chain(page, edge)
}

fn chain<T: Texel>(page: &[T], edge: u32) -> Vec<Vec<T>> {
    let mut chain = vec![page.to_vec()];
    for l in 1..level_count(edge) {
        let parent = &chain[l as usize - 1];
        chain.push(half(parent, edge >> (l - 1)));
    }
    chain
}

pub fn direction_levels(page: &[Vec4], edge: u32) -> Vec<Vec<[u8; 4]>> {
    chain(page, edge)
        .iter()
        .map(|level| level.iter().map(|&d| encode_direction(d)).collect())
        .collect()
}

pub fn mask_levels(page: &[Vec4], edge: u32) -> Vec<Vec<[u8; 4]>> {
    chain(page, edge)
        .iter()
        .map(|level| level.iter().map(|&m| encode_mask(m)).collect())
        .collect()
}

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
