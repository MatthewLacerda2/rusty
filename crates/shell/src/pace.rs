use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};

use rusty_time::MAXIMUM_DELTA_TIME;
use winit::window::Window;

/// Frames drawn after one trigger, since some of what a frame shows arrives later (a GPU
/// readback lands frames after its request).
const SETTLE_FRAMES: u8 = 8;

/// The wait before retrying a frame the swapchain refused, so a window hidden for hours
/// does not spin.
const RETRY_AFTER: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pace {
    Continuous,
    OnDemand { next: Option<Instant> },
}

impl Pace {
    /// An on-demand frame follows time asleep, not a frame, so its step is clamped.
    pub fn frame_step(self, raw: f32) -> f32 {
        match self {
            Pace::Continuous => raw,
            Pace::OnDemand { .. } => raw.min(MAXIMUM_DELTA_TIME),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Next {
    Now,
    At(Instant),
    Idle,
}

#[derive(Debug)]
pub struct Pacer {
    settling: u8,
    retry_at: Option<Instant>,
}

impl Default for Pacer {
    fn default() -> Self {
        Self {
            settling: SETTLE_FRAMES,
            retry_at: None,
        }
    }
}

impl Pacer {
    /// Something happened that can change what is shown.
    pub(super) fn activity(&mut self) {
        self.settling = SETTLE_FRAMES;
    }

    /// Settle as after input when `changed`: a scene swap with no input (a game's
    /// `Application.Quit()` stopping Play).
    pub(super) fn settle_if(&mut self, changed: bool) {
        if changed {
            self.activity();
        }
    }

    pub(super) fn drew(&mut self) {
        self.settling = self.settling.saturating_sub(1);
        self.retry_at = None;
    }

    /// The swapchain refused the frame: still owed, a little later.
    pub(super) fn skipped(&mut self, now: Instant) {
        self.retry_at = Some(now + RETRY_AFTER);
    }

    pub(super) fn next(&self, pace: Pace, now: Instant) -> Next {
        let Pace::OnDemand { next } = pace else {
            return Next::Now;
        };
        match (self.retry_at, next) {
            // Nothing reaches the screen before the retry, so nothing else counts.
            (Some(retry), _) if retry > now => Next::At(retry),
            (Some(_), _) => Next::Now,
            _ if self.settling > 0 => Next::Now,
            (None, Some(at)) if at <= now => Next::Now,
            (None, Some(at)) => Next::At(at),
            (None, None) => Next::Idle,
        }
    }
}

pub(super) fn schedule(pacer: &Pacer, pace: Pace, window: &Arc<Window>, loop_: &ActiveEventLoop) {
    loop_.set_control_flow(match pacer.next(pace, Instant::now()) {
        Next::Now => {
            window.request_redraw();
            ControlFlow::Poll
        }
        Next::At(at) => ControlFlow::WaitUntil(at),
        Next::Idle => ControlFlow::Wait,
    });
}

pub(super) fn install_wake(event_loop: &EventLoop<()>) {
    let proxy = event_loop.create_proxy();
    rusty_core::wake::install(move || {
        let _ = proxy.send_event(());
    });
}

#[cfg(test)]
#[path = "pace_tests.rs"]
mod pace_tests;
