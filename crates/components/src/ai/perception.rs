//! crates/components/src/ai/perception.rs — AIPerception (#1900).
//!
//! Unreal's `UAIPerceptionComponent` with its *Sight*, *Hearing* and *Damage*
//! senses (Unity has no perception system). The settings are authored and saved;
//! the [`Memory`] of what the agent sensed is runtime state, never saved, so a
//! scene loads (and Play starts) with an agent that knows nothing. The senses
//! are stepped by `ai::perception` on the fixed tick.

use serde::{Deserialize, Serialize};

use super::memory::Memory;

/// A perceiver: what it can sense, and how long it remembers. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AIPerceptionComponent {
    /// Starts seeing a target inside this distance, in metres (`SightRadius`).
    pub sight_range: f32,
    /// Keeps seeing a target it already sees out to this distance, so the edge
    /// does not flicker (`LoseSightRadius`). Never below `sight_range`.
    pub lose_sight_range: f32,
    /// Half-angle of the view cone around the entity's forward (+Z), in degrees
    /// (`PeripheralVisionAngleDegrees`): 0..=180, 180 sees all round.
    pub sight_angle: f32,
    /// Where the sight ray starts, in metres above the entity's origin.
    pub eye_height: f32,
    /// The layers a sight target may be on (one bit per layer).
    pub sight_mask: u32,
    /// The layers whose colliders block a sight ray (one bit per layer).
    pub blocking_mask: u32,
    /// How far a noise of loudness 1 is heard, in metres (`HearingRange`).
    pub hearing_range: f32,
    /// Seconds a seen target is remembered after it leaves sight; 0 = forever.
    pub sight_max_age: f32,
    /// Seconds a heard noise is remembered; 0 = forever.
    pub hearing_max_age: f32,
    /// Seconds a damage stimulus is remembered; 0 = forever.
    pub damage_max_age: f32,
    /// What the agent knows. Runtime state: never saved, empty on load.
    #[serde(skip)]
    pub memory: Memory,
}

/// The `Default` layer's bit: what blocks sight unless the author says otherwise.
pub const DEFAULT_BLOCKING_MASK: u32 = 1;

impl Default for AIPerceptionComponent {
    /// The issue's defaults (#1900), Unreal's ballpark for a shooter: 20 m sight
    /// kept to 25 m, a 60° half-cone, eyes at 1.6 m, 30 m hearing.
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
