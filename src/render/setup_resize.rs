//! Depth-resource creation, framebuffer resize, and quality-tier switching.
//! Split out of `setup.rs` to keep both files under the size cap; behaviour is
//! the original `resize` plus the new post-FX-aware reallocation.

use super::postfx::QualityPreset;
use super::Renderer;

impl Renderer {
    /// Switch the scalability tier. Reallocates the bloom buffers when the new
    /// preset changes their resolution divisor; cheap no-op if unchanged.
    pub fn set_quality(&mut self, preset: QualityPreset) {
        if self.quality == preset {
            return;
        }
        let old_divisor = self.quality.bloom_divisor();
        self.quality = preset;
        let new_divisor = preset.bloom_divisor();
        if old_divisor != new_divisor {
            self.post_fx.resize(
                &self.device,
                self.config.width,
                self.config.height,
                new_divisor,
            );
        }
    }

    pub(super) fn create_depth_resources(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            if let Some(surface) = &self.surface {
                surface.configure(&self.device, &self.config);
            }
            let (depth_tex, depth_view) = Self::create_depth_resources(&self.device, &self.config);
            self.depth_texture = depth_tex;
            self.depth_view = depth_view;
            self.post_fx.resize(
                &self.device,
                self.config.width,
                self.config.height,
                self.quality.bloom_divisor(),
            );
        }
    }
}
