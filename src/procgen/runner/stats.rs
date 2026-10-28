//! Per-node value statistics of a recipe (#1486), the numbers `Texture.Stats` returns.
//!
//! An agent authoring a recipe never sees its intermediate maps, so it reads their
//! value distribution instead: before setting a `color_ramp`'s stops or a
//! `map_range`'s bounds it needs to know where the input actually sits (a 4-octave
//! fBM spans only about 0.35–0.65). Every node is evaluated once, through
//! [`super::evaluate_each`], and reduced to numbers as it finishes.

use super::{evaluate_each, RunError};
use crate::procgen::image_buf::{Image, Rgba};
use crate::procgen::recipe::TextureRecipe;

/// One node's value distribution, each field per channel (R, G, B, A).
#[derive(Clone, Debug, PartialEq)]
pub struct NodeStats {
    pub min: Rgba,
    pub max: Rgba,
    pub mean: Rgba,
    /// Population standard deviation.
    pub std: Rgba,
    /// The 1st and 99th percentiles, exact (nearest rank): the range all but the
    /// outer 2% of pixels fall in, which a stray hot pixel cannot stretch.
    pub p1: Rgba,
    pub p99: Rgba,
}

/// Every node's [`NodeStats`], in `recipe.nodes` order, from one evaluation.
pub fn stats(recipe: &TextureRecipe) -> Result<Vec<NodeStats>, RunError> {
    let mut out: Vec<Option<NodeStats>> = recipe.nodes.iter().map(|_| None).collect();
    let mut scratch = Vec::new();
    evaluate_each(recipe, |i, img| {
        out[i] = Some(node_stats(img, &mut scratch))
    })?;
    Ok(out
        .into_iter()
        .map(|s| s.expect("evaluate_each visits every node"))
        .collect())
}

/// Reduce one image to its per-channel statistics; `scratch` is reused between
/// calls for the percentile selection.
pub fn node_stats(img: &Image, scratch: &mut Vec<f32>) -> NodeStats {
    let mut s = NodeStats {
        min: [0.0; 4],
        max: [0.0; 4],
        mean: [0.0; 4],
        std: [0.0; 4],
        p1: [0.0; 4],
        p99: [0.0; 4],
    };
    for c in 0..4 {
        scratch.clear();
        scratch.extend(img.pixels().iter().map(|p| p[c]));
        let n = scratch.len() as f64;
        let mean = scratch.iter().map(|&v| f64::from(v)).sum::<f64>() / n;
        let var = scratch
            .iter()
            .map(|&v| (f64::from(v) - mean).powi(2))
            .sum::<f64>()
            / n;
        s.mean[c] = mean as f32;
        s.std[c] = var.sqrt() as f32;
        s.min[c] = scratch.iter().copied().fold(f32::INFINITY, f32::min);
        s.max[c] = scratch.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        s.p1[c] = percentile(scratch, 1);
        s.p99[c] = percentile(scratch, 99);
    }
    s
}

/// The exact `p`th percentile of `values` by nearest rank (the smallest value with
/// at least `p`% of the values at or below it). Reorders `values`.
fn percentile(values: &mut [f32], p: usize) -> f32 {
    let rank = (p * values.len()).div_ceil(100).max(1);
    *values.select_nth_unstable_by(rank - 1, f32::total_cmp).1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_are_the_nearest_rank() {
        let mut v: Vec<f32> = (1..=200).rev().map(|i| i as f32).collect();
        assert_eq!(percentile(&mut v, 1), 2.0);
        assert_eq!(percentile(&mut v, 99), 198.0);
        assert_eq!(percentile(&mut [7.0], 1), 7.0);
    }

    #[test]
    fn a_two_value_image_has_its_mean_and_spread() {
        let mut img = Image::filled(2, [0.0, 1.0, 0.5, 1.0]);
        img.pixels_mut()[0][0] = 1.0;
        img.pixels_mut()[1][0] = 1.0;
        let s = node_stats(&img, &mut Vec::new());
        assert_eq!(
            (s.min[0], s.max[0], s.mean[0], s.std[0]),
            (0.0, 1.0, 0.5, 0.5)
        );
        assert_eq!((s.p1[0], s.p99[0]), (0.0, 1.0));
        assert_eq!((s.mean[1], s.std[1]), (1.0, 0.0));
    }
}
