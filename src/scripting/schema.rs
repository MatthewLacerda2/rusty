//! src/scripting/schema.rs — schema-driven script fields (#84).
//!
//! A Lua script may `return` an optional `fields` table describing its
//! inspector-editable serialized fields — the engine's equivalent of Unity's
//! `[Range(0,1)]` / `[Tooltip]` / `[Header]` attributes:
//!
//! ```lua
//! return {
//!   fields = {
//!     speed = { type = "number", range = {0, 1}, default = 0.5, tooltip = "u/s" },
//!     name  = "Rusty",                       -- bare value: type inferred, no decoration
//!   },
//!   Start = function() end,
//!   Update = function(dt) end,
//! }
//! ```
//!
//! The schema is parsed in a throwaway sandboxed `Lua` (same approach as
//! `discovery.rs`), so parsing is side-effect-free and determinism-clean — no
//! wall-clock, no RNG. Order within a Lua table is unspecified, so the parsed
//! fields are sorted by name for a stable inspector layout.

use std::collections::BTreeMap;

use mlua::{Lua, Table, Value};

use crate::components::ScriptFieldValue;

/// Inspector-editable type of a script field. Drives which egui control the
/// inspector draws (slider/drag for `Number`, checkbox for `Boolean`, text edit
/// for `Text`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Number,
    Boolean,
    Text,
}

/// One parsed field from a script's `fields` schema: its name, type, optional
/// decorators, and the default the inspector starts from.
#[derive(Clone, Debug)]
pub struct ScriptField {
    pub name: String,
    pub kind: FieldKind,
    /// `[Range(min, max)]` — only meaningful for `Number`; drives a slider.
    pub range: Option<(f64, f64)>,
    /// The default value, used until the inspector overrides it.
    pub default: ScriptFieldValue,
    /// `[Tooltip]` hover text.
    pub tooltip: Option<String>,
    /// `[Header]` section label rendered above the field.
    pub header: Option<String>,
}

/// Parse the optional `fields` schema from a script's source `code`. Returns the
/// declared fields sorted by name; an empty vec if the script has no schema or
/// fails to compile (the inspector then simply shows no field controls).
pub fn parse_fields(code: &str) -> Vec<ScriptField> {
    let lua = Lua::new();
    let Ok(table) = lua.load(code).eval::<Table>() else {
        return Vec::new();
    };
    let Ok(fields) = table.get::<_, Table>("fields") else {
        return Vec::new();
    };
    let mut out: Vec<ScriptField> = fields
        .pairs::<String, Value>()
        .flatten()
        .filter_map(|(name, value)| parse_field(name, value))
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Merge the inspector `overrides` over the schema `defaults` and write the
/// resulting field values onto the live script `table`, so `self.speed` inside
/// the script reads the configured value. Schema defaults fill in any field the
/// inspector left untouched. Pure data assignment — no clock/RNG.
pub fn apply_field_values(
    table: &Table,
    code: &str,
    overrides: &BTreeMap<String, ScriptFieldValue>,
) -> mlua::Result<()> {
    for field in parse_fields(code) {
        let key = field.name.as_str();
        match overrides.get(&field.name).unwrap_or(&field.default) {
            ScriptFieldValue::Number(n) => table.set(key, *n)?,
            ScriptFieldValue::Boolean(b) => table.set(key, *b)?,
            ScriptFieldValue::Text(s) => table.set(key, s.as_str())?,
        }
    }
    Ok(())
}

/// Parse one `name = meta` entry. `meta` is either a metadata table or a bare
/// value (number/bool/string) whose type is inferred and used as the default.
fn parse_field(name: String, meta: Value) -> Option<ScriptField> {
    match meta {
        Value::Table(t) => Some(parse_meta_table(name, &t)),
        Value::Number(n) => Some(scalar(name, FieldKind::Number, ScriptFieldValue::Number(n))),
        Value::Integer(i) => Some(scalar(
            name,
            FieldKind::Number,
            ScriptFieldValue::Number(i as f64),
        )),
        Value::Boolean(b) => Some(scalar(
            name,
            FieldKind::Boolean,
            ScriptFieldValue::Boolean(b),
        )),
        Value::String(s) => Some(scalar(
            name,
            FieldKind::Text,
            ScriptFieldValue::Text(s.to_str().unwrap_or_default().to_owned()),
        )),
        _ => None,
    }
}

/// A bare-value field: inferred type, given default, no decorators.
fn scalar(name: String, kind: FieldKind, default: ScriptFieldValue) -> ScriptField {
    ScriptField {
        name,
        kind,
        range: None,
        default,
        tooltip: None,
        header: None,
    }
}

/// Parse a full `{ type=, range=, default=, tooltip=, header= }` metadata table.
fn parse_meta_table(name: String, t: &Table) -> ScriptField {
    let default_val = t.get::<_, Value>("default").ok();
    let kind = t
        .get::<_, String>("type")
        .ok()
        .and_then(|s| kind_from_str(&s))
        .or_else(|| default_val.as_ref().and_then(kind_from_value))
        .unwrap_or(FieldKind::Number);

    let range = t
        .get::<_, Table>("range")
        .ok()
        .and_then(|r| Some((r.get::<_, f64>(1).ok()?, r.get::<_, f64>(2).ok()?)));

    let default = default_val
        .and_then(|v| value_as(kind, &v))
        .unwrap_or_else(|| zero_for(kind));

    ScriptField {
        name,
        kind,
        range,
        default,
        tooltip: t.get::<_, String>("tooltip").ok(),
        header: t.get::<_, String>("header").ok(),
    }
}

fn kind_from_str(s: &str) -> Option<FieldKind> {
    match s {
        "number" => Some(FieldKind::Number),
        "boolean" | "bool" => Some(FieldKind::Boolean),
        "string" | "text" => Some(FieldKind::Text),
        _ => None,
    }
}

fn kind_from_value(v: &Value) -> Option<FieldKind> {
    match v {
        Value::Number(_) | Value::Integer(_) => Some(FieldKind::Number),
        Value::Boolean(_) => Some(FieldKind::Boolean),
        Value::String(_) => Some(FieldKind::Text),
        _ => None,
    }
}

/// Coerce a Lua value into a stored field value of the declared `kind`.
fn value_as(kind: FieldKind, v: &Value) -> Option<ScriptFieldValue> {
    match kind {
        FieldKind::Number => match v {
            Value::Number(n) => Some(ScriptFieldValue::Number(*n)),
            Value::Integer(i) => Some(ScriptFieldValue::Number(*i as f64)),
            _ => None,
        },
        FieldKind::Boolean => match v {
            Value::Boolean(b) => Some(ScriptFieldValue::Boolean(*b)),
            _ => None,
        },
        FieldKind::Text => match v {
            Value::String(s) => Some(ScriptFieldValue::Text(s.to_str().ok()?.to_owned())),
            _ => None,
        },
    }
}

fn zero_for(kind: FieldKind) -> ScriptFieldValue {
    match kind {
        FieldKind::Number => ScriptFieldValue::Number(0.0),
        FieldKind::Boolean => ScriptFieldValue::Boolean(false),
        FieldKind::Text => ScriptFieldValue::Text(String::new()),
    }
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
