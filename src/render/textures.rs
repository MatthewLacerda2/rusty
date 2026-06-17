use std::rc::Rc;

use image::GenericImageView;

use super::{GpuTexture, Renderer};

impl Renderer {
    /// Generates a standard checkerboard texture for meshes that don't have texture files assigned
    #[allow(clippy::too_many_lines)] // grandfathered: burn down in #124
    pub(super) fn create_default_checkerboard_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> GpuTexture {
        let width = 64;
        let height = 64;
        let mut pixels = Vec::with_capacity(width * height * 4);

        // Generate glowing dark neon checker pattern
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

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Default Checkerboard Bind Group"),
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

    /// Loads and registers a new texture from a filepath. Caches results dynamically.
    #[allow(clippy::too_many_lines)] // grandfathered: burn down in #124
    pub fn load_texture(&mut self, path_str: &str) -> Rc<GpuTexture> {
        if let Some(tex) = self.gpu_textures.get(path_str) {
            return Rc::clone(tex);
        }

        // Try load texture
        let tex = match image::open(path_str) {
            Ok(img) => {
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

                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                    address_mode_u: wgpu::AddressMode::Repeat,
                    address_mode_v: wgpu::AddressMode::Repeat,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Linear,
                    anisotropy_clamp: 16,
                    ..Default::default()
                });

                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(path_str),
                    layout: &self.texture_layout,
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

                Rc::new(GpuTexture {
                    texture,
                    view,
                    sampler,
                    bind_group,
                })
            }
            Err(_) => {
                // Fallback to default
                Rc::clone(&self.default_texture)
            }
        };

        self.gpu_textures
            .insert(path_str.to_string(), Rc::clone(&tex));
        tex
    }
}
