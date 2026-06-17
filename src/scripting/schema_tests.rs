//! Tests for the script field schema parser (#84). Kept in a sibling file so the
//! impl module stays well under the size cap.

use super::*;
use crate::components::ScriptFieldValue;
use mlua::{Lua, Table};
use std::collections::BTreeMap;

fn field<'a>(fields: &'a [ScriptField], name: &str) -> &'a ScriptField {
    fields
        .iter()
        .find(|f| f.name == name)
        .expect("field present")
}

#[test]
fn no_schema_yields_no_fields() {
    assert!(parse_fields("return { Start = function() end }").is_empty());
    assert!(parse_fields("return 42").is_empty());
    assert!(parse_fields("not lua").is_empty());
}

#[test]
fn full_metadata_table_is_parsed() {
    let code = r#"
        return { fields = {
            speed = { type = "number", range = {0, 1}, default = 0.5, tooltip = "u/s", header = "Movement" },
        }}
    "#;
    let fields = parse_fields(code);
    let f = field(&fields, "speed");
    assert_eq!(f.kind, FieldKind::Number);
    assert_eq!(f.range, Some((0.0, 1.0)));
    assert_eq!(f.default, ScriptFieldValue::Number(0.5));
    assert_eq!(f.tooltip.as_deref(), Some("u/s"));
    assert_eq!(f.header.as_deref(), Some("Movement"));
}

#[test]
fn type_is_inferred_from_default_when_absent() {
    let code = r#"return { fields = {
        flag = { default = true },
        label = { default = "hi" },
    }}"#;
    let fields = parse_fields(code);
    assert_eq!(field(&fields, "flag").kind, FieldKind::Boolean);
    assert_eq!(field(&fields, "label").kind, FieldKind::Text);
}

#[test]
fn bare_values_infer_type_and_become_defaults() {
    let code = r#"return { fields = {
        n = 3,
        ratio = 0.25,
        on = false,
        name = "Rusty",
    }}"#;
    let fields = parse_fields(code);
    assert_eq!(field(&fields, "n").default, ScriptFieldValue::Number(3.0));
    assert_eq!(field(&fields, "ratio").kind, FieldKind::Number);
    assert_eq!(
        field(&fields, "on").default,
        ScriptFieldValue::Boolean(false)
    );
    assert_eq!(
        field(&fields, "name").default,
        ScriptFieldValue::Text("Rusty".into())
    );
}

#[test]
fn fields_are_sorted_by_name() {
    let code = r#"return { fields = { zebra = 1, alpha = 2, mid = 3 } }"#;
    let names: Vec<_> = parse_fields(code).into_iter().map(|f| f.name).collect();
    assert_eq!(names, vec!["alpha", "mid", "zebra"]);
}

#[test]
fn apply_uses_override_then_default() {
    let lua = Lua::new();
    let code = r#"return { fields = {
        speed = { type = "number", default = 0.5 },
        title = { type = "string", default = "base" },
        on = { type = "boolean", default = false },
    }}"#;
    let table: Table = lua.load(code).eval().unwrap();

    let mut overrides = BTreeMap::new();
    overrides.insert("speed".to_string(), ScriptFieldValue::Number(2.0));
    overrides.insert("on".to_string(), ScriptFieldValue::Boolean(true));
    apply_field_values(&table, code, &overrides).unwrap();

    // Overridden field wins; untouched field falls back to schema default.
    assert_eq!(table.get::<_, f64>("speed").unwrap(), 2.0);
    assert!(table.get::<_, bool>("on").unwrap());
    assert_eq!(table.get::<_, String>("title").unwrap(), "base");
}

#[test]
fn apply_without_schema_is_a_noop() {
    let lua = Lua::new();
    let code = "return { Start = function() end }";
    let table: Table = lua.load(code).eval().unwrap();
    apply_field_values(&table, code, &BTreeMap::new()).unwrap();
    assert!(table.get::<_, mlua::Value>("anything").unwrap().is_nil());
}
