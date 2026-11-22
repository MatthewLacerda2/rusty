use image::{DynamicImage, ImageBuffer, Rgba};

use super::extent;
use crate::GpuTexture;
use rusty_asset::sidecar::TextureSettings;
use rusty_asset::texture_import::{keeps_deep, TextureKind};
use rusty_core::color::f32_to_f16;

/// Half floats, not `Rgba16Unorm`: that needs a device feature and filters only where the
/// adapter allows; this is core, filterable and renderable.
pub(crate) const DEEP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub(crate) fn wanted(import: &TextureSettings, img: &DynamicImage, kind: TextureKind) -> bool {
    kind == TextureKind::Normal && keeps_deep(import, img.color())
}

/// A normal slot never takes an 8-bit upload of a 16-bit normal map: it gets its own copy.
pub(crate) fn serves(tex: &GpuTexture, kind: TextureKind) -> bool {
    let format = tex.texture.format();
    let short = kind == TextureKind::Normal && tex.deep_normal && format != DEEP_FORMAT;
    super::compressed::reads_as(format, kind) && !short
}

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
    // Each 16-bit unorm value as a half float in [0, 1]: eight bytes a texel.
    let bytes: Vec<u8> = image
        .as_raw()
        .iter()
        .flat_map(|&v| f32_to_f16(f32::from(v) / 65535.0).to_le_bytes())
        .collect();
    super::write_base(writes, &texture, &bytes, 8);
    texture
}
