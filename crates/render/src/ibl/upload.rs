use super::cubemap::{
    parse_ktx2_cubemap_mips, CubeShape, CubemapMips, CubemapTexture, Refusal, CUBE_FORMAT,
};
use super::reflection_prefilter::TEXEL_BYTES;
use crate::gpu::ops::CountedQueue as _;
use crate::Renderer;

impl Renderer {
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

    fn write_cube_level(&self, texture: &wgpu::Texture, level: u32, size: u32, bytes: &[u8]) {
        let format = texture.format();
        let (extent, rows) = (
            block_extent(format, size),
            size.div_ceil(block_edge(format)),
        );
        let row_bytes = rows * format.block_copy_size(None).unwrap_or(TEXEL_BYTES as u32);
        self.queue.counted_write_texture(
            &self.staging,
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

    fn finish_cube_texture(texture: wgpu::Texture, size: u32) -> CubemapTexture {
        CubemapTexture { texture, size }
    }
}

fn block_edge(format: wgpu::TextureFormat) -> u32 {
    format.block_dimensions().0
}

pub(crate) fn block_extent(format: wgpu::TextureFormat, size: u32) -> u32 {
    size.next_multiple_of(block_edge(format))
}

#[cfg(test)]
#[path = "upload_tests.rs"]
mod upload_tests;
#[cfg(test)]
pub(crate) use upload_tests::chained_cube;
