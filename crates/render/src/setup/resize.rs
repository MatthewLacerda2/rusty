use crate::{PixelSize, Renderer};
use rusty_core::settings::quality::QualityValues;
use rusty_core::settings::render_scale::RenderScale;
use rusty_core::settings::GraphicsSettings;

impl Renderer {
    pub fn set_quality(&mut self, values: QualityValues) {
        self.quality = values;
    }

    pub fn apply_graphics(&mut self, graphics: GraphicsSettings, display_scale: f32) {
        self.set_quality(graphics.quality);
        self.graphics_options = graphics.options;
        self.render_scale = graphics.render_scale.resolve(display_scale);
    }

    pub fn render_scale(&self, editor_mode: bool) -> RenderScale {
        match editor_mode {
            true => RenderScale::NATIVE,
            false => self.render_scale,
        }
    }

    pub(crate) fn graphics(&self, editor_mode: bool) -> GraphicsSettings {
        let player = match editor_mode {
            true => GraphicsSettings::default(),
            false => GraphicsSettings {
                options: self.graphics_options,
                ..Default::default()
            },
        };
        GraphicsSettings {
            quality: self.quality,
            options: self.switches.gate(player.options),
            ..player
        }
    }

    fn supports_no_vsync(&self) -> bool {
        self.present_modes.contains(&wgpu::PresentMode::Immediate)
    }

    /// Returns whether vsync is on now: off is refused, keeping `Fifo`, when the surface lacks
    /// `Immediate`.
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

    pub fn vsync(&self) -> bool {
        self.config.present_mode == wgpu::PresentMode::Fifo
    }

    pub fn resize(&mut self, new_size: PixelSize) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            if let Some(surface) = &self.surface {
                surface.configure(&self.device, &self.config);
            }
        }
    }

    pub fn reconfigure_surface(&mut self) {
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
    }
}

pub(super) fn surface_config(
    caps: &wgpu::SurfaceCapabilities,
    size: PixelSize,
) -> wgpu::SurfaceConfiguration {
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or_else(|| {
            log::warn!("[Render] no sRGB surface format; the window will not match screenshots");
            caps.formats[0]
        });
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
        color_space: wgpu::SurfaceColorSpace::Auto,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    }
}
