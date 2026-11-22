use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScriptComponent {
    pub path: String,
    pub is_loaded: bool,
    /// Inspector overrides of the script's `fields` schema, merged over its defaults at Play.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, ScriptFieldValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptFieldValue {
    /// `f64`, so a Lua number round-trips without loss.
    Number(f64),
    Boolean(bool),
    Text(String),
}
