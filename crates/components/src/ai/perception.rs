//! Unreal's `UAIPerceptionComponent` (Sight, Hearing, Damage); Unity has no perception system.
//! Settings are saved; the [`Memory`] is runtime state, never saved.

use serde::{Deserialize, Serialize};

use super::memory::Memory;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AIPerceptionComponent {
    /// Metres (`SightRadius`).
    pub sight_range: f32,
    /// Metres; hysteresis so the sight edge does not flicker (`LoseSightRadius`). Never below `sight_range`.
    pub lose_sight_range: f32,
    /// Half-angle around forward (+Z), degrees, 0..=180 (`PeripheralVisionAngleDegrees`).
    pub sight_angle: f32,
    /// Metres above the entity's origin.
    pub eye_height: f32,
    pub sight_mask: u32,
    pub blocking_mask: u32,
    /// Metres a noise of loudness 1 is heard (`HearingRange`).
    pub hearing_range: f32,
    /// Seconds remembered; 0 = forever.
    pub sight_max_age: f32,
    pub hearing_max_age: f32,
    pub damage_max_age: f32,
    #[serde(skip)]
    pub memory: Memory,
}

pub const DEFAULT_BLOCKING_MASK: u32 = 1;

impl Default for AIPerceptionComponent {
    /// Unreal's ballpark for a shooter.
    fn default() -> Self {
        Self {
            sight_range: 20.0,
            lose_sight_range: 25.0,
            sight_angle: 60.0,
            eye_height: 1.6,
            sight_mask: u32::MAX,
            blocking_mask: DEFAULT_BLOCKING_MASK,
            hearing_range: 30.0,
            sight_max_age: 5.0,
            hearing_max_age: 3.0,
            damage_max_age: 5.0,
            memory: Memory::default(),
        }
    }
}

impl crate::FirstClass for AIPerceptionComponent {
    const NAME: &'static str = "AIPerception";
    const ALIASES: &'static [&'static str] = &["perception"];
}
