//! A normal map kept at 16 bits (#2257): a `NormalMap` whose file stores more than
//! 8 bits a channel (`Texture.Bake`'s `normal16`, `texture_import::keeps_deep`) is
//! never block-compressed, and a normal slot uploads it at [`DEEP_FORMAT`] instead
//! of 8 bits a channel, so a slope gentler than one 8-bit step (0.45°) reads as a
//! slope, not as terraces. Whichever slot loads the file first, a normal slot gets
//! the 16-bit upload ([`serves`]); any other slot reads it at 8 bits, or reads the
//! 16-bit upload where one is already made (any slot but colour).
//!
//! **Why half floats.** `Rgba16Unorm` (Unity's RGBA64) needs a device feature, and
//! even with it wgpu filters it only where the adapter says so; `Rgba16Float` is
//! core, filterable and renderable (its mips are drawn as any data map's). A
//! normal's x and y near 0 are stored near 0.5, where a half steps every 2⁻¹¹:
//! a step of about 0.056° in the normal, eight times finer than 8 bits, and finer
//! still for x and y below 0. It keeps blue, so the shader's mip spread
//! (`mapping::stored_length`) works as it does for an 8-bit map.
//!
//! **What it costs**: 8 bytes a texel, twice RGBA8 and eight times BC5 (a 4096²
//! map with its mips: about 171 MiB, against 85 MiB and 21 MiB). Uploading it
//! holds the decoded 16-bit image and its half floats at once, 128 MiB each for a
//! 4096² map, until `write_texture` has copied the bytes.

use image::{DynamicImage, ImageBuffer, Rgba};

use super::extent;
use crate::GpuTexture;
use rusty_asset::sidecar::TextureSettings;
use rusty_asset::texture_import::{keeps_deep, TextureKind};
use rusty_core::color::f32_to_f16;

/// The format a 16-bit normal map uploads as (the module doc).
pub(crate) const DEEP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Whether a decoded `img` with `import` settings, read for a slot holding `kind`,
/// uploads at [`DEEP_FORMAT`]: a normal slot's normal map kept at 16 bits (the
/// importer's own test, `texture_import::keeps_deep`). Any other slot, or any
/// other image, uploads as before.
pub(crate) fn wanted(import: &TextureSettings, img: &DynamicImage, kind: TextureKind) -> bool {
    kind == TextureKind::Normal && keeps_deep(import, img.color())
}

/// Whether the cached upload `tex` serves a slot holding `kind`: its format reads
/// right there (`compressed::reads_as`), and a normal slot never gets an 8-bit
/// upload of a normal map kept at 16 bits, which another slot loaded first; it
/// gets its own 16-bit copy instead (`Renderer::load_texture_as`'s fallback).
pub(crate) fn serves(tex: &GpuTexture, kind: TextureKind) -> bool {
    let format = tex.texture.format();
    let short = kind == TextureKind::Normal && tex.deep_normal && format != DEEP_FORMAT;
    super::compressed::reads_as(format, kind) && !short
}

/// Create a [`DEEP_FORMAT`] texture of `levels` mips with `image` written to its
/// base, each 16-bit value turned straight into a half float; the caller draws
/// the rest, as [`super::upload`]'s.
pub(super) fn upload(
    (device, writes): (&wgpu::Device, crate::gpu::ops::Writes),
    label: &str,
    image: &ImageBuffer<Rgba<u16>, Vec<u16>>,
    levels: u32,
) -> wgpu::Texture {
    let (width, height) = image.dimensions();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: extent(width, height),
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEEP_FORMAT,
        usage: super::usage(levels),
        view_formats: &[],
    });
    let bytes: Vec<u8> = image
        .as_raw()
        .iter()
        .flat_map(|&v| f32_to_f16(f32::from(v) / 65535.0).to_le_bytes())
        .collect();
    super::write_base(writes, &texture, &bytes, 8);
    texture
}
