//! The harness's rendering verbs: `Screenshot` (a frame to a PNG) and `Render`
//! (a frame drawn only for what it costs, #835). Both draw through the run's one
//! [`CaptureHost`](crate::dev::capture::CaptureHost) and fold the frame's cost —
//! render counters, `renderer_ms`, GPU time where the adapter measures it — into
//! the run's frame stats.

use std::path::Path;

use super::Harness;
use crate::dev::capture::RenderedFrame;
use crate::dev::screenshot::{DEFAULT_HEIGHT, DEFAULT_WIDTH};
use crate::dev::stats::record_render;

impl Harness {
    /// Render the current scene/camera offscreen and write a PNG to `path`.
    ///
    /// Returns `true` if a frame was captured, `false` if no GPU/software adapter
    /// is available (skipped gracefully — never panics). Logs the outcome.
    pub fn screenshot(&mut self, path: impl AsRef<Path>) -> bool {
        let path = path.as_ref().to_path_buf();
        let result = crate::dev::screenshot::capture_world_into(
            &mut self.capture,
            &self.world.borrow(),
            &path,
            crate::dev::screenshot::DEFAULT_WIDTH,
            crate::dev::screenshot::DEFAULT_HEIGHT,
        );
        if let (Ok(true), Some(frame)) = (&result, self.capture.last_frame.take()) {
            record_render(&mut self.stats.borrow_mut(), &frame);
        }
        match result {
            Ok(true) => {
                self.log(format!("Screenshot written: {}", path.display()));
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

    /// Render the current scene/camera offscreen at the screenshot size, writing
    /// nothing, and fold the frame's cost into the stats. `None` when no GPU or
    /// software adapter is available (a skip, as for a screenshot) or on an error,
    /// which is logged.
    pub fn render_frame(&mut self) -> Option<RenderedFrame> {
        let result = {
            let world = self.world.borrow();
            let scene = world.scene().borrow();
            let camera = world.camera().borrow();
            self.capture
                .draw(&scene, &camera, DEFAULT_WIDTH, DEFAULT_HEIGHT)
        };
        match result {
            Ok(frame) => {
                let frame = frame?;
                record_render(&mut self.stats.borrow_mut(), &frame);
                Some(frame)
            }
            Err(e) => {
                self.console.borrow_mut().error(format!("[Harness] {e}"));
                None
            }
        }
    }
}
