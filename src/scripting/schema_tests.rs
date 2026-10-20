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
    assert_eq!(table.get::<f64>("speed").unwrap(), 2.0);
    assert!(table.get::<bool>("on").unwrap());
    assert_eq!(table.get::<String>("title").unwrap(), "base");
}

#[test]
fn apply_without_schema_is_a_noop() {
    let lua = Lua::new();
    let code = "return { Start = function() end }";
    let table: Table = lua.load(code).eval().unwrap();
    apply_field_values(&table, code, &BTreeMap::new()).unwrap();
    assert!(table.get::<mlua::Value>("anything").unwrap().is_nil());
}

#[test]
fn a_script_requiring_a_helper_module_still_parses() {
    // Slider and Scrollbar `require("ui.range_axis")` at their top level (#996);
    // the schema read never loads the module, so it neither fails nor touches disk.
    let code = r#"
        local Axis = require("ui.does_not_exist")
        return { fields = { speed = 2 }, Start = function() Axis.parse() end }
    "#;
    assert_eq!(parse_fields(code).len(), 1);
    assert!(crate::scripting::discovery::exposes_lifecycle(code));
}

#[test]
fn type_aliases_parse_and_an_unknown_type_falls_back_to_the_default_s_kind() {
    let code = r#"return { fields = {
        b = { type = "bool", default = true },
        t = { type = "text", default = "x" },
        n = { type = "vector", default = 4 },
        none = { type = "vector" },
        spoken = { default = {} },
    }}"#;
    let fields = parse_fields(code);
    assert_eq!(field(&fields, "b").default, ScriptFieldValue::Boolean(true));
    assert_eq!(
        field(&fields, "t").default,
        ScriptFieldValue::Text("x".into())
    );
    assert_eq!(field(&fields, "n").default, ScriptFieldValue::Number(4.0));
    // Nothing to infer from: a number, zeroed.
    assert_eq!(
        field(&fields, "none").default,
        ScriptFieldValue::Number(0.0)
    );
    assert_eq!(field(&fields, "spoken").kind, FieldKind::Number);
}

#[test]
fn a_default_of_the_wrong_kind_is_zeroed_for_the_declared_type() {
    let code = r#"return { fields = {
        n = { type = "number", default = "fast" },
        on = { type = "boolean", default = 1 },
        label = { type = "string", default = false },
        whole = { type = "number", default = 7 },
    }}"#;
    let fields = parse_fields(code);
    assert_eq!(field(&fields, "n").default, ScriptFieldValue::Number(0.0));
    assert_eq!(
        field(&fields, "on").default,
        ScriptFieldValue::Boolean(false)
    );
    assert_eq!(
        field(&fields, "label").default,
        ScriptFieldValue::Text(String::new())
    );
    assert_eq!(
        field(&fields, "whole").default,
        ScriptFieldValue::Number(7.0)
    );
}

#[test]
fn a_bare_value_that_is_not_a_scalar_is_no_field() {
    let code = r#"return { fields = { f = function() end, ok = 1 } }"#;
    let names: Vec<_> = parse_fields(code).into_iter().map(|f| f.name).collect();
    assert_eq!(names, vec!["ok"]);
}
