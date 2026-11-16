//! A normal map kept at 16 bits (#2257): a `NormalMap` whose file stores more than
//! 8 bits a channel (`Texture.Bake`'s `normal16`) is never block-compressed
//! (`texture_import::is_deep_normal`), and its normal slot uploads it at
//! [`DEEP_FORMAT`] instead of 8 bits a channel, so a slope gentler than one 8-bit
//! step (0.45°) reads as a slope, not as terraces.
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
//! map with its mips: about 171 MiB, against 85 MiB and 21 MiB).

use image::{DynamicImage, Rgba32FImage};

use super::extent;
use rusty_asset::texture_import::{is_deep, TextureKind};
use rusty_core::color::f32_to_f16;

/// The format a 16-bit normal map uploads as (the module doc).
pub(crate) const DEEP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Whether a decoded `img` read for a slot holding `kind` uploads at
/// [`DEEP_FORMAT`]: a normal slot's image of more than 8 bits a channel. Any other
/// slot, or an 8-bit image, uploads as before.
pub(crate) fn wanted(img: &DynamicImage, kind: TextureKind) -> bool {
    kind == TextureKind::Normal && is_deep(img.color())
}

/// Create a [`DEEP_FORMAT`] texture of `levels` mips with `image` written to its
/// base; the caller draws the rest, as [`super::upload`]'s.
pub(super) fn upload(
    (device, queue): (&wgpu::Device, &wgpu::Queue),
    label: &str,
    image: &Rgba32FImage,
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
        .flat_map(|&v| f32_to_f16(v).to_le_bytes())
        .collect();
    super::write_base(queue, &texture, &bytes, 8);
    texture
}
