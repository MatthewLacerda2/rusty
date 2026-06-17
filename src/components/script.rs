//! src/components/script.rs — Script component
//!
//! script path + load state. Unity: MonoBehaviour reference. Moved verbatim from
//! the legacy `core/scene.rs`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScriptComponent {
    pub path: String,
    pub is_loaded: bool,
    /// Inspector-set overrides for the script's exported `fields` schema (#84),
    /// keyed by field name — the engine's equivalent of Unity's serialized
    /// `[SerializeField]` values. A [`BTreeMap`] (not `HashMap`) so scene
    /// serialization stays byte-identical/deterministic. Empty for scripts with
    /// no schema or fields left at their defaults; merged over the schema
    /// defaults when the script loads at Play.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, ScriptFieldValue>,
}

/// A persisted value for a schema-driven script field (#84). The variants cover
/// the inspector-editable field types; numbers ride as `f64` so a Lua number
/// round-trips without loss.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptFieldValue {
    Number(f64),
    Boolean(bool),
    Text(String),
}
