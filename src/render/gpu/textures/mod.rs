pub(crate) mod freshness;
pub(crate) mod mips;
mod panorama;

use std::rc::Rc;

use crate::asset::MapRef;
use crate::render::{GpuTexture, Renderer};
pub(crate) use mips::{MipSettings, TextureKind};

/// The storage format of every image the renderer uploads: colour is authored in
/// sRGB, so the colour view decodes it on sample.
const IMAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// The format an uploaded image's data view reinterprets it as: the same bytes, no
/// decode — a data map stores its values raw (glTF 2.0, `Texture.Bake`) (#647).
const DATA_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl Renderer {
    /// Generates a standard checkerboard texture for meshes that don't have texture
    /// files assigned. Its sampler is the one every material map samples through
    /// (`material_bind_group`), so it is the image textures' trilinear one; 32-texel
    /// cells keep the checker's edges crisp under its linear magnification.
    pub(crate) fn create_default_checkerboard_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> GpuTexture {
        let label = "Default Checkerboard Texture";
        Self::create_pixel_texture(device, queue, layout, checkerboard(256, 32), label)
    }

    /// A 1×1 white texture: what an effect with no texture of its own samples, so
    /// its colour is its tint alone (#441's ribbons).
    pub(crate) fn create_white_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> GpuTexture {
        let white = image::RgbaImage::from_pixel(1, 1, image::Rgba([255; 4]));
        Self::create_pixel_texture(device, queue, layout, white, "White Texture")
    }

    /// Upload a built-in sRGB `image`, with its mips, and the image textures'
    /// sampler.
    fn create_pixel_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        image: image::RgbaImage,
        label: &str,
    ) -> GpuTexture {
        let colour = MipSettings {
            generate: true,
            kind: TextureKind::Colour,
        };
        let texture = upload(device, queue, label, mips::chain(image, colour));
        let sampler = image_sampler(device);
        Self::finalize_texture(device, layout, (texture, DATA_FORMAT), sampler, Some(label))
    }

    /// Create the colour and data views + bind group and assemble the final
    /// `GpuTexture`. `data_format` is the format the data view reads the texels as:
    /// the storage format itself for a texture that holds linear values, or a
    /// format listed in the texture's `view_formats`.
    pub(crate) fn finalize_texture(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        (texture, data_format): (wgpu::Texture, wgpu::TextureFormat),
        sampler: wgpu::Sampler,
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

        GpuTexture {
            texture,
            view,
            data_view,
            sampler,
            bind_group,
        }
    }

    /// Loads and registers the texture `map` names. Caches results dynamically,
    /// keyed by the map; a failed load is remembered as a miss (drawn with the
    /// default) and retried once the file appears, never cached under its path (#689).
    /// A colour texture; [`Self::load_texture_as`] names what the texels hold.
    pub fn load_texture(&mut self, map: &MapRef) -> Rc<GpuTexture> {
        self.load_texture_as(map, TextureKind::Colour)
    }

    /// [`Self::load_texture`] for a texture holding `kind`, which decides how its
    /// mips average. The first load of a file decides it: the cache keeps one
    /// upload per file.
    pub(crate) fn load_texture_as(&mut self, map: &MapRef, kind: TextureKind) -> Rc<GpuTexture> {
        let path_str = map.as_str();
        if let Some(tex) = self.gpu_textures.get(path_str) {
            return Rc::clone(tex);
        }
        if self.texture_freshness.is_miss(path_str) {
            return Rc::clone(&self.default_texture);
        }
        // A render texture (#430) is never on disk: it is registered by the frame
        // that draws it, and until then shows the default — uncached, so it appears
        // the moment its camera exists.
        if crate::render::render_texture::is_render_texture(path_str) {
            return Rc::clone(&self.default_texture);
        }

        let Some(img) = open(map) else {
            self.texture_freshness.miss(path_str);
            return Rc::clone(&self.default_texture);
        };
        let settings = MipSettings::for_map(map, kind);
        let tex = self.upload_image_texture(path_str, &img, settings);
        self.gpu_textures.insert(map.clone(), Rc::clone(&tex));
        tex
    }

    /// Uploads a decoded image, with the mips `settings` ask for, and wraps it in a
    /// cached `GpuTexture`.
    fn upload_image_texture(
        &self,
        path_str: &str,
        img: &image::DynamicImage,
        settings: MipSettings,
    ) -> Rc<GpuTexture> {
        let levels = mips::chain(img.to_rgba8(), settings);
        let texture = upload(&self.device, &self.queue, path_str, levels);
        Rc::new(Self::finalize_texture(
            &self.device,
            &self.texture_layout,
            (texture, DATA_FORMAT),
            image_sampler(&self.device),
            Some(path_str),
        ))
    }
}

/// Create an sRGB texture holding `levels` (the base, then each mip) and write
/// every level.
fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    levels: Vec<image::RgbaImage>,
) -> wgpu::Texture {
    let (width, height) = levels[0].dimensions();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: extent(width, height),
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: IMAGE_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[DATA_FORMAT],
    });
    for (mip_level, level) in (0..).zip(&levels) {
        let (w, h) = level.dimensions();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            level,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            extent(w, h),
        );
    }
    texture
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

/// Every image texture's sampler: repeating, trilinear (Unity's default for a
/// texture with mips), anisotropic.
fn image_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Image Sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        anisotropy_clamp: 16,
        ..Default::default()
    })
}

/// A `side`² dark neon checker of `cell`-texel squares.
fn checkerboard(side: u32, cell: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(side, side, |x, y| match (x / cell + y / cell) % 2 {
        0 => image::Rgba([28, 30, 42, 255]), // Dark violet grey
        _ => image::Rgba([52, 45, 78, 255]), // Glowing neon violet
    })
}

#[cfg(test)]
mod formats_tests;
#[cfg(test)]
mod freshness_tests;
#[cfg(test)]
mod mips_gpu_tests;

/// The image `map` names, decoded: a map that references a texture a conversion
/// extracted is read where the import cache holds it ([`MapRef::file`], #1130).
/// `None` when it cannot be read. Takes a [`MapRef`], never a bare string, so no
/// reader opens a reference as if it were a path (#1269).
pub(crate) fn open(map: &MapRef) -> Option<image::DynamicImage> {
    image::open(map.file()?).ok()
}
