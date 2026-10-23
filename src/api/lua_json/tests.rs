//! The script-visible answers of the serde bridge (#755): each pins one rule the
//! module doc states, so an mlua upgrade that changes a default fails here.

use mlua::{Lua, Value};
use serde_json::json;

use super::*;

fn json_of(lua: &Lua, src: &str) -> Result<serde_json::Value, String> {
    lua_to_json(&lua.load(src).eval::<Value>().unwrap())
}

#[test]
fn tables_become_objects_arrays_or_both() {
    let lua = Lua::new();
    let j = |src| json_of(&lua, src).unwrap();
    assert_eq!(
        j("return { m = 0.5, mode = 'Cutout' }"),
        json!({"m": 0.5, "mode": "Cutout"})
    );
    assert_eq!(j("return { 0.1, 0.2, 0.3 }"), json!([0.1, 0.2, 0.3]));
    assert_eq!(j("return { c = { 1.0, 0.5 } }"), json!({"c": [1.0, 0.5]}));
    assert_eq!(j("return {}"), json!({}), "an empty table is an object");
    assert_eq!(
        j("return { a = {}, b = { {} } }"),
        json!({"a": {}, "b": [{}]})
    );
    assert_eq!(
        j("return { 1, 2, a = 3 }"),
        json!({"1": 1, "2": 2, "a": 3}),
        "mixed keeps every key"
    );
    assert_eq!(
        j("return { 1, nil, 3 }"),
        json!([1, null, 3]),
        "a nil hole is null"
    );
    assert_eq!(
        j("return { n = 2, f = 2.0 }"),
        json!({"n": 2, "f": 2.0}),
        "integer vs float kept"
    );
}

#[test]
fn unsupported_values_and_cycles_are_errors() {
    let lua = Lua::new();
    let err = json_of(&lua, "return { f = print }").unwrap_err();
    assert!(err.contains("function"), "{err}");
    assert!(json_of(&lua, "local t = {} t.t = t return t").is_err());
}

#[test]
fn serde_values_come_back_as_plain_tables() {
    let lua = Lua::new();
    let v = to_lua(&lua, &json!({"a": [1, 2.5], "n": null})).unwrap();
    lua.globals().set("v", v).unwrap();
    let probe: (i64, f64, bool, bool) = lua
        .load("return v.a[1], v.a[2], v.n == nil, getmetatable(v.a) == nil")
        .eval()
        .unwrap();
    assert_eq!(probe, (1, 2.5, true, true));
}

#[test]
fn a_table_and_its_json_string_decode_alike() {
    #[derive(serde::Deserialize, Debug, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct R {
        a: f32,
        b: Vec<u8>,
    }
    let lua = Lua::new();
    let val = |src: &str| lua.load(src).eval::<Value>().unwrap();
    let from_table: R = recipe_from_lua(&val("return { a = 0.5, b = {1, 2} }")).unwrap();
    let from_json: R = recipe_from_lua(&val(r#"return '{"a":0.5,"b":[1,2]}'"#)).unwrap();
    assert_eq!(from_table, from_json);

    let e1 = recipe_from_lua::<R>(&val("return { a = 1, bb = {} }")).unwrap_err();
    let e2 = recipe_from_lua::<R>(&val(r#"return '{"a":1,"bb":{}}'"#)).unwrap_err();
    assert_eq!(e1, e2, "both forms name the bad key the same way");
    assert!(e1.contains("bb"), "{e1}");

    let err = recipe_from_lua::<R>(&val("return 3")).unwrap_err();
    assert!(err.contains("table or a JSON string"), "{err}");
}
