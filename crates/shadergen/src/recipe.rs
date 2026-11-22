use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum PassKind {
        Surface = "surface",
        Postfx = "postfx",
        Ui = "ui",
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Scalar(f32),
    Vector(Vec<f32>),
    Name(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockSel {
    pub id: String,
    #[serde(default)]
    pub params: BTreeMap<String, ParamValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShaderRecipe {
    pub pass: PassKind,
    pub name: String,
    #[serde(default)]
    pub blocks: Vec<BlockSel>,
}

impl ShaderRecipe {
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_tag_is_snake_case() {
        let json = serde_json::to_string(&PassKind::Postfx).unwrap();
        assert_eq!(json, "\"postfx\"");
    }

    #[test]
    fn untagged_param_parses_scalar_or_array() {
        let s: ParamValue = serde_json::from_str("0.5").unwrap();
        assert_eq!(s, ParamValue::Scalar(0.5));
        let v: ParamValue = serde_json::from_str("[1.0, 0.0, 0.0]").unwrap();
        assert_eq!(v, ParamValue::Vector(vec![1.0, 0.0, 0.0]));
    }
}
