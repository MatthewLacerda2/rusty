use std::path::Path;

use super::Harness;
use crate::capture::RenderedFrame;
use crate::screenshot::{DEFAULT_HEIGHT, DEFAULT_WIDTH};
use crate::stats::record_render;

impl Harness {
    pub fn size(&self) -> (u32, u32) {
        let world = self.world.borrow();
        let view = world.resources.screen.borrow().game_view();
        let (width, height) = view.unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT));
        (width.max(1), height.max(1))
    }

    pub fn set_size(&mut self, width: u32, height: u32) {
        let world = self.world.borrow();
        let mut screen = world.resources.screen.borrow_mut();
        screen.set_game_view(width.max(1), height.max(1));
    }

    pub fn screenshot(&mut self, path: impl AsRef<Path>) -> bool {
        self.screenshot_at(path, self.size())
    }

    pub fn screenshot_at(&mut self, path: impl AsRef<Path>, size: (u32, u32)) -> bool {
        self.screenshot_with(path, size, false)
    }

    pub fn screenshot_with(
        &mut self,
        path: impl AsRef<Path>,
        size: (u32, u32),
        gizmos: bool,
    ) -> bool {
        let path = path.as_ref().to_path_buf();
        self.capture.gizmos = gizmos;
        let result = crate::screenshot::capture_world_into(
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

    fn publish_exposure(&self) {
        let world = self.world.borrow();
        world.scene().borrow_mut().screen_exposure = self.capture.exposure;
    }

    pub fn renderer(&mut self) -> Option<&mut rusty_render::Renderer> {
        let (width, height) = self.size();
        self.capture.renderer(width, height)
    }

    pub fn idle_at(&self) -> Option<std::time::Instant> {
        self.capture.idle_at
    }

    pub fn render_frame(&mut self) -> Option<RenderedFrame> {
        self.render_frame_at(self.size())
    }

    pub fn settle(&mut self, size: (u32, u32), frames: u32) -> bool {
        (0..frames).all(|_| self.render_frame_at(size).is_some())
    }

    fn render_frame_at(&mut self, (width, height): (u32, u32)) -> Option<RenderedFrame> {
        let result = {
            let world = self.world.borrow();
            crate::screenshot::share_world(&mut self.capture, &world, width, height);
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
