//! src/ui/screen.rs — `ScreenSize`: the screen the UI is laid out on (#417).
//!
//! The layout is a pure function of (scene, screen size), so the screen size is a
//! **sim input**, like the seed and the inputs: same (seed, inputs, dt, screen size)
//! ⇒ same layout. The windowed platform writes the game view's pixel size here every
//! frame; a headless run never writes it and falls back to the `VideoSettings`
//! resolution, so the harness lays out exactly as a window of that size would.

use glam::Vec2;

use crate::core::video::VideoSettings;

/// The UI's target surface size (a resource). See the module docs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScreenSize {
    /// The game view's size in physical pixels, as the platform last reported it.
    /// `None` until a platform writes one (always, headless).
    game_view: Option<(u32, u32)>,
}

impl ScreenSize {
    /// Record the game view's pixel size (the platform layer, once per frame).
    pub fn set_game_view(&mut self, width: u32, height: u32) {
        self.game_view = Some((width, height));
    }

    /// The screen size in pixels the UI lays out on: the game view when a platform
    /// reported one, else `video`'s resolution. Each axis is at least 1.
    pub fn pixels(&self, video: &VideoSettings) -> Vec2 {
        let (w, h) = self.game_view.unwrap_or_else(|| video.resolution());
        Vec2::new(w.max(1) as f32, h.max(1) as f32)
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
}
