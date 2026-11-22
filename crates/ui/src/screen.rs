use std::rc::Rc;

use glam::Vec2;

use crate::{TextureSizes, UiView};
use rusty_core::settings::video::VideoSettings;
use rusty_scene::Camera;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScreenSize {
    game_view: Option<(u32, u32)>,
    display_scale: Option<f32>,
    textures: Rc<TextureSizes>,
}

impl ScreenSize {
    /// Record the game view's size in physical pixels (the platform layer, once per frame).
    pub fn set_game_view(&mut self, width: u32, height: u32) {
        self.game_view = Some((width, height));
    }

    /// Put back a size read with [`game_view`](Self::game_view), `None` included:
    /// a demo replay restoring the screen it started on.
    pub fn restore_game_view(&mut self, game_view: Option<(u32, u32)>) {
        self.game_view = game_view;
    }

    pub fn game_view(&self) -> Option<(u32, u32)> {
        self.game_view
    }

    /// Record the display's backing scale, physical pixels per point (the platform layer,
    /// once per frame).
    pub fn set_display_scale(&mut self, scale: f32) {
        self.display_scale = Some(scale);
    }

    /// Put back a [`reported_display_scale`](Self::reported_display_scale), as
    /// [`restore_game_view`](Self::restore_game_view) does.
    pub fn restore_display_scale(&mut self, scale: Option<f32>) {
        self.display_scale = scale;
    }

    /// The display scale as the platform last wrote it; `None` headless.
    pub fn reported_display_scale(&self) -> Option<f32> {
        self.display_scale
    }

    pub fn display_scale(&self) -> f32 {
        self.display_scale.filter(|s| *s > 0.0).unwrap_or(1.0)
    }

    pub fn pixels(&self, video: &VideoSettings) -> Vec2 {
        let (w, h) = self.game_view.unwrap_or_else(|| video.resolution());
        Vec2::new(w.max(1) as f32, h.max(1) as f32)
    }

    /// The textures' native sizes the layout measures Images by (the shared table).
    pub fn textures(&self) -> &Rc<TextureSizes> {
        &self.textures
    }

    pub fn shared_textures_mut(&mut self) -> &mut Rc<TextureSizes> {
        &mut self.textures
    }

    /// The view the UI is computed against: this screen sized against `video`, its texture
    /// sizes, and `camera` when there is one.
    pub fn view(&self, video: &VideoSettings, camera: Option<Camera>) -> UiView {
        let (px, textures) = (self.pixels(video), Rc::clone(&self.textures));
        match camera {
            Some(camera) => UiView::with_camera(px, camera, textures),
            None => UiView::screen(px, textures),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_falls_back_to_the_video_resolution() {
        let video = VideoSettings {
            width: 800,
            height: 600,
            ..VideoSettings::default()
        };
        let mut screen = ScreenSize::default();
        assert_eq!(screen.pixels(&video), Vec2::new(800.0, 600.0));
        screen.set_game_view(1920, 0);
        assert_eq!(screen.pixels(&video), Vec2::new(1920.0, 1.0));
    }

    #[test]
    fn display_scale_is_one_until_a_platform_reports_one() {
        let mut screen = ScreenSize::default();
        assert_eq!(screen.display_scale(), 1.0);
        screen.set_display_scale(2.0);
        assert_eq!(screen.display_scale(), 2.0);
        for bad in [0.0, -1.0, f32::NAN] {
            screen.set_display_scale(bad);
            assert_eq!(screen.display_scale(), 1.0, "{bad}");
        }
        screen.restore_display_scale(None);
        assert_eq!(screen.reported_display_scale(), None);
    }
}
