//! src/ui/screen.rs — `ScreenSize`: the screen the UI is laid out on (#417).
//!
//! The layout is a pure function of (scene, screen size), so the screen size is a
//! **sim input**, like the seed and the inputs: same (seed, inputs, dt, screen size)
//! ⇒ same layout. The windowed platform writes the game view's pixel size here every
//! frame; a headless run never writes it and falls back to the `VideoSettings`
//! resolution, so the harness lays out exactly as a window of that size would.
//!
//! The textures' native sizes ride along (#1221): an `Image` measures by its
//! texture's size, which the tick reads from the files and writes here
//! ([`TextureSizes`]) before it steps, so the layout never reads a file. One `Rc` is shared with
//! every view and the renderer; a write copies it only while one is held.

use std::rc::Rc;

use glam::Vec2;

use crate::core::settings::video::VideoSettings;
use crate::scene::Camera;
use crate::ui::{TextureSizes, UiView};

/// The UI's target surface size (a resource). See the module docs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScreenSize {
    /// The game view's size in physical pixels, as the platform last reported it.
    /// `None` until a platform writes one (always, headless).
    game_view: Option<(u32, u32)>,
    /// The native size of each texture the scene's Images use, as the platform
    /// last read them.
    textures: Rc<TextureSizes>,
}

impl ScreenSize {
    /// Record the game view's pixel size (the platform layer, once per frame).
    pub fn set_game_view(&mut self, width: u32, height: u32) {
        self.game_view = Some((width, height));
    }

    /// Put back a size read with [`game_view`](Self::game_view), `None` included:
    /// a demo replay restoring the screen it started on (#1598).
    pub fn restore_game_view(&mut self, game_view: Option<(u32, u32)>) {
        self.game_view = game_view;
    }

    /// The game view's pixel size as the platform last wrote it; `None` headless
    /// until a run sets one.
    pub fn game_view(&self) -> Option<(u32, u32)> {
        self.game_view
    }

    /// The screen size in pixels the UI lays out on: the game view when a platform
    /// reported one, else `video`'s resolution. Each axis is at least 1.
    pub fn pixels(&self, video: &VideoSettings) -> Vec2 {
        let (w, h) = self.game_view.unwrap_or_else(|| video.resolution());
        Vec2::new(w.max(1) as f32, h.max(1) as f32)
    }

    /// The textures' native sizes the layout measures Images by (the shared table).
    pub fn textures(&self) -> &Rc<TextureSizes> {
        &self.textures
    }

    /// The shared table's handle, for the tick's refresh to update in place
    /// (`ui::TextureSizeReader`): it copies the table only when it changes it.
    pub fn shared_textures_mut(&mut self) -> &mut Rc<TextureSizes> {
        &mut self.textures
    }

    /// The view the UI is computed against: this screen (sized against `video`)
    /// and its texture sizes, through `camera` when there is one.
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
}
