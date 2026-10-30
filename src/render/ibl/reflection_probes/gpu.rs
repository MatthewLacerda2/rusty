//! The probe array's GPU side (#1206): allocating the cube-array texture and copying
//! a baked cube into one of its layers.

use super::super::cubemap::CubemapTexture;

/// A cube-array texture of `cubes` black cubes, `face` texels square with `mips`
/// levels, and its `CubeArray` view.
pub(super) fn array_texture(
    device: &wgpu::Device,
    face: u32,
    mips: u32,
    cubes: usize,
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
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
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

/// Record the copy of every mip of `cube` into array layer `slot`, `shift` mips down.
pub(super) fn copy_cube(
    encoder: &mut wgpu::CommandEncoder,
    cube: &CubemapTexture,
    array: &wgpu::Texture,
    slot: u32,
    shift: u32,
) {
    for level in 0..cube.texture.mip_level_count() {
        let size = (cube.size >> level).max(1);
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

/// Copy each `(cube, slot, shift)` into its array layer, in one submission; nothing
/// to copy submits nothing.
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
    queue.submit([encoder.finish()]);
}
