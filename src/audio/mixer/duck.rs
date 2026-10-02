//! src/audio/mixer/duck.rs — ducking: one group lowers another while it plays (#465).
//!
//! Unity's "Duck Volume", cut down to what a game needs: while the **trigger**
//! group (or any group under it) has a live voice, the **target** group's gain
//! ramps down to `volume` over `attack` seconds; once the trigger falls silent it
//! ramps back up over `release` seconds. Voice lines duck music; a flashbang's ring
//! ducks the world. Stepped by sim time, so a replay ducks identically.

use serde::{Deserialize, Serialize};

use super::GroupId;

/// One duck rule plus its envelope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Duck {
    pub trigger: GroupId,
    pub target: GroupId,
    /// The target's gain at full duck, linear in `[0, 1]`.
    pub volume: f32,
    /// Seconds to reach full duck once the trigger plays.
    pub attack: f32,
    /// Seconds to recover once the trigger is silent.
    pub release: f32,
    /// How far ducked right now: `0` untouched, `1` fully ducked.
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

    /// The gain this rule applies to its target right now.
    pub fn gain(&self) -> f32 {
        1.0 + (self.volume - 1.0) * self.level
    }
}
