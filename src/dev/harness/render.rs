//! The harness's rendering verbs: `Screenshot` (a frame to a PNG, after up to
//! `settle` frames drawn first, #2003: none when the view reads no history,
//! #2092) and `Render` (a frame drawn only for what it costs, #835). Both draw
//! through the run's one [`CaptureHost`](crate::dev::capture::CaptureHost) and
//! fold the frame's cost (render counters, `renderer_ms`, GPU time where the
//! adapter measures it) into the run's frame stats.
//!
//! **Size (#1583).** A run renders at its size, a Game view resolution: 1280x720
//! unless `play --size WxH` set another ([`Harness::set_size`]), which the sim's UI
//! layout reads too. One `Screenshot` may name its own size, as Unity's
//! `ScreenCapture.CaptureScreenshot(path, superSize)` does; the run keeps its size.
//! Every target, post effect and UI layout of a frame is sized to the frame: the
//! renderer lays the UI out for the view it draws (`render::ui::draw`).

use std::path::Path;

use super::Harness;
use crate::dev::capture::RenderedFrame;
use crate::dev::screenshot::{DEFAULT_HEIGHT, DEFAULT_WIDTH};
use crate::dev::stats::record_render;

impl Harness {
    /// The run's render size (see the module docs): the Game view the sim lays
    /// the UI out on, so a demo replay that sets the recorded one (#1581) renders
    /// at it too; 1280x720 until one is set.
    pub fn size(&self) -> (u32, u32) {
        let world = self.world.borrow();
        let view = world.resources.screen.borrow().game_view();
        let (width, height) = view.unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT));
        (width.max(1), height.max(1))
    }

    /// Set the run's render size, as a Game view of `width` x `height` pixels: every
    /// later frame draws at it and the sim's UI lays out on it, as the window's does.
    pub fn set_size(&mut self, width: u32, height: u32) {
        let world = self.world.borrow();
        let mut screen = world.resources.screen.borrow_mut();
        screen.set_game_view(width.max(1), height.max(1));
    }

    /// Render the current scene/camera offscreen at the run's size and write a PNG
    /// to `path`; see [`screenshot_at`](Self::screenshot_at).
    pub fn screenshot(&mut self, path: impl AsRef<Path>) -> bool {
        self.screenshot_at(path, self.size())
    }

    /// Render the current scene/camera offscreen at `size` and write a PNG to `path`.
    /// The run's own size is unchanged.
    ///
    /// Returns `true` if a frame was captured, `false` if no GPU/software adapter
    /// is available (skipped gracefully — never panics). Logs the outcome.
    pub fn screenshot_at(&mut self, path: impl AsRef<Path>, size: (u32, u32)) -> bool {
        self.screenshot_with(path, size, false)
    }

    /// [`screenshot_at`](Self::screenshot_at), with the colliders and the
    /// `Debug.Draw*` lines drawn over the frame when `gizmos` (#1882), as the Game
    /// view's **Gizmos** toggle draws them. Without, the frame is the plain one.
    pub fn screenshot_with(
        &mut self,
        path: impl AsRef<Path>,
        size: (u32, u32),
        gizmos: bool,
    ) -> bool {
        let path = path.as_ref().to_path_buf();
        self.capture.gizmos = gizmos;
        let result = crate::dev::screenshot::capture_world_into(
            &mut self.capture,
            &self.world.borrow(),
            &path,
            size.0,
            size.1,
        );
        self.capture.gizmos = false;
        if let (Ok(true), Some(frame)) = (&result, self.capture.last_frame.take()) {
            record_render(&mut self.stats.borrow_mut(), &frame);
            self.publish_exposure();
        }
        self.report_shot(result, &path, size)
    }

    /// [`screenshot_with`](Self::screenshot_with) after up to `settle` frames drawn
    /// with no sim step (#2003), so the temporal effects (TAA above all) converge on
    /// the view first. None are drawn when the first one reads no history an earlier
    /// frame left (#2092, `history_passes`): every later frame would be the same
    /// image, so that first frame is the shot, drawn again only for the gizmos.
    /// The shot is the same image a full settle gives, at one render instead of
    /// `settle + 1`.
    pub fn screenshot_settled(
        &mut self,
        path: impl AsRef<Path>,
        size: (u32, u32),
        (gizmos, settle): (bool, u32),
    ) -> bool {
        if settle == 0 {
            return self.screenshot_with(path, size, gizmos);
        }
        let Some(first) = self.render_frame_at(size) else {
            return false;
        };
        if first.counters.history_passes > 0 {
            if !self.settle(size, settle - 1) {
                return false;
            }
        } else if !gizmos {
            let path = path.as_ref();
            let result = self.capture.write_png(path).map(|()| true);
            return self.report_shot(result, path, size);
        }
        self.screenshot_with(path, size, gizmos)
    }

    /// Log how a shot went; whether it was written.
    fn report_shot(&mut self, result: Result<bool, String>, path: &Path, size: (u32, u32)) -> bool {
        match result {
            Ok(true) => {
                self.log(format!(
                    "Screenshot written ({}x{}): {}",
                    size.0,
                    size.1,
                    path.display()
                ));
                true
            }
            Ok(false) => {
                self.log(format!(
                    "Screenshot skipped (no GPU adapter): {}",
                    path.display()
                ));
                false
            }
            Err(e) => {
                self.console.borrow_mut().error(format!("[Harness] {e}"));
                false
            }
        }
    }

    /// Hand the frame's adapted exposure to `Debug.GetAdaptedExposure` (#1444).
    fn publish_exposure(&self) {
        let world = self.world.borrow();
        world.scene().borrow_mut().screen_exposure = self.capture.exposure;
    }

    /// The renderer the harness draws with (built on first use at the run's size),
    /// for a caller that queues GPU work of its own (the bench's clock hold,
    /// #862). `None` without a GPU or software adapter.
    pub fn renderer(&mut self) -> Option<&mut crate::render::Renderer> {
        let (width, height) = self.size();
        self.capture.renderer(width, height)
    }

    /// Render the current scene/camera offscreen at the run's size, writing
    /// nothing, and fold the frame's cost into the stats. `None` when no GPU or
    /// software adapter is available (a skip, as for a screenshot) or on an error,
    /// which is logged.
    pub fn render_frame(&mut self) -> Option<RenderedFrame> {
        self.render_frame_at(self.size())
    }

    /// Render `frames` frames at `size` with no sim step between them, writing
    /// nothing (#2003): a screenshot's `settle`, so temporal effects (TAA above all)
    /// converge on the view before the shot. Each frame's cost lands in the stats,
    /// as `Render`'s does. `false` when one did not draw (no adapter, an error).
    pub fn settle(&mut self, size: (u32, u32), frames: u32) -> bool {
        (0..frames).all(|_| self.render_frame_at(size).is_some())
    }

    /// [`render_frame`](Self::render_frame) at `size`.
    fn render_frame_at(&mut self, (width, height): (u32, u32)) -> Option<RenderedFrame> {
        let result = {
            let world = self.world.borrow();
            crate::dev::screenshot::share_world(&mut self.capture, &world, width, height);
            let camera = world.main_view();
            let scene = world.scene().borrow();
            self.capture.draw(&scene, &camera, width, height)
        };
        match result {
            Ok(frame) => {
                let frame = frame?;
                record_render(&mut self.stats.borrow_mut(), &frame);
                self.publish_exposure();
                Some(frame)
            }
            Err(e) => {
                self.console.borrow_mut().error(format!("[Harness] {e}"));
                None
            }
        }
    }
}
