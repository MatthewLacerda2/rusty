pub(crate) mod freshness;
pub(crate) mod kind_watch;
pub(crate) mod mips;
mod panorama;
mod sampler;

use std::rc::Rc;

use crate::asset::sidecar::{FilterMode, TextureSettings};
use crate::asset::MapRef;
use crate::render::{GpuTexture, Renderer};
use image::GenericImageView;
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

        GpuTexture {
            texture,
            view,
            data_view,
            sampler,
            filter,
            bind_group,
        }
    }

    /// Loads and registers the texture `map` names. Caches results dynamically,
    /// keyed by the map; a failed load is remembered as a miss (drawn with the
    /// default) and retried once the file appears, never cached under its path (#689).
    /// A colour texture; `load_texture_as` names what the texels hold.
    pub fn load_texture(&mut self, map: &MapRef) -> Rc<GpuTexture> {
        self.load_texture_as(map, TextureKind::Colour)
    }

    /// [`Self::load_texture`] for a texture holding `kind`, which decides how its
    /// mips average. The first load of a file decides it: the cache keeps one
    /// upload per file, and a later request of the other kind warns once
    /// ([`kind_watch::KindWatch`]).
    pub(crate) fn load_texture_as(&mut self, map: &MapRef, kind: TextureKind) -> Rc<GpuTexture> {
        let path_str = map.as_str();
        if let Some(tex) = self.gpu_textures.get(path_str) {
            self.texture_kinds.requested(path_str, kind, "material");
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

        let Some(tex) = self.upload_map(map, kind) else {
            self.texture_freshness.miss(path_str);
            return Rc::clone(&self.default_texture);
        };
        self.texture_kinds.uploaded(path_str, kind);
        self.gpu_textures.insert(map.clone(), Rc::clone(&tex));
        tex
    }

    /// Decode the image `map` names and upload it, uncached, with the mips and
    /// sampler its import settings ask for; `None` when it cannot be read. A cache
    /// with its own keys (bloom's lens dirt, #1442) loads through this.
    pub(crate) fn upload_map(&self, map: &MapRef, kind: TextureKind) -> Option<Rc<GpuTexture>> {
        let img = open(map)?;
        let import = import_settings(map);
        Some(self.upload_image_texture(map.as_str(), &img, &import, kind))
    }

    /// Uploads a decoded image holding `kind`, with the mips and the sampler its
    /// import settings ask for (#1323), and wraps it in a cached `GpuTexture`.
    fn upload_image_texture(
        &self,
        path_str: &str,
        img: &image::DynamicImage,
        import: &TextureSettings,
        kind: TextureKind,
    ) -> Rc<GpuTexture> {
        let settings = MipSettings::for_map(import, kind);
        let (width, height) = img.dimensions();
        let levels = settings.levels(width, height);
        let gpu = (&self.device, &self.queue);
        let texture = upload(gpu, path_str, &img.to_rgba8(), levels);
        let kind = settings.kind;
        self.mip_generator
            .generate(&self.device, &self.queue, &texture, kind);
        Rc::new(Self::finalize_texture(
            &self.device,
            &self.texture_layout,
            (texture, DATA_FORMAT),
            (sampler(&self.device, import), import.filter_mode),
            Some(path_str),
        ))
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

/// The import settings of the image `map` names (#1323), from the sidecar beside
/// the file it resolves to; the defaults when it has none.
fn import_settings(map: &MapRef) -> TextureSettings {
    let file = map.file();
    file.map(|f| crate::asset::sidecar::texture(&f))
        .unwrap_or_default()
}

/// A `side`² dark neon checker of `cell`-texel squares.
fn checkerboard(side: u32, cell: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(side, side, |x, y| match (x / cell + y / cell) % 2 {
        0 => image::Rgba([28, 30, 42, 255]), // Dark violet grey
        _ => image::Rgba([52, 45, 78, 255]), // Glowing neon violet
    })
}

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
