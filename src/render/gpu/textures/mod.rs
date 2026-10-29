pub(crate) mod compressed;
pub(crate) mod freshness;
pub(crate) mod kind_watch;
mod load;
pub(crate) mod mips;
mod panorama;
mod sampler;

use crate::asset::sidecar::{FilterMode, TextureSettings};
use crate::asset::MapRef;
use crate::render::{GpuTexture, Renderer};
use mips::full_chain_len;
pub(crate) use mips::{MipGenerator, MipSettings, TextureKind};
pub(crate) use sampler::{sampler, ui_sampler};

/// The storage format of every image the renderer uploads: colour is authored in
/// sRGB, so the colour view decodes it on sample.
const IMAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// The format an uploaded image's data view reinterprets it as: the same bytes, no
/// decode — a data map stores its values raw (glTF 2.0, `Texture.Bake`) (#647).
const DATA_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl Renderer {
    /// Generates a standard checkerboard texture for meshes that don't have texture
    /// files assigned. Its sampler is an image texture's default one (trilinear,
    /// repeating); 32-texel cells keep the checker's edges crisp under its linear
    /// magnification.
    pub(crate) fn create_default_checkerboard_texture(
        (device, queue, mips): Uploader,
        layout: &wgpu::BindGroupLayout,
    ) -> GpuTexture {
        let label = "Default Checkerboard Texture";
        let checker = checkerboard(256, 32);
        let texture = upload((device, queue), label, &checker, full_chain_len(256, 256));
        mips.generate(device, queue, &texture, TextureKind::Colour);
        let sampler = default_sampler(device);
        Self::finalize_texture(device, layout, (texture, DATA_FORMAT), sampler, Some(label))
    }

    /// A 1×1 white texture: what an effect with no texture of its own samples, so
    /// its colour is its tint alone (#441's ribbons).
    pub(crate) fn create_white_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> GpuTexture {
        let label = "White Texture";
        let white = image::RgbaImage::from_pixel(1, 1, image::Rgba([255; 4]));
        let texture = upload((device, queue), label, &white, 1);
        let sampler = default_sampler(device);
        Self::finalize_texture(device, layout, (texture, DATA_FORMAT), sampler, Some(label))
    }

    /// Create the colour and data views + bind group and assemble the final
    /// `GpuTexture`. `data_format` is the format the data view reads the texels as:
    /// the storage format itself for a texture that holds linear values, or a
    /// format listed in the texture's `view_formats`; `filter` is the one the
    /// sampler reads with.
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        }
    }
}

/// An image texture's default sampler (trilinear, repeating), with its filter.
fn default_sampler(device: &wgpu::Device) -> (wgpu::Sampler, FilterMode) {
    let settings = TextureSettings::default();
    (sampler(device, &settings), settings.filter_mode)
}

/// What uploading an image with mips needs of the GPU: the device, the queue and
/// the mip pass.
pub(crate) type Uploader<'a> = (&'a wgpu::Device, &'a wgpu::Queue, &'a MipGenerator);

/// Create an sRGB texture of `levels` mips with `image` written to its base; the
/// caller draws the rest ([`MipGenerator::generate`]).
pub(super) fn upload(
    (device, queue): (&wgpu::Device, &wgpu::Queue),
    label: &str,
    image: &image::RgbaImage,
    levels: u32,
) -> wgpu::Texture {
    let (width, height) = image.dimensions();
    let mut usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
    if cfg!(test) {
        // Tests read the mips back.
        usage |= wgpu::TextureUsages::COPY_SRC;
    }
    if levels > 1 {
        // The mip pass draws each level.
        usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: extent(width, height),
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: IMAGE_FORMAT,
        usage,
        view_formats: &[DATA_FORMAT],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        image,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        extent(width, height),
    );
    texture
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

/// A `side`² dark neon checker of `cell`-texel squares.
fn checkerboard(side: u32, cell: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(side, side, |x, y| match (x / cell + y / cell) % 2 {
        0 => image::Rgba([28, 30, 42, 255]), // Dark violet grey
        _ => image::Rgba([52, 45, 78, 255]), // Glowing neon violet
    })
}

#[cfg(test)]
mod compressed_gpu_tests;
#[cfg(test)]
mod data_map_tests;
#[cfg(test)]
mod formats_tests;
#[cfg(test)]
mod freshness_tests;
#[cfg(test)]
mod panorama_tests;

/// The image `map` names, decoded: a map that references a texture a conversion
/// extracted is read where the import cache holds it ([`MapRef::file`], #1130).
/// `None` when it cannot be read. Takes a [`MapRef`], never a bare string, so no
/// reader opens a reference as if it were a path (#1269).
pub(crate) fn open(map: &MapRef) -> Option<image::DynamicImage> {
    image::open(map.file()?).ok()
}
