//! The renderer's runtime framebuffer concern, distinct from one-time construction
//! (`setup.rs` / `setup_build.rs`): depth-resource creation, surface resize, vsync
//! toggling, and quality-tier switching — everything that reacts to the window or
//! settings changing *after* the renderer exists.

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

    /// Whether the surface can turn vsync OFF — i.e. advertises `Immediate`. `Fifo`
    /// (vsync on) is mandated by wgpu on every backend, so vsync can always be
    /// turned back on; only the off direction is gated.
    pub fn supports_no_vsync(&self) -> bool {
        self.present_modes.contains(&wgpu::PresentMode::Immediate)
    }

    /// Toggle vsync by reconfiguring the surface's present mode: `Fifo` (vsync on,
    /// no tearing, always supported) vs `Immediate` (vsync off, lowest latency, may
    /// tear). Returns the vsync state actually in effect — a request to turn vsync
    /// off is refused (and `Fifo` kept) when the surface lacks `Immediate`, so this
    /// never feeds wgpu an unsupported mode. Cheap no-op when unchanged. Headless
    /// (no surface) just keeps the config field in sync.
    pub fn set_vsync(&mut self, vsync: bool) -> bool {
        let want_off = !vsync && self.supports_no_vsync();
        let mode = if want_off {
            wgpu::PresentMode::Immediate
        } else {
            wgpu::PresentMode::Fifo
        };
        if self.config.present_mode != mode {
            self.config.present_mode = mode;
            if let Some(surface) = &self.surface {
                surface.configure(&self.device, &self.config);
            }
        }
        self.vsync()
    }

    /// Whether vsync is currently on (`Fifo` present mode).
    pub fn vsync(&self) -> bool {
        self.config.present_mode == wgpu::PresentMode::Fifo
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
            // The cached decal depth bind group references the old depth view (#210).
            self.decal_depth_bind_group = None;
            self.post_fx.resize(
                &self.device,
                self.config.width,
                self.config.height,
                self.quality.bloom_divisor(),
            );
        }
    }
}
