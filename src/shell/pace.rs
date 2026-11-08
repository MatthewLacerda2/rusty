//! src/shell/pace.rs — when the window draws its next frame (#1721).
//!
//! A running game draws as fast as it can. The editor outside Play has nothing moving
//! on its own, so it draws **on demand**, as Unity's Scene and Game views do: a window
//! event, a [`crate::core::wake`] ring (a command, a file change, a background job) or
//! a deadline egui asked for. Between those the loop sleeps in the OS, at no CPU or
//! GPU cost. A frontend says which it wants ([`Pace`]); [`Pacer`] holds what the loop
//! itself knows, and the decision is free of any window so it can be tested.
//!
//! One trigger draws a few frames, not one. Some of what a frame shows only arrives
//! on a later one: a camera an `eval` moved is synced at the next tick, and a GPU
//! readback lands frames after it was asked for.

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};

use crate::time::MAXIMUM_DELTA_TIME;
use winit::window::Window;

/// Frames drawn after a trigger, so what takes more than one frame to arrive does.
const SETTLE_FRAMES: u8 = 8;

/// How long to wait before retrying a frame the swapchain refused (a hidden window,
/// a GPU timeout), so a window hidden for hours doesn't spin on it.
const RETRY_AFTER: Duration = Duration::from_millis(100);

/// How a frontend wants its frames right now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pace {
    /// Draw as fast as the loop goes: something is moving on its own.
    Continuous,
    /// Draw when something asks; on its own, only at `next`.
    OnDemand { next: Option<Instant> },
}

impl Pace {
    /// The step a frame `raw` seconds after the last one hands the sim's clock. A
    /// frame drawn on demand follows time asleep, not a frame, so it is clamped to
    /// [`MAXIMUM_DELTA_TIME`]; in Play the fixed-tick accumulator clamps
    /// (`frame::ticks`).
    pub fn frame_step(self, raw: f32) -> f32 {
        match self {
            Pace::Continuous => raw,
            Pace::OnDemand { .. } => raw.min(MAXIMUM_DELTA_TIME),
        }
    }
}

/// What the loop does next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Next {
    /// Draw a frame.
    Now,
    /// Sleep until this instant, or until an event comes first.
    At(Instant),
    /// Sleep until an event.
    Idle,
}

/// The triggers the loop has seen and not yet drawn for.
#[derive(Debug)]
pub struct Pacer {
    /// Frames still owed to the last trigger.
    settling: u8,
    /// When a frame the swapchain refused is tried again.
    retry_at: Option<Instant>,
}

impl Default for Pacer {
    /// A new window is owed its first frames.
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

    /// [`Self::activity`], when `changed`: a scene swapped with no input, as a game's
    /// `Application.Quit()` stopping Play does.
    pub(super) fn settle_if(&mut self, changed: bool) {
        if changed {
            self.activity();
        }
    }

    /// A frame reached the screen.
    pub(super) fn drew(&mut self) {
        self.settling = self.settling.saturating_sub(1);
        self.retry_at = None;
    }

    /// The swapchain refused the frame: it is still owed, a little later.
    pub(super) fn skipped(&mut self, now: Instant) {
        self.retry_at = Some(now + RETRY_AFTER);
    }

    /// What to do next, given how the frontend wants its frames.
    pub(super) fn next(&self, pace: Pace, now: Instant) -> Next {
        let Pace::OnDemand { next } = pace else {
            return Next::Now;
        };
        match (self.retry_at, next) {
            // Nothing can reach the screen until then, so nothing else counts.
            (Some(retry), _) if retry > now => Next::At(retry),
            (Some(_), _) => Next::Now,
            _ if self.settling > 0 => Next::Now,
            (None, Some(at)) if at <= now => Next::Now,
            (None, Some(at)) => Next::At(at),
            (None, None) => Next::Idle,
        }
    }
}

/// Act on [`Pacer::next`]: called each time the loop runs out of events.
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

/// Make [`crate::core::wake::frame`] wake this loop, from any thread.
pub(super) fn install_wake(event_loop: &EventLoop<()>) {
    let proxy = event_loop.create_proxy();
    crate::core::wake::install(move || {
        // The loop is gone only while the process exits: nothing left to draw.
        let _ = proxy.send_event(());
    });
}

#[cfg(test)]
#[path = "pace_tests.rs"]
mod pace_tests;
