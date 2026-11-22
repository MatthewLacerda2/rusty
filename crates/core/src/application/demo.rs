//! The timeline is the editor window's, so a request waits here for the frame
//! boundary, as `Editor.EnterPlay` waits for the play transition.

use super::Application;

#[derive(Clone, Debug, PartialEq)]
pub enum DemoRequest {
    Open(String),
    Seek(u64),
    Pause(bool),
}

#[derive(Default)]
pub struct DemoDesk {
    viewer: bool,
    requests: Vec<DemoRequest>,
    tick: Option<(u64, u64)>,
}

impl Application {
    pub fn mark_demo_viewer(&mut self) {
        self.demo.viewer = true;
    }

    pub fn demo_viewer(&self) -> bool {
        self.demo.viewer
    }

    pub fn request_demo(&mut self, request: DemoRequest) {
        self.demo.requests.push(request);
    }

    /// Oldest first.
    pub fn take_demo_requests(&mut self) -> Vec<DemoRequest> {
        std::mem::take(&mut self.demo.requests)
    }

    /// `None`: no demo open.
    pub fn set_demo_tick(&mut self, tick: Option<(u64, u64)>) {
        self.demo.tick = tick;
    }

    /// `(tick, of)`.
    pub fn demo_tick(&self) -> Option<(u64, u64)> {
        self.demo.tick
    }
}
