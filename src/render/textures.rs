use std::rc::Rc;

use image::GenericImageView;

use super::{GpuTexture, Renderer};

impl Renderer {
    /// Generates a standard checkerboard texture for meshes that don't have texture files assigned
    pub(super) fn create_default_checkerboard_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> GpuTexture {
        let width = 64;
        let height = 64;
        let pixels = Self::checkerboard_pixels(width, height);

        let size = wgpu::Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default Checkerboard Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width as u32),
                rows_per_image: Some(height as u32),
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

        Self::finalize_texture(
            device,
            layout,
            texture,
            sampler,
            Some("Default Checkerboard Bind Group"),
        )
    }

    /// Create the texture view + bind group and assemble the final `GpuTexture`.
    fn finalize_texture(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture: wgpu::Texture,
        sampler: wgpu::Sampler,
        label: Option<&str>,
    ) -> GpuTexture {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
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
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
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
            texture,
            sampler,
            Some(path_str),
        ))
    }
}
