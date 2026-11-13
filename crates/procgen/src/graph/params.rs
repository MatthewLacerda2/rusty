//! crates/procgen/src/graph/params.rs — a graph's declared params (#1744): each one's type,
//! default and range, and the check every value an instance gives must pass.
//!
//! Strict, as every recipe key is (#395): a value of the wrong type, outside
//! `min`/`max`, or naming no declared param is refused by name. Blender clamps a
//! node group's input; rusty refuses, since a typo should never bake.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One declared param: `{ type, default, min?, max? }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamDecl {
    #[serde(rename = "type")]
    pub ty: ParamType,
    pub default: Value,
    /// The least value a `number` or `integer` param takes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// The greatest value a `number` or `integer` param takes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// What a param's values are. A `color` is a linear RGB(A) array or an sRGB
/// `"#rrggbb"` string, as a node's colours are; a `text` fills an enum or a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamType {
    Number,
    Integer,
    Bool,
    Color,
    Text,
}

impl ParamType {
    fn name(self) -> &'static str {
        match self {
            ParamType::Number => "number",
            ParamType::Integer => "integer",
            ParamType::Bool => "bool",
            ParamType::Color => "color",
            ParamType::Text => "text",
        }
    }

    fn fits(self, value: &Value) -> bool {
        match self {
            ParamType::Number => value.as_f64().is_some_and(f64::is_finite),
            ParamType::Integer => value.is_i64() || value.is_u64(),
            ParamType::Bool => value.is_boolean(),
            ParamType::Color => match value {
                Value::String(s) => s.starts_with('#'),
                Value::Array(a) => matches!(a.len(), 3 | 4) && a.iter().all(Value::is_number),
                _ => false,
            },
            ParamType::Text => value.is_string(),
        }
    }
}

impl ParamDecl {
    /// Refuse `value` for param `name` when it is not of this type or lies outside
    /// `min`/`max`.
    pub fn check(&self, name: &str, value: &Value) -> Result<(), String> {
        let ty = self.ty.name();
        if !self.ty.fits(value) {
            return Err(format!("param {name:?} takes a {ty}, got {value}"));
        }
        let Some(n) = value.as_f64() else {
            return Ok(());
        };
        if self.min.is_some_and(|min| n < min) || self.max.is_some_and(|max| n > max) {
            let min = self.min.map_or("-inf".into(), |m| m.to_string());
            let max = self.max.map_or("inf".into(), |m| m.to_string());
            return Err(format!("param {name:?} is {n}, outside {min} to {max}"));
        }
        Ok(())
    }

    /// Refuse a declaration whose range does not fit its type or whose default
    /// fails its own check.
    pub(super) fn check_self(&self, name: &str) -> Result<(), String> {
        let ranged = matches!(self.ty, ParamType::Number | ParamType::Integer);
        if !ranged && (self.min.is_some() || self.max.is_some()) {
            return Err(format!(
                "param {name:?}: a {} takes no min or max",
                self.ty.name()
            ));
        }
        self.check(name, &self.default)
            .map_err(|e| format!("{e} as its default"))
    }
}

/// The values a graph's params take under an instance giving `given`: each
/// default, overridden by the given value once it passes its check. `graph` names
/// the file in the errors. Each declaration is checked first (its range fits its
/// type, its default passes its own check).
pub fn values(
    params: &BTreeMap<String, ParamDecl>,
    given: &BTreeMap<String, Value>,
    graph: &str,
) -> Result<BTreeMap<String, Value>, String> {
    for (name, d) in params {
        d.check_self(name)
            .map_err(|e| format!("graph {graph}: {e}"))?;
    }
    let mut values: BTreeMap<String, Value> = params
        .iter()
        .map(|(name, d)| (name.clone(), d.default.clone()))
        .collect();
    for (name, value) in given {
        let decl = params.get(name).ok_or_else(|| {
            let takes = match params.is_empty() {
                true => "no params".to_string(),
                false => params.keys().cloned().collect::<Vec<_>>().join(", "),
            };
            format!("graph {graph} has no param {name:?}; it takes {takes}")
        })?;
        decl.check(name, value)
            .map_err(|e| format!("graph {graph}: {e}"))?;
        values.insert(name.clone(), value.clone());
    }
    Ok(values)
}
