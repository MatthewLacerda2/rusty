use std::collections::VecDeque;

use glam::Vec3;

pub const MAX_DEBUG_LINES: usize = 16_384;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DebugLine {
    pub from: Vec3,
    pub to: Vec3,
    pub color: [f32; 4],
    pub ticks: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DebugLines {
    lines: VecDeque<DebugLine>,
}

impl DebugLines {
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

    pub fn tick(&mut self) {
        self.lines.retain_mut(|line| {
            line.ticks -= 1;
            line.ticks > 0
        });
    }

    pub fn lines(&self) -> impl ExactSizeIterator<Item = &DebugLine> {
        self.lines.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

pub fn duration_ticks(duration: f32, dt: f32) -> u32 {
    if !(duration > 0.0 && duration.is_finite() && dt > 0.0) {
        return 1;
    }
    let ticks = (duration / dt - 1e-4).ceil();
    (ticks as u32).max(1)
}

#[cfg(test)]
#[path = "debug_lines_tests.rs"]
mod tests;
