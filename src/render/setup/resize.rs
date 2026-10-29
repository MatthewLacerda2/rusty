//! The renderer's runtime framebuffer concern, distinct from one-time construction
//! (`setup.rs` / `setup_build.rs`): depth-resource creation, surface resize, vsync
//! toggling, and quality-preset switching — everything that reacts to the window or
//! settings changing *after* the renderer exists.

use crate::core::settings::quality::QualityValues;
use crate::core::settings::GraphicsSettings;
use crate::render::{PixelSize, Renderer};

impl Renderer {
    /// Switch to a quality preset's values (#1522). Each effect reads its level's
    /// value from them as it runs, and bloom re-sizes its buffers when its divisor
    /// changes, so setting the values is all that happens here (#355).
    pub fn set_quality(&mut self, values: QualityValues) {
        self.quality = values;
    }

    /// Adopt the preset's values, the player's options (#1438) and the render scale (#1498) the
    /// shared cell carries.
    pub fn apply_graphics(&mut self, graphics: GraphicsSettings) {
        self.set_quality(graphics.quality);
        self.graphics_options = graphics.options;
        self.render_scale = graphics.render_scale;
    }

    /// What shapes a view's post-FX: the preset's values, and the player's options and render
    /// scale except in the editor's Scene view (`editor_mode`), which ignores them as
    /// Unity's Scene view ignores PlayerPrefs and the URP asset's render scale. The
    /// Game view and the player honour all of them.
    pub(crate) fn graphics(&self, editor_mode: bool) -> GraphicsSettings {
        let player = match editor_mode {
            true => GraphicsSettings::default(),
            false => GraphicsSettings {
                options: self.graphics_options,
                render_scale: self.render_scale,
                ..Default::default()
            },
        };
        GraphicsSettings {
            quality: self.quality,
            ..player
        }
    }

    /// Whether the surface can turn vsync OFF — i.e. advertises `Immediate`. `Fifo`
    /// (vsync on) is mandated by wgpu on every backend, so vsync can always be
    /// turned back on; only the off direction is gated.
    fn supports_no_vsync(&self) -> bool {
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

    /// Reconfigure the window surface/swapchain to `new_size`. This only touches the
    /// surface (egui paints its panels over the swapchain); the scene renders into
    /// per-view offscreen targets whose sizes each [`crate::render::RenderView`] owns,
    /// so a surface resize never disturbs a view's targets (#355). No-op headless / on
    /// a zero-size request.
    pub fn resize(&mut self, new_size: PixelSize) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            if let Some(surface) = &self.surface {
                surface.configure(&self.device, &self.config);
            }
        }
    }

    /// Re-apply the current surface configuration — the surface-loss recovery path,
    /// which reconfigures to the swapchain's own size (never a view's size, #355).
    pub fn reconfigure_surface(&mut self) {
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
    }
}
