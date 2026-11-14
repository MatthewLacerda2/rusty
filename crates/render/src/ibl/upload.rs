//! Uploading a baked reflection cube (#245, #1885): read from disk, parsed by
//! `cubemap`, and written into a mipped cube texture in the probe array's format,
//! BC6H blocks as the file holds them, or decoded to `Rgba16Float` where the
//! adapter lacks `TEXTURE_COMPRESSION_BC`.

use super::cubemap::{
    parse_ktx2_cubemap_mips, CubeShape, CubemapMips, CubemapTexture, Refusal, CUBE_FORMAT,
};
use super::reflection_prefilter::TEXEL_BYTES;
use crate::Renderer;

impl Renderer {
    /// Load a reflection-probe cubemap WITH its roughness mip chain from `path` (#245),
    /// into a mipped cube texture the probe array copies in, in the array's format:
    /// the file's BC6H blocks as they are, or decoded to `Rgba16Float` where the
    /// adapter lacks BC (#1885). Refused when the file is missing, is not a usable
    /// BC6H cubemap, or is not the `expect`ed shape (size and mip count) the frame
    /// planned for, which is parsed but never uploaded (#1245, #1609).
    pub(crate) fn load_cubemap_mips(
        &self,
        path: &str,
        expect: CubeShape,
    ) -> Result<CubemapTexture, Refusal> {
        let bytes = std::fs::read(path).map_err(|_| "it cannot be read")?;
        let data = parse_ktx2_cubemap_mips(&bytes)?;
        if data.shape() != expect {
            return Err("it changed shape since its header was read");
        }
        let data = match self.reflection_probes.format() {
            CUBE_FORMAT => data,
            _ => data.decoded(),
        };
        Ok(self.upload_cubemap_mips(path, &data))
    }

    /// Upload a decoded mipped cubemap into a mipped 6-layer cube texture with a cube view
    /// + trilinear sampler. Each mip's six faces upload at their own level/size.
    fn upload_cubemap_mips(&self, label: &str, data: &CubemapMips) -> CubemapTexture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: data.size,
                height: data.size,
                depth_or_array_layers: 6,
            },
            mip_level_count: data.mips.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: data.format,
            // COPY_SRC: the probe array copies it into its layer (#1206).
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        for (level, bytes) in data.mips.iter().enumerate() {
            let level_size = (data.size >> level).max(1);
            self.write_cube_level(&texture, level as u32, level_size, bytes);
        }
        Self::finish_cube_texture(texture, data.size)
    }

    /// Write one mip level's six faces into a cube texture: whole blocks for BC6H, so
    /// a level under 4×4 writes its one padded block ([`block_extent`]).
    fn write_cube_level(&self, texture: &wgpu::Texture, level: u32, size: u32, bytes: &[u8]) {
        let format = texture.format();
        let (extent, rows) = (
            block_extent(format, size),
            size.div_ceil(block_edge(format)),
        );
        let row_bytes = rows * format.block_copy_size(None).unwrap_or(TEXEL_BYTES as u32);
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(rows),
            },
            wgpu::Extent3d {
                width: extent,
                height: extent,
                depth_or_array_layers: 6,
            },
        );
    }

    /// Wrap an uploaded cube texture.
    fn finish_cube_texture(texture: wgpu::Texture, size: u32) -> CubemapTexture {
        CubemapTexture { texture, size }
    }
}

/// The edge of one of `format`'s blocks: 4 for BC6H, 1 for `Rgba16Float`.
fn block_edge(format: wgpu::TextureFormat) -> u32 {
    format.block_dimensions().0
}

/// The edge a copy of a `size`-texel cube level moves in `format`: `size` rounded
/// up to whole blocks, as wgpu wants a BC mip under 4×4 copied (its physical size).
pub(crate) fn block_extent(format: wgpu::TextureFormat, size: u32) -> u32 {
    size.next_multiple_of(block_edge(format))
}

#[cfg(test)]
#[path = "upload_tests.rs"]
mod upload_tests;
#[cfg(test)]
pub(crate) use upload_tests::chained_cube;
