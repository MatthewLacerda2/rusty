//! src/procgen/ops/mod.rs — the curated op-set, grouped by category, plus the
//! single-node dispatcher the runner calls.
//!
//! The op-set is "Blender-lite": the actually-used subset of Blender's texture/shader
//! nodes — enough to author the maps a PBR material needs, not a Substance clone. Ops
//! are grouped like Blender's node menus:
//!
//! - [`generators`] — sources with no input (constant, noise and its fBM / ridged /
//!   turbulence family, Voronoi F1 / F2 / edges, gradient,
//!   wave, brick, shape, checker, white noise).
//! - [`color`] — color ramp, mix (blend modes), invert, bright/contrast, HSV, gamma.
//! - [`vector`] — domain mapping, domain warp, bump→normal, combine/separate RGB;
//!   [`tile`] — the seeded grid scatter.
//! - [`math`] — math, map range, clamp, RGB→BW.
//! - [`filter`] — neighbourhood ops: separable blur, and the height-derived masks
//!   cavity and curvature.
//!
//! [`eval_node`] dispatches one [`OpKind`] given its inputs (already evaluated by the
//! runner). It is the single place op semantics live, so the Lua surface, a `.json`
//! recipe, and a future material/shader leg all route through identical math. Its
//! match is exhaustive, one arm per op, with no fallback: a new op does not compile
//! until it is dispatched (#1485).

pub mod color;
pub mod filter;
pub mod generators;
pub mod math;
pub mod tile;
pub mod vector;

use super::image_buf::Image;
use super::recipe::OpKind;

/// Evaluate one node's op. `inputs` are the buffers of the node's input ids in order;
/// `resolution`/`seed` come from the recipe. Returns the node's output image.
///
/// `inputs` must hold as many images as the op's [`Arity`](super::recipe::Arity)
/// accepts: the runner refuses a miswired node before anything evaluates (#1485), so
/// every required input is read directly, and this panics if called with fewer.
pub fn eval_node(op: &OpKind, inputs: &[&Image], resolution: u32, seed: u64) -> Image {
    let paint = |sampler| Image::fill_uv(resolution, sampler);
    let own = || inputs[0].clone();
    let px = |radius| filter::pixel_radius(radius, resolution);
    match op {
        OpKind::Constant(p) => paint(generators::constant(p)),
        OpKind::Noise(p) => paint(generators::noise(p, seed)),
        OpKind::Voronoi(p) => paint(generators::voronoi(p, seed)),
        OpKind::Gradient(p) => paint(generators::gradient(p.kind)),
        OpKind::Wave(p) => paint(generators::wave(p.kind, p.frequency)),
        OpKind::Brick(p) => paint(generators::brick(p, seed)),
        OpKind::Shape(p) => paint(generators::shape(p)),
        OpKind::Checker(p) => paint(generators::checker(p)),
        OpKind::WhiteNoise => paint(generators::white_noise(resolution, seed)),
        OpKind::ColorRamp(p) => color::color_ramp(own(), &p.stops),
        OpKind::Mix(p) => color::mix(own(), inputs[1], inputs.get(2).copied(), p.mode, p.factor),
        OpKind::Invert => color::invert(own()),
        OpKind::BrightContrast(p) => color::bright_contrast(own(), p.bright, p.contrast),
        OpKind::HueSatValue(p) => color::hue_sat_value(own(), p.hue, p.sat, p.value),
        OpKind::Gamma(p) => color::gamma(own(), p.gamma),
        OpKind::Mapping(p) => {
            vector::mapping(inputs[0], p.scale, p.rotation, p.translation, p.tiling)
        }
        OpKind::BumpToNormal(p) => vector::bump_to_normal(inputs[0], p.strength),
        OpKind::CombineRgb => vector::combine_rgb(inputs, resolution),
        OpKind::SeparateRgb(p) => vector::separate_rgb(own(), p.channel),
        OpKind::Tile(p) => tile::tile(inputs[0], p.count, tile::Jitter::of(p), seed),
        OpKind::Warp(p) => vector::warp(inputs[0], inputs[1], p.strength),
        OpKind::Math(p) => math::math(own(), p.func, p.value),
        OpKind::MapRange(p) => math::map_range(own(), p.from_min, p.from_max, p.to_min, p.to_max),
        OpKind::Clamp(p) => math::clamp(own(), p.min, p.max),
        OpKind::RgbToBw => math::rgb_to_bw(own()),
        OpKind::Blur(p) => filter::blur(inputs[0], px(p.radius)),
        OpKind::Cavity(p) => filter::cavity(inputs[0], px(p.radius)),
        OpKind::Curvature(p) => filter::curvature(inputs[0], p.radius, p.strength),
    }
}

#[cfg(test)]
mod arity_tests;
