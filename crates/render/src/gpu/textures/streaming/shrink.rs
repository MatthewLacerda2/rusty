//! A texture re-made with its top levels left out, copied on the GPU from the one it replaces.

use crate::gpu::ops::CountedQueue as _;
use crate::{GpuTexture, Renderer};
use rusty_asset::texture_import::variance::VarianceTable;

impl Renderer {
    /// `by` more top levels left out than `tex` has.
    pub(super) fn shrunk(&self, tex: &GpuTexture, by: u32) -> GpuTexture {
        let old = &tex.texture;
        let format = old.format();
        let size = old.size().mip_level_size(by, wgpu::TextureDimension::D2);
        let levels = old.mip_level_count() - by;
        let view_formats = sibling(format);
        let new = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Streamed Texture"),
            size,
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: old.usage(),
            view_formats: view_formats.as_slice(),
        });
        let mut encoder = (self.device).create_command_encoder(&Default::default());
        for mip in 0..levels {
            let extent = size.mip_level_size(mip, wgpu::TextureDimension::D2);
            encoder.copy_texture_to_texture(
                at(old, mip + by),
                at(&new, mip),
                extent.physical_size(format),
            );
        }
        self.queue.counted_submit([encoder.finish()]);
        let sampler = (tex.sampler.clone(), tex.filter);
        let data = format.remove_srgb_suffix();
        let mut made = Self::finalize_texture(
            &self.device,
            &self.texture_layout,
            (new, data),
            sampler,
            Some("Streamed Texture"),
        );
        made.source_size = tex.source_size;
        made.single_channel = tex.single_channel;
        made.deep_normal = tex.deep_normal;
        made.normal_variance = tex.normal_variance.map(|t| shift(t, by));
        made.mips = tex.mips.map(|m| m.dropping(by));
        made
    }
}

fn shift(table: VarianceTable, by: u32) -> VarianceTable {
    rusty_asset::texture_import::shift_variance(table, by)
}

/// The view format a streamed upload allows besides its own: its sRGB twin.
fn sibling(format: wgpu::TextureFormat) -> Vec<wgpu::TextureFormat> {
    let twins = [format.remove_srgb_suffix(), format.add_srgb_suffix()];
    twins.into_iter().filter(|&f| f != format).take(1).collect()
}

fn at(texture: &wgpu::Texture, mip_level: u32) -> wgpu::TexelCopyTextureInfo<'_> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level,
        origin: wgpu::Origin3d::ZERO,
        aspect: wgpu::TextureAspect::All,
    }
}
