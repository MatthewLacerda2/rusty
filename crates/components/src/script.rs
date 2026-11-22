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
    /// `f64`: a Lua float round-trips exactly; an integer only up to 2^53.
    Number(f64),
    Boolean(bool),
    Text(String),
}
