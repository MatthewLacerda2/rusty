use super::Application;
use crate::settings::render_scale::{RenderScaleDefault, RenderScaleSetting};

impl Application {
    pub fn render_scale_default(&self) -> RenderScaleDefault {
        self.graphics.render_scale
    }

    /// Also sets `live.default`, so a player with no chosen scale gets it at once.
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
