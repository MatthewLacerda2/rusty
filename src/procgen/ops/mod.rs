//! src/procgen/ops/mod.rs — the curated op-set, grouped by category, plus the
//! single-node dispatcher the runner calls.
//!
//! The op-set is "Blender-lite": the actually-used subset of Blender's texture/shader
//! nodes — enough to author the maps a PBR material needs, not a Substance clone. Ops
//! are grouped like Blender's node menus:
//!
//! - [`generators`] — sources with no input (constant, noise and its fBM / ridged /
//!   turbulence family, Voronoi F1 / F2 / edges, gradient,
//!   wave, brick, shape, checker, white noise, and an image file).
//! - [`color`] — color ramp, mix (blend modes), invert, bright/contrast, HSV, gamma,
//!   auto levels.
//! - [`vector`] — domain mapping, domain warp, bump→normal, normal blend,
//!   combine/separate RGB;
//!   [`tile`] — the seeded grid scatter.
//! - [`math`] — math (against a constant or a second image), map range, clamp,
//!   RGB→BW.
//! - [`filter`] — neighbourhood ops: separable box or Gaussian blur, and the
//!   height-derived masks cavity and curvature; [`distance`] — a mask's distance
//!   from its edge, a bevel for any shape.
//!
//! [`eval_node`] dispatches one [`OpKind`] given its inputs (already evaluated by the
//! runner). It is the single place op semantics live, so the Lua surface, a `.json`
//! recipe, and a future material/shader leg all route through identical math. Its
//! match is exhaustive, one arm per op, with no fallback: a new op does not compile
//! until it is dispatched (#1485).

pub mod color;
pub mod distance;
pub mod filter;
pub mod generators;
pub mod math;
pub mod tile;
pub mod vector;

use super::image_buf::{Image, Size};
use super::recipe::OpKind;
use super::source::Sources;

/// Evaluate one node's op. `inputs` are the buffers of the node's input ids in order;
/// `size` is the recipe's extent, and `seed` is the recipe seed with the node's
/// own folded in ([`super::hash::node_seed`], done by the runner). `sources` holds
/// the decoded files `image` nodes read. Returns the node's output image.
///
/// `inputs` must hold as many images as the op's [`Arity`](super::recipe::Arity)
/// accepts: the runner refuses a miswired node before anything evaluates (#1485), so
/// every required input is read directly, and this panics if called with fewer. The
/// runner likewise refuses an `image` node whose file is not in `sources`.
pub fn eval_node(
    op: &OpKind,
    inputs: &[&Image],
    size: Size,
    seed: u64,
    sources: &Sources,
) -> Image {
    let paint = |sampler| Image::fill_uv(size, sampler);
    let own = || inputs[0].clone();
    let px = |radius| filter::pixel_radii(radius, size);
    match op {
        OpKind::Constant(p) => paint(generators::constant(p)),
        OpKind::Noise(p) => paint(generators::noise(p, seed)),
        OpKind::Voronoi(p) => paint(generators::voronoi(p, seed)),
        OpKind::Gradient(p) => paint(generators::gradient(p.kind)),
        OpKind::Wave(p) => paint(generators::wave(p.kind, p.frequency)),
        OpKind::Brick(p) => paint(generators::brick(p, seed)),
        OpKind::Shape(p) => paint(generators::shape(p)),
        OpKind::Checker(p) => paint(generators::checker(p)),
        OpKind::WhiteNoise(_) => paint(generators::white_noise(size, seed)),
        OpKind::Image(p) => {
            let source = sources
                .get(&p.path)
                .expect("the runner checks every image source");
            generators::image(source, p.encoding, size)
        }
        OpKind::ColorRamp(p) => color::color_ramp(own(), &p.stops, p.interpolation),
        OpKind::Mix(p) => color::mix(own(), inputs[1], inputs.get(2).copied(), p.mode, p.factor),
        OpKind::Invert => color::invert(own()),
        OpKind::BrightContrast(p) => color::bright_contrast(own(), p.bright, p.contrast),
        OpKind::HueSatValue(p) => color::hue_sat_value(own(), p.hue, p.sat, p.value),
        OpKind::Gamma(p) => color::gamma(own(), p.gamma),
        OpKind::AutoLevels => color::auto_levels(own()),
        OpKind::Mapping(p) => {
            vector::mapping(inputs[0], p.scale, p.rotation, p.translation, p.tiling)
        }
        OpKind::BumpToNormal(p) => vector::bump_to_normal(inputs[0], p.strength),
        OpKind::CombineRgb => vector::combine_rgb(inputs, size),
        OpKind::SeparateRgb(p) => vector::separate_rgb(own(), p.channel),
        OpKind::Tile(p) => tile::tile(inputs[0], p.count, tile::Jitter::of(p), seed),
        OpKind::Warp(p) => vector::warp(inputs[0], inputs[1], p.strength),
        OpKind::NormalBlend(p) => vector::normal_blend(inputs[0], inputs[1], p.strength),
        OpKind::Math(p) => math::math(own(), inputs.get(1).copied(), p.func, p.value),
        OpKind::MapRange(p) => math::map_range(own(), p.from_min, p.from_max, p.to_min, p.to_max),
        OpKind::Clamp(p) => math::clamp(own(), p.min, p.max),
        OpKind::RgbToBw => math::rgb_to_bw(own()),
        OpKind::Blur(p) => filter::blur(inputs[0], px(p.radius), p.kind),
        OpKind::Cavity(p) => filter::cavity(inputs[0], px(p.radius)),
        OpKind::Curvature(p) => filter::curvature(inputs[0], p.radius, p.strength),
        OpKind::Distance(p) => distance::distance(inputs[0], p.radius),
    }
}

#[cfg(test)]
mod arity_tests;
#[cfg(test)]
mod aspect_tests;
