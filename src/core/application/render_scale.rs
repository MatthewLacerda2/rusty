//! The project's render-scale default (#1715), `project.rusty`'s `"graphics"` block:
//! what runs while the player has chosen no render scale. The Build Settings window
//! and `Graphics.SetDefaultRenderScale` set it through one op.

use super::Application;
use crate::core::settings::render_scale::{RenderScaleDefault, RenderScaleSetting};

impl Application {
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
}
