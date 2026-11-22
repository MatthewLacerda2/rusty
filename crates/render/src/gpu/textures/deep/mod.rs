use image::{DynamicImage, ImageBuffer, Rgba};

use super::extent;
use crate::GpuTexture;
use rusty_asset::sidecar::TextureSettings;
use rusty_asset::texture_import::{keeps_deep, TextureKind};
use rusty_core::color::f32_to_f16;

pub(crate) const DEEP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The record's `use_normal_map` for a normal map stored signed: the shader reads
/// its x, y and z as they are, with no `* 2 - 1`.
pub(crate) const SIGNED_NORMAL_MAP: u32 = 3;

pub(crate) fn wanted(import: &TextureSettings, img: &DynamicImage, kind: TextureKind) -> bool {
    kind == TextureKind::Normal && keeps_deep(import, img.color())
}

pub(crate) fn serves(tex: &GpuTexture, kind: TextureKind) -> bool {
    let format = tex.texture.format();
    let short = kind == TextureKind::Normal && tex.deep_normal && format != DEEP_FORMAT;
    super::compressed::reads_as(format, kind) && !short
}

/// One channel as uploaded: x, y and z signed (`v * 2 - 1`), where half floats are
/// finest near flat's 0; alpha as the file's 0..1.
pub(crate) fn stored(channel: usize, v: u16) -> u16 {
    let unit = f64::from(v) / 65535.0;
    let value = if channel < 3 { unit * 2.0 - 1.0 } else { unit };
    f32_to_f16(value as f32)
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
    let bytes: Vec<u8> = image
        .as_raw()
        .iter()
        .enumerate()
        .flat_map(|(i, &v)| stored(i % 4, v).to_le_bytes())
        .collect();
    super::write_base(writes, &texture, &bytes, 8);
    texture
}

#[cfg(test)]
mod draw_gpu_tests;
#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod tests;
