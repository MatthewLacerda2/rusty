use serde::{Deserialize, Serialize};

use super::GroupId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Duck {
    pub trigger: GroupId,
    pub target: GroupId,
    /// The target's gain at full duck, linear in `[0, 1]`.
    pub volume: f32,
    pub attack: f32,
    pub release: f32,
    pub level: f32,
}

impl Duck {
    pub fn new(trigger: GroupId, target: GroupId, volume: f32, attack: f32, release: f32) -> Self {
        Self {
            trigger,
            target,
            volume: volume.clamp(0.0, 1.0),
            attack: attack.max(0.0),
            release: release.max(0.0),
            level: 0.0,
        }
    }

    /// Advance the envelope by `dt` seconds with the trigger `active` or not.
    pub fn step(&mut self, active: bool, dt: f32) {
        let (goal, seconds) = if active {
            (1.0, self.attack)
        } else {
            (0.0, self.release)
        };
        if seconds <= 0.0 {
            self.level = goal;
            return;
        }
        let step = dt / seconds;
        self.level = if goal > self.level {
            (self.level + step).min(goal)
        } else {
            (self.level - step).max(goal)
        };
    }

    pub fn gain(&self) -> f32 {
        1.0 + (self.volume - 1.0) * self.level
    }
}
