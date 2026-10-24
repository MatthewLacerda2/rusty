//! Mip chains for uploaded images (#1322). Every image texture gets a full chain,
//! built on the CPU at upload, so a tiled surface far from the camera samples a
//! level its footprint fits instead of aliasing the full-size image; Unity
//! generates mipmaps by default too.
//!
//! Each level is a 2×2 box filter of the one above. What the texels hold decides
//! how they average: a colour map is decoded from sRGB, averaged in linear light
//! and re-encoded (averaging the sRGB bytes darkens every mip of a high-contrast
//! texture), a data map averages its raw values. Alpha is always linear.
//!
//! [`MipSettings::for_map`] is the one place an upload decides whether to build a
//! chain and how; texture import settings (#1323) plug in there.

use std::sync::LazyLock;

use crate::asset::MapRef;

/// What a texture's texels hold, which decides how its mips average. A texture's
/// first use decides it: the cache keeps one upload per file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextureKind {
    /// Authored in sRGB and sampled through the decoding view (albedo, emissive,
    /// UI images, particles).
    Colour,
    /// Raw values sampled as stored (metallic, roughness, normal, extra slots).
    Data,
}

/// How an upload builds its mips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MipSettings {
    /// A full chain when true, the base level alone when false.
    pub generate: bool,
    pub kind: TextureKind,
}

impl MipSettings {
    /// The settings for `map`, used as `kind`. Unity's default: a full chain.
    /// Texture import settings (#1323) read the map's sidecar here.
    pub(crate) fn for_map(_map: &MapRef, kind: TextureKind) -> Self {
        Self {
            generate: true,
            kind,
        }
    }
}

/// How many levels a full chain of a `width`×`height` image has, down to 1×1.
pub(crate) fn full_chain_len(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

/// `base` and every level below it, halving each side (never below 1) down to
/// 1×1. With `generate` off, `base` alone.
pub(crate) fn chain(base: image::RgbaImage, settings: MipSettings) -> Vec<image::RgbaImage> {
    let levels = match settings.generate {
        true => full_chain_len(base.width(), base.height()),
        false => 1,
    };
    let mut chain = vec![base];
    for _ in 1..levels {
        let next = downsample(chain.last().unwrap(), settings.kind);
        chain.push(next);
    }
    chain
}

/// One level down: each texel averages the (up to) 2×2 texels above it; an odd
/// last row or column folds into its neighbour, a side of 1 stays 1.
fn downsample(src: &image::RgbaImage, kind: TextureKind) -> image::RgbaImage {
    let (w, h) = src.dimensions();
    let (dw, dh) = ((w / 2).max(1), (h / 2).max(1));
    let colour = kind == TextureKind::Colour;
    // The tables are dereferenced once per level, not once per texel.
    let tables = (&*SRGB_TO_LINEAR, LINEAR_TO_SRGB.as_slice());
    let texels = src.as_raw();
    let at = |x: u32, y: u32| ((y * w + x) * 4) as usize;
    let mut out = Vec::with_capacity((dw * dh * 4) as usize);
    for y in 0..dh {
        let rows = [2 * y, (2 * y + 1).min(h - 1)];
        for x in 0..dw {
            let cols = [2 * x, (2 * x + 1).min(w - 1)];
            let mut sum = [0.0f32; 4];
            for row in rows {
                for col in cols {
                    let i = at(col, row);
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s += decode(texels[i + c], colour && c < 3, tables.0);
                    }
                }
            }
            out.extend((0..4).map(|c| encode(sum[c] / 4.0, colour && c < 3, tables.1)));
        }
    }
    image::RgbaImage::from_raw(dw, dh, out).expect("a full level")
}

/// A byte as a value to average: linear light when `srgb`, else /255.
fn decode(byte: u8, srgb: bool, to_linear: &[f32; 256]) -> f32 {
    match srgb {
        true => to_linear[byte as usize],
        false => byte as f32 / 255.0,
    }
}

/// An averaged value back to a byte, re-encoded to sRGB when `srgb`.
fn encode(value: f32, srgb: bool, to_srgb: &[u8]) -> u8 {
    let v = value.clamp(0.0, 1.0);
    match srgb {
        true => to_srgb[(v * LINEAR_STEPS as f32).round() as usize],
        false => (v * 255.0).round() as u8,
    }
}

/// The sRGB transfer function's decode, one entry per byte.
static SRGB_TO_LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let c = i as f32 / 255.0;
        match c <= 0.04045 {
            true => c / 12.92,
            false => ((c + 0.055) / 1.055).powf(2.4),
        }
    })
});

/// Steps of the encode table: fine enough that the steepest stretch of the curve,
/// near black, moves less than half a byte per step.
const LINEAR_STEPS: usize = 4096;

/// The sRGB transfer function's encode, sampled at `LINEAR_STEPS` + 1 points so a
/// 2K chain costs no `powf` per texel.
static LINEAR_TO_SRGB: LazyLock<Vec<u8>> = LazyLock::new(|| {
    (0..=LINEAR_STEPS)
        .map(|i| {
            let l = i as f32 / LINEAR_STEPS as f32;
            let c = match l <= 0.003_130_8 {
                true => l * 12.92,
                false => 1.055 * l.powf(1.0 / 2.4) - 0.055,
            };
            (c * 255.0).round() as u8
        })
        .collect()
});

#[cfg(test)]
#[path = "mips_tests.rs"]
mod tests;
