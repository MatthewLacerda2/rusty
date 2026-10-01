use std::rc::Rc;

use image::GenericImageView;

use crate::render::{GpuTexture, Renderer};

/// The storage format of every image the renderer uploads: colour is authored in
/// sRGB, so the colour view decodes it on sample.
const IMAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// The format an uploaded image's data view reinterprets it as: the same bytes, no
/// decode — a data map stores its values raw (glTF 2.0, `Texture.Bake`) (#647).
const DATA_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl Renderer {
    /// Generates a standard checkerboard texture for meshes that don't have texture files assigned
    pub(crate) fn create_default_checkerboard_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> GpuTexture {
        let pixels = Self::checkerboard_pixels(64, 64);
        let label = "Default Checkerboard Texture";
        Self::create_pixel_texture(device, queue, layout, (64, &pixels), label)
    }

    /// A 1×1 white texture: what an effect with no texture of its own samples, so
    /// its colour is its tint alone (#441's ribbons).
    pub(crate) fn create_white_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> GpuTexture {
        Self::create_pixel_texture(device, queue, layout, (1, &[255; 4]), "White Texture")
    }

    /// Upload a square `(side, rgba8 pixels)` sRGB texture with a repeating,
    /// nearest-filtered sampler.
    fn create_pixel_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        (side, pixels): (u32, &[u8]),
        label: &str,
    ) -> GpuTexture {
        let size = wgpu::Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: IMAGE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[DATA_FORMAT],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * side),
                rows_per_image: Some(side),
            },
            size,
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

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

    /// Builds the RGBA8 pixel buffer for the glowing dark neon checker pattern.
    fn checkerboard_pixels(width: usize, height: usize) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                let is_even = ((x / 8) + (y / 8)) % 2 == 0;
                let (r, g, b) = if is_even {
                    (28, 30, 42) // Dark violet grey
                } else {
                    (52, 45, 78) // Glowing neon violet
                };
                pixels.push(r);
                pixels.push(g);
                pixels.push(b);
                pixels.push(255);
            }
        }
        pixels
    }

    /// Loads and registers a new texture from a filepath. Caches results dynamically.
    pub fn load_texture(&mut self, path_str: &str) -> Rc<GpuTexture> {
        if let Some(tex) = self.gpu_textures.get(path_str) {
            return Rc::clone(tex);
        }
        // A render texture (#430) is never on disk: it is registered by the frame
        // that draws it, and until then shows the default — uncached, so it appears
        // the moment its camera exists.
        if crate::render::render_texture::is_render_texture(path_str) {
            return Rc::clone(&self.default_texture);
        }

        // Try load texture, falling back to the default on failure.
        let tex = match image::open(path_str) {
            Ok(img) => self.upload_image_texture(path_str, &img),
            Err(_) => Rc::clone(&self.default_texture),
        };

        self.gpu_textures
            .insert(path_str.to_string(), Rc::clone(&tex));
        tex
    }

    /// Uploads a decoded image to the GPU and wraps it in a cached `GpuTexture`.
    fn upload_image_texture(&self, path_str: &str, img: &image::DynamicImage) -> Rc<GpuTexture> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(path_str),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: IMAGE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[DATA_FORMAT],
        });

        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });

        Rc::new(Self::finalize_texture(
            &self.device,
            &self.texture_layout,
            (texture, DATA_FORMAT),
            sampler,
            Some(path_str),
        ))
    }
}
