//! src/procgen/ops/mod.rs — the curated op-set, grouped by category, plus the
//! single-node dispatcher the runner calls.
//!
//! The op-set is "Blender-lite": the actually-used subset of Blender's texture/shader
//! nodes — enough to author the maps a PBR material needs, not a Substance clone. Ops
//! are grouped like Blender's node menus:
//!
//! - [`generators`] — sources with no input (constant, noise, Voronoi, gradient,
//!   wave, brick, checker, white noise).
//! - [`color`] — color ramp, mix (blend modes), invert, bright/contrast, HSV, gamma.
//! - [`vector`] — domain mapping, bump→normal, combine/separate RGB.
//! - [`math`] — math, map range, clamp, RGB→BW.
//! - [`filter`] — separable blur.
//!
//! [`eval_node`] dispatches one [`OpKind`] given its inputs (already evaluated by the
//! runner). It is the single place op semantics live, so the Lua surface, a `.json`
//! recipe, and a future material/shader leg all route through identical math.

pub mod color;
pub mod filter;
pub mod generators;
pub mod math;
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
        | OpKind::SeparateRgb { .. } => eval_vector(op, inputs, resolution),
        OpKind::Math { .. } | OpKind::MapRange { .. } | OpKind::Clamp { .. } | OpKind::RgbToBw => {
            eval_math(op, inputs, resolution)
        }
        OpKind::Blur { radius } => match inputs.first() {
            Some(src) => filter::blur(src, *radius),
            None => Image::new(resolution),
        },
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

/// Dispatch the source ops (no inputs).
fn eval_generator(op: &OpKind, resolution: u32, seed: u64) -> Image {
    match op {
        OpKind::Constant { color } => generators::constant(resolution, *color),
        OpKind::Noise {
            kind,
            scale,
            octaves,
        } => generators::noise(resolution, *kind, *scale, *octaves, seed),
        OpKind::Voronoi { scale, output } => generators::voronoi(resolution, *scale, *output, seed),
        OpKind::Gradient { kind } => generators::gradient(resolution, *kind),
        OpKind::Wave { kind, frequency } => generators::wave(resolution, *kind, *frequency),
        OpKind::Brick { rows, cols, mortar } => {
            generators::brick(resolution, *rows, *cols, *mortar)
        }
        OpKind::Checker {
            tiles,
            color_a,
            color_b,
        } => generators::checker(resolution, *tiles, *color_a, *color_b),
        OpKind::WhiteNoise => generators::white_noise(resolution, seed),
        _ => Image::new(resolution),
    }
}

/// Dispatch the color-grading ops.
fn eval_color(op: &OpKind, inputs: &[&Image], resolution: u32) -> Image {
    match op {
        OpKind::ColorRamp { stops } => color::color_ramp(first_or_black(inputs, resolution), stops),
        OpKind::Mix { mode, factor } => match (inputs.first(), inputs.get(1)) {
            (Some(a), Some(b)) => color::mix((*a).clone(), b, *mode, *factor),
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
        } => match inputs.first() {
            Some(src) => vector::mapping(src, *scale, *rotation, *translation),
            None => Image::new(resolution),
        },
        OpKind::BumpToNormal { strength } => match inputs.first() {
            Some(src) => vector::bump_to_normal(src, *strength),
            None => Image::filled(resolution, [0.5, 0.5, 1.0, 1.0]),
        },
        OpKind::CombineRgb => vector::combine_rgb(inputs, resolution),
        OpKind::SeparateRgb { channel } => {
            vector::separate_rgb(first_or_black(inputs, resolution), *channel)
        }
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
