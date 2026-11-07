//! The project's render-scale (#1715) and upscaler (#1848) defaults,
//! `project.rusty`'s `"graphics"` block: what runs while the player has chosen
//! none. The Build Settings window and `Graphics.SetDefaultRenderScale` set the
//! first through one op, `Graphics.SetDefaultUpscaler` the second.

use super::Application;
use crate::core::settings::project_graphics::ProjectGraphics;
use crate::core::settings::render_scale::{RenderScaleDefault, RenderScaleSetting};
use crate::core::settings::upscaler::{Upscaler, UpscalerSetting};

impl Application {
    /// The project's `"graphics"` block: every default a graphics menu may override.
    pub fn project_graphics(&self) -> ProjectGraphics {
        self.graphics
    }

    /// The project's render-scale default.
    pub fn render_scale_default(&self) -> RenderScaleDefault {
        self.graphics.render_scale
    }

    /// Set the project's render-scale default, writing the bound file (if any) over
    /// the file as it stands ([`reread`](Self::reread) first). The
    /// `live` setting takes it too, so it runs at once wherever the player hasn't
    /// chosen a scale (the editor previews what a fresh install sees).
    pub fn set_default_render_scale(
        &mut self,
        default: RenderScaleDefault,
        live: &mut RenderScaleSetting,
    ) -> Result<(), String> {
        self.reread();
        self.graphics.render_scale = default;
        live.default = default;
        self.save()
    }

    /// The project's upscaler default (#1848).
    pub fn upscaler_default(&self) -> Upscaler {
        self.graphics.upscaler
    }

    /// Set the project's upscaler default, as
    /// [`set_default_render_scale`](Self::set_default_render_scale) sets the render
    /// scale's: into the bound file and the `live` setting.
    pub fn set_default_upscaler(
        &mut self,
        default: Upscaler,
        live: &mut UpscalerSetting,
    ) -> Result<(), String> {
        self.reread();
        self.graphics.upscaler = default;
        live.default = default;
        self.save()
    }
}
