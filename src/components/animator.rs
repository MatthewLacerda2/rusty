//! src/components/animator.rs — Animator component
//!
//! clip/time/speed/playing. Unity: Animator. Moved verbatim from the legacy
//! `core/scene.rs`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimatorComponent {
    pub current_clip: String,
    pub time: f32,
    pub speed: f32,
    pub is_playing: bool,
    pub freeze: bool,
}
