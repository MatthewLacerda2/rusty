use super::super::cubemap::CubemapTexture;
use super::super::upload::block_extent;
use super::ProbeArray;
use crate::gpu::ops::CountedQueue as _;
use crate::gpu::textures::compressed::gpu_bytes;

impl ProbeArray {
    pub(crate) fn gpu_bytes(&self) -> u64 {
        if (self.face, self.mips, self.cubes) == (1, 1, 1) && !self.table.holds_any() {
            return 0;
        }
        gpu_bytes(&self.texture)
    }

    pub(super) fn release_if_empty(&mut self, device: &wgpu::Device) -> bool {
        if self.table.holds_any() || (self.face, self.mips, self.cubes) == (1, 1, 1) {
            return false;
        }
        self.table.clear();
        (self.texture, self.view) = stand_in(device);
        (self.face, self.mips, self.cubes) = (1, 1, 1);
        true
    }
}

/// `Rgba16Float`, as a BC texture cannot be smaller than its block.
pub(super) fn stand_in(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    array_texture(device, 1, 1, 1, wgpu::TextureFormat::Rgba16Float)
}

pub(super) fn array_texture(
    device: &wgpu::Device,
    face: u32,
    mips: u32,
    cubes: usize,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Reflection Probe Array"),
        size: wgpu::Extent3d {
            width: face,
            height: face,
            depth_or_array_layers: 6 * cubes as u32,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Reflection Probe Array"),
        dimension: Some(wgpu::TextureViewDimension::CubeArray),
        ..Default::default()
    });
    (texture, view)
}

pub(super) fn copy_cube(
    encoder: &mut wgpu::CommandEncoder,
    cube: &CubemapTexture,
    array: &wgpu::Texture,
    slot: u32,
    shift: u32,
) {
    for level in 0..cube.texture.mip_level_count() {
        let size = block_extent(cube.texture.format(), (cube.size >> level).max(1));
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &cube.texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: array,
                mip_level: level + shift,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: 6 * slot,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
        );
    }
}

pub(super) fn copy_into(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    array: &wgpu::Texture,
    copies: &[(CubemapTexture, usize, u32)],
) {
    if copies.is_empty() {
        return;
    }
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Reflection Probe Copies"),
    });
    for (cube, slot, shift) in copies {
        copy_cube(&mut encoder, cube, array, *slot as u32, *shift);
    }
    queue.counted_submit([encoder.finish()]);
}

#[cfg(test)]
#[path = "bytes_tests.rs"]
mod bytes_tests;
