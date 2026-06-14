//! src/components/script.rs — Script component
//!
//! script path + load state. Unity: MonoBehaviour reference. Moved verbatim from
//! the legacy `core/scene.rs`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptComponent {
    pub path: String,
    pub is_loaded: bool,
}
