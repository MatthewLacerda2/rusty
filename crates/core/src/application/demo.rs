//! The demo timeline's mailbox (#1598): what a script asks of the editor's demo
//! timeline, and where the editor says the open demo stands. The timeline is the
//! editor window's, so the request waits here for the frame boundary, as
//! `Editor.EnterPlay` waits for the play transition.

use super::Application;

/// What a script asks of the editor's demo timeline, as its own controls do.
#[derive(Clone, Debug, PartialEq)]
pub enum DemoRequest {
    /// Open the demo file at this path.
    Open(String),
    /// Replay to this tick.
    Seek(u64),
    /// Pause (`true`) or play (`false`) the replay.
    Pause(bool),
}

/// The mailbox: requests in, the open demo's place out.
#[derive(Default)]
pub struct DemoDesk {
    viewer: bool,
    requests: Vec<DemoRequest>,
    tick: Option<(u64, u64)>,
}

impl Application {
    /// Mark the host as one that shows a demo on a timeline (the editor window).
    pub fn mark_demo_viewer(&mut self) {
        self.demo.viewer = true;
    }

    /// Whether the host shows demos on a timeline.
    pub fn demo_viewer(&self) -> bool {
        self.demo.viewer
    }

    /// Ask the host's demo timeline for something, at the next frame boundary.
    pub fn request_demo(&mut self, request: DemoRequest) {
        self.demo.requests.push(request);
    }

    /// Consume the pending demo requests, oldest first.
    pub fn take_demo_requests(&mut self) -> Vec<DemoRequest> {
        std::mem::take(&mut self.demo.requests)
    }

    /// The host says where the open demo stands (`None`: none open).
    pub fn set_demo_tick(&mut self, tick: Option<(u64, u64)>) {
        self.demo.tick = tick;
    }

    /// The open demo's `(tick, of)`, as the host last drew it.
    pub fn demo_tick(&self) -> Option<(u64, u64)> {
        self.demo.tick
    }
}
