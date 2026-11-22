use std::collections::BTreeMap;
use std::path::Path;

use mlua::{Table, Value};

use rusty_components::ScriptFieldValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Number,
    Boolean,
    Text,
}

#[derive(Clone, Debug)]
pub struct ScriptField {
    pub name: String,
    pub kind: FieldKind,
    pub range: Option<(f64, f64)>,
    pub default: ScriptFieldValue,
    pub tooltip: Option<String>,
    pub header: Option<String>,
}

/// `path` names the chunk, so an error points at the script's line; `dofile` and
/// `loadfile` resolve against the working directory, the editor's project.
pub fn parse_fields(path: &str, code: &str) -> Vec<ScriptField> {
    read_fields(Path::new(""), path, code).unwrap_or_default()
}

/// `workspace` is what Play's `dofile` and `loadfile` resolve against.
pub(super) fn read_fields(
    workspace: &Path,
    path: &str,
    code: &str,
) -> mlua::Result<Vec<ScriptField>> {
    let lua = super::sandbox::inspection_vm(workspace)?;
    let table = lua
        .load(code)
        .set_name(format!("@{path}"))
        .eval::<Table>()?;
    let Ok(fields) = table.get::<Table>("fields") else {
        return Ok(Vec::new());
    };
    let mut out: Vec<ScriptField> = fields
        .pairs::<String, Value>()
        .flatten()
        .filter_map(|(name, value)| parse_field(name, value))
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn apply_field_values(
    table: &Table,
    fields: &[ScriptField],
    overrides: &BTreeMap<String, ScriptFieldValue>,
) -> mlua::Result<()> {
    for field in fields {
        let key = field.name.as_str();
        match overrides.get(&field.name).unwrap_or(&field.default) {
            ScriptFieldValue::Number(n) => table.set(key, *n)?,
            ScriptFieldValue::Boolean(b) => table.set(key, *b)?,
            ScriptFieldValue::Text(s) => table.set(key, s.as_str())?,
        }
    }
    Ok(())
}

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
            ScriptFieldValue::Text(s.to_str().map(|s| s.to_owned()).unwrap_or_default()),
        )),
        _ => None,
    }
}

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

fn parse_meta_table(name: String, t: &Table) -> ScriptField {
    let default_val = t.get::<Value>("default").ok();
    let kind = t
        .get::<String>("type")
        .ok()
        .and_then(|s| kind_from_str(&s))
        .or_else(|| default_val.as_ref().and_then(kind_from_value))
        .unwrap_or(FieldKind::Number);

    let range = t
        .get::<Table>("range")
        .ok()
        .and_then(|r| Some((r.get::<f64>(1).ok()?, r.get::<f64>(2).ok()?)));

    let default = default_val
        .and_then(|v| value_as(kind, &v))
        .unwrap_or_else(|| zero_for(kind));

    ScriptField {
        name,
        kind,
        range,
        default,
        tooltip: t.get::<String>("tooltip").ok(),
        header: t.get::<String>("header").ok(),
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
