//! `Debug.DrawLine` / `Debug.DrawRay`'s lines (#1882): what a script asked to see,
//! held for a number of fixed ticks and drawn by the renderer's gizmo pass.
//!
//! **Draw-only.** Nothing in the sim reads them back, so they never change what a
//! run does; they are kept here, on the scene, only because the renderer reads the
//! scene. A duration counts ticks (`ticks`), never the clock: a recorded run draws
//! the same lines on the same frames. The tick's head ages them
//! (`app::play::age_debug_lines`), so a one-tick line drawn by `Update` is still
//! there when the frame after that tick renders, and gone at the next tick.
//! Outside Play no tick passes: a line drawn from the console in edit mode stays
//! until Play, Stop or another scene opened in edit mode (Play's own state,
//! cleared by Stop; an edit-mode load drops them with the World they were drawn
//! over, while a Play load leaves them to age out).

use std::collections::VecDeque;

use glam::Vec3;

/// The most lines held at once. A script that draws past it (a ray per particle,
/// every tick, for seconds) drops its oldest lines, so the store and the frame's
/// upload stay bounded.
pub const MAX_DEBUG_LINES: usize = 16_384;

/// One line: its two ends in world space, its colour, and the ticks it has left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DebugLine {
    pub from: Vec3,
    pub to: Vec3,
    pub color: [f32; 4],
    /// Ticks still to start with this line drawn; at least 1 when pushed.
    pub ticks: u32,
}

/// The scene's debug lines, oldest first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DebugLines {
    lines: VecDeque<DebugLine>,
}

impl DebugLines {
    /// Hold `from`..`to` in `color` for `ticks` ticks (at least one). A non-finite
    /// end or colour is dropped: it would draw nothing, or across the whole view.
    pub fn push(&mut self, from: Vec3, to: Vec3, color: [f32; 4], ticks: u32) {
        let finite = from.is_finite() && to.is_finite() && color.iter().all(|c| c.is_finite());
        if !finite {
            return;
        }
        if self.lines.len() >= MAX_DEBUG_LINES {
            self.lines.pop_front();
        }
        let ticks = ticks.max(1);
        self.lines.push_back(DebugLine {
            from,
            to,
            color,
            ticks,
        });
    }

    /// A tick starts: every line has one tick fewer, and those with none left go.
    pub fn tick(&mut self) {
        self.lines.retain_mut(|line| {
            line.ticks -= 1;
            line.ticks > 0
        });
    }

    /// The lines held now, oldest first.
    pub fn lines(&self) -> impl ExactSizeIterator<Item = &DebugLine> {
        self.lines.iter()
    }

    /// Whether no line is held.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// The ticks a `duration` in seconds of sim time lasts at the fixed step `dt`: at
/// least one (a duration of 0, or none, is one frame, as Unity's), rounded up, and
/// a non-finite or negative duration is one tick too.
pub fn duration_ticks(duration: f32, dt: f32) -> u32 {
    if !(duration > 0.0 && duration.is_finite() && dt > 0.0) {
        return 1;
    }
    // Saturates on a huge duration; a hair under a whole tick count stays it.
    let ticks = (duration / dt - 1e-4).ceil();
    (ticks as u32).max(1)
}

#[cfg(test)]
#[path = "debug_lines_tests.rs"]
mod tests;
