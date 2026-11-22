pub(crate) mod compressed;
pub(crate) mod deep;
pub(crate) mod freshness;
pub(crate) mod kind_watch;
mod load;
pub(crate) mod mips;
mod panorama;
mod sampler;
mod uncached;

use crate::gpu::ops::{CountedDevice as _, CountedQueue as _, Writes};
use crate::Renderer;
pub(crate) use freshness::texture_written;
pub(crate) use load::{decode, Decoded};
use mips::full_chain_len;
pub(crate) use mips::{MipGenerator, MipSettings, TextureKind};
use rusty_asset::sidecar::{FilterMode, TextureSettings};
use rusty_asset::texture_import::variance::VarianceTable;
use rusty_asset::MapRef;
pub(crate) use sampler::{sampler, ui_sampler};

const IMAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DATA_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub data_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub filter: rusty_asset::sidecar::FilterMode,
    pub bind_group: wgpu::BindGroup,
    pub source_size: (u32, u32),
    pub single_channel: bool,
    pub normal_variance: Option<VarianceTable>,
    pub deep_normal: bool,
}

impl Renderer {
    pub(crate) fn create_default_checkerboard_texture(
        (device, writes, mips): Uploader,
        layout: &wgpu::BindGroupLayout,
    ) -> GpuTexture {
        let label = "Default Checkerboard Texture";
        let checker = checkerboard(256, 32);
        let texture = upload(
            (device, writes),
            label,
            &checker,
            (full_chain_len(256, 256), true),
        );
        mips.generate(device, writes.0, &texture, TextureKind::Colour);
        let sampler = default_sampler(device);
        Self::finalize_texture(device, layout, (texture, DATA_FORMAT), sampler, Some(label))
    }

    pub(crate) fn create_soft_disc_texture(
        (device, writes, mips): Uploader,
        layout: &wgpu::BindGroupLayout,
    ) -> GpuTexture {
        let label = "Default Particle Texture";
        let side = SOFT_DISC_SIDE;
        let disc = soft_disc(side);
        let levels = (full_chain_len(side, side), true);
        let texture = upload((device, writes), label, &disc, levels);
        mips.generate(device, writes.0, &texture, TextureKind::Colour);
        let sampler = default_sampler(device);
        Self::finalize_texture(device, layout, (texture, DATA_FORMAT), sampler, Some(label))
    }

    pub(crate) fn create_white_texture(
        (device, writes): (&wgpu::Device, Writes),
        layout: &wgpu::BindGroupLayout,
    ) -> GpuTexture {
        let label = "White Texture";
        let white = image::RgbaImage::from_pixel(1, 1, image::Rgba([255; 4]));
        let texture = upload((device, writes), label, &white, (1, true));
        let sampler = default_sampler(device);
        Self::finalize_texture(device, layout, (texture, DATA_FORMAT), sampler, Some(label))
    }

    pub(crate) fn finalize_texture(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        (texture, data_format): (wgpu::Texture, wgpu::TextureFormat),
        (sampler, filter): (wgpu::Sampler, FilterMode),
        label: Option<&str>,
    ) -> GpuTexture {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let data_view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(data_format),
            ..Default::default()
        });
        let bind_group = device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let size = texture.size();
        GpuTexture {
            texture,
            view,
            data_view,
            sampler,
            filter,
            bind_group,
            source_size: (size.width, size.height),
            single_channel: false,
            normal_variance: None,
            deep_normal: false,
        }
    }
}

fn default_sampler(device: &wgpu::Device) -> (wgpu::Sampler, FilterMode) {
    let settings = TextureSettings::default();
    (sampler(device, &settings), settings.filter_mode)
}

pub(crate) type Uploader<'a> = (&'a wgpu::Device, Writes<'a>, &'a MipGenerator);

pub(super) fn upload(
    (device, writes): (&wgpu::Device, Writes),
    label: &str,
    image: &image::RgbaImage,
    (levels, srgb): (u32, bool),
) -> wgpu::Texture {
    let (width, height) = image.dimensions();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: extent(width, height),
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: if srgb { IMAGE_FORMAT } else { DATA_FORMAT },
        usage: usage(levels),
        view_formats: if srgb {
            &[DATA_FORMAT]
        } else {
            &[IMAGE_FORMAT]
        },
    });
    write_base(writes, &texture, image, 4);
    texture
}

fn usage(levels: u32) -> wgpu::TextureUsages {
    let mut usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
    // lint: allow(cfg-test-switch) only render's own unit tests read a loaded texture back
    if cfg!(test) {
        usage |= wgpu::TextureUsages::COPY_SRC;
    }
    if levels > 1 {
        usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    usage
}

fn write_base((queue, staging): Writes, texture: &wgpu::Texture, bytes: &[u8], texel: u32) {
    let size = texture.size();
    queue.counted_write_texture(
        staging,
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(texel * size.width),
            rows_per_image: Some(size.height),
        },
        extent(size.width, size.height),
    );
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

fn checkerboard(side: u32, cell: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(side, side, |x, y| match (x / cell + y / cell) % 2 {
        0 => image::Rgba([28, 30, 42, 255]),
        _ => image::Rgba([52, 45, 78, 255]),
    })
}

const SOFT_DISC_SIDE: u32 = 64;

pub(crate) fn soft_disc(side: u32) -> image::RgbaImage {
    let half = side as f32 * 0.5;
    image::RgbaImage::from_fn(side, side, |x, y| {
        let (dx, dy) = (x as f32 + 0.5 - half, y as f32 + 0.5 - half);
        let t = (1.0 - (dx * dx + dy * dy).sqrt() / half).clamp(0.0, 1.0);
        let alpha = t * t * (3.0 - 2.0 * t);
        image::Rgba([255, 255, 255, (alpha * 255.0).round() as u8])
    })
}

#[cfg(test)]
mod compressed_gpu_tests;
#[cfg(test)]
mod data_map_tests;
#[cfg(test)]
mod deep_gpu_tests;
#[cfg(test)]
mod deep_tests;
#[cfg(test)]
mod extracted_gpu_tests;
#[cfg(test)]
mod formats_tests;
#[cfg(test)]
mod freshness_tests;
#[cfg(test)]
mod normal_variance_gpu_tests;
#[cfg(test)]
mod panorama_tests;
#[cfg(test)]
mod soft_disc_tests;
#[cfg(test)]
mod uncached_gpu_tests;

pub(crate) fn open(map: &MapRef) -> Option<image::DynamicImage> {
    image::open(map.file()?).ok()
}
