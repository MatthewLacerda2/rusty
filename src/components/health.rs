//! src/components/health.rs — Health component
//!
//! current/max/dead. Moved verbatim from the legacy `core/scene.rs`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthComponent {
    pub current_health: f32,
    pub max_health: f32,
    pub is_dead: bool,
}
