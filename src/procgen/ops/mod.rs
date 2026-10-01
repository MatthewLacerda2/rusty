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
//! recipe, and a future material/shader leg all route through identical math.

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
/// Ops that need an input they weren't given fall back to a black canvas rather than
/// erroring, so a partially-wired recipe still bakes (the agent sees black, not a
/// crash).
pub fn eval_node(op: &OpKind, inputs: &[&Image], resolution: u32, seed: u64) -> Image {
    // Generators take no input; the rest consume `inputs`. Each category dispatches
    // in its own helper so no single match exceeds the function-length cap.
    match op {
        OpKind::Constant { .. }
        | OpKind::Noise { .. }
        | OpKind::Voronoi { .. }
        | OpKind::Gradient { .. }
        | OpKind::Wave { .. }
        | OpKind::Brick { .. }
        | OpKind::Shape { .. }
        | OpKind::Checker { .. }
        | OpKind::WhiteNoise => eval_generator(op, resolution, seed),
        OpKind::ColorRamp { .. }
        | OpKind::Mix { .. }
        | OpKind::Invert
        | OpKind::BrightContrast { .. }
        | OpKind::HueSatValue { .. }
        | OpKind::Gamma { .. } => eval_color(op, inputs, resolution),
        OpKind::Mapping { .. }
        | OpKind::BumpToNormal { .. }
        | OpKind::CombineRgb
        | OpKind::SeparateRgb { .. }
        | OpKind::Warp { .. } => eval_vector(op, inputs, resolution),
        OpKind::Math { .. } | OpKind::MapRange { .. } | OpKind::Clamp { .. } | OpKind::RgbToBw => {
            eval_math(op, inputs, resolution)
        }
        OpKind::Tile {
            count,
            jitter,
            rotation_jitter,
            scale_jitter,
        } => match inputs.first() {
            Some(src) => {
                let j = tile::Jitter::new(*jitter, *rotation_jitter, *scale_jitter);
                tile::tile(src, *count, j, seed)
            }
            None => Image::new(resolution),
        },
        OpKind::Blur { .. } | OpKind::Cavity { .. } | OpKind::Curvature { .. } => {
            eval_filter(op, inputs, resolution)
        }
    }
}

/// The single-input fallback: the first input, or a fresh black canvas if none.
fn first_or_black(inputs: &[&Image], resolution: u32) -> Image {
    inputs
        .first()
        .copied()
        .cloned()
        .unwrap_or_else(|| Image::new(resolution))
}

/// Paint a source op (no inputs) by sampling its generator per pixel.
fn eval_generator(op: &OpKind, resolution: u32, seed: u64) -> Image {
    match generators::sampler(op, resolution, seed) {
        Some(f) => Image::fill_uv(resolution, f),
        None => Image::new(resolution),
    }
}

/// Dispatch the color-grading ops.
fn eval_color(op: &OpKind, inputs: &[&Image], resolution: u32) -> Image {
    match op {
        OpKind::ColorRamp { stops } => color::color_ramp(first_or_black(inputs, resolution), stops),
        OpKind::Mix { mode, factor } => match (inputs.first(), inputs.get(1)) {
            (Some(a), Some(b)) => {
                color::mix((*a).clone(), b, inputs.get(2).copied(), *mode, *factor)
            }
            (Some(a), None) => (*a).clone(),
            _ => Image::new(resolution),
        },
        OpKind::Invert => color::invert(first_or_black(inputs, resolution)),
        OpKind::BrightContrast { bright, contrast } => {
            color::bright_contrast(first_or_black(inputs, resolution), *bright, *contrast)
        }
        OpKind::HueSatValue { hue, sat, value } => {
            color::hue_sat_value(first_or_black(inputs, resolution), *hue, *sat, *value)
        }
        OpKind::Gamma { gamma } => color::gamma(first_or_black(inputs, resolution), *gamma),
        _ => Image::new(resolution),
    }
}

/// Dispatch the vector / normal ops.
fn eval_vector(op: &OpKind, inputs: &[&Image], resolution: u32) -> Image {
    match op {
        OpKind::Mapping {
            scale,
            rotation,
            translation,
            tiling,
        } => match inputs.first() {
            Some(src) => vector::mapping(src, *scale, *rotation, *translation, *tiling),
            None => Image::new(resolution),
        },
        OpKind::BumpToNormal { strength } => match inputs.first() {
            Some(src) => vector::bump_to_normal(src, *strength),
            None => Image::filled(resolution, [0.5, 0.5, 1.0, 1.0]),
        },
        OpKind::CombineRgb => vector::combine_rgb(inputs, resolution),
        OpKind::Warp { strength } => match (inputs.first(), inputs.get(1)) {
            (Some(a), Some(by)) => vector::warp(a, by, *strength),
            (Some(a), None) => (*a).clone(),
            _ => Image::new(resolution),
        },
        OpKind::SeparateRgb { channel } => {
            vector::separate_rgb(first_or_black(inputs, resolution), *channel)
        }
        _ => Image::new(resolution),
    }
}

/// Dispatch the neighbourhood ops. Each reads its input's red as the field.
fn eval_filter(op: &OpKind, inputs: &[&Image], resolution: u32) -> Image {
    let Some(src) = inputs.first() else {
        return Image::new(resolution);
    };
    let px = |radius: f32| filter::pixel_radius(radius, src.resolution());
    match op {
        OpKind::Blur { radius } => filter::blur(src, px(*radius)),
        OpKind::Cavity { radius } => filter::cavity(src, px(*radius)),
        OpKind::Curvature { radius, strength } => filter::curvature(src, *radius, *strength),
        _ => Image::new(resolution),
    }
}

/// Dispatch the converter / math ops.
fn eval_math(op: &OpKind, inputs: &[&Image], resolution: u32) -> Image {
    match op {
        OpKind::Math { func, value } => {
            math::math(first_or_black(inputs, resolution), *func, *value)
        }
        OpKind::MapRange {
            from_min,
            from_max,
            to_min,
            to_max,
        } => math::map_range(
            first_or_black(inputs, resolution),
            *from_min,
            *from_max,
            *to_min,
            *to_max,
        ),
        OpKind::Clamp { min, max } => math::clamp(first_or_black(inputs, resolution), *min, *max),
        OpKind::RgbToBw => math::rgb_to_bw(first_or_black(inputs, resolution)),
        _ => Image::new(resolution),
    }
}
