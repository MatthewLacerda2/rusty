//! Tests for `Texture.BakeSet` (#403): one evaluation, one PNG per slot.

use std::path::PathBuf;

use mlua::{Lua, Table};

use super::register;

/// A brick-and-noise graph with three outputs sharing the `b` upstream node.
const RECIPE: &str = r#"{
  resolution = 32, seed = 7,
  nodes = {
    { id = "b", op = "brick", rows = 4, cols = 2 },
    { id = "n", op = "noise", kind = "fbm", scale = 4, octaves = 2 },
    { id = "albedo", op = "mix", mode = "multiply", factor = 1, inputs = {"b", "n"} },
    { id = "nrm", op = "bump_to_normal", strength = 0.05, inputs = {"b"} },
    { id = "mr", op = "combine_rgb", inputs = {"n", "b"} },
  },
  outputs = { albedo = "albedo", normal = "nrm", orm = "mr" },
}"#;

fn lua_with(dir: &str) -> (Lua, PathBuf) {
    let lua = Lua::new();
    register(&lua).unwrap();
    let dir = crate::test_temp::dir().join(dir);
    std::fs::create_dir_all(&dir).unwrap();
    lua.globals().set("DIR", dir.to_str().unwrap()).unwrap();
    (lua, dir)
}

#[test]
fn bake_set_matches_baking_each_output_singly() {
    let (lua, dir) = lua_with("rusty_bake_set_identical");
    let set: Table = lua
        .load(format!("return Texture.BakeSet({RECIPE}, DIR .. '/brick')"))
        .eval()
        .unwrap();
    let slots: Vec<String> = set
        .clone()
        .pairs::<String, String>()
        .map(|p| p.unwrap().0)
        .collect();
    assert_eq!(slots.len(), 3, "{slots:?}");
    for (slot, node) in [
        ("base_color", "albedo"),
        ("normal", "nrm"),
        ("metallic_roughness", "mr"),
    ] {
        let path: String = set.get(slot).unwrap();
        assert!(path.ends_with(&format!("brick_{slot}.png")), "{path}");
        let single: String = lua
            .load(format!(
                "local r = {RECIPE}; r.outputs = nil; r.output = '{node}'
                 return Texture.Bake(r, DIR .. '/single_{slot}.png', '{slot}')"
            ))
            .eval()
            .unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            std::fs::read(&single).unwrap(),
            "{slot} differs from its single bake"
        );
    }
    std::fs::remove_dir_all(dir).ok();
}

fn refused(lua: &Lua, outputs: &str) -> String {
    let script = format!(
        "local r = {RECIPE}; r.outputs = {outputs}
         return Texture.BakeSet(r, DIR .. '/bad')"
    );
    lua.load(script).eval::<Table>().unwrap_err().to_string()
}

#[test]
fn a_bad_outputs_table_is_refused_before_writing() {
    let (lua, dir) = lua_with("rusty_bake_set_refused");
    let err = refused(&lua, "nil");
    assert!(err.contains("no `outputs`"), "{err}");
    let err = refused(&lua, "{ basecolour = 'albedo' }");
    assert!(err.contains("unknown slot \"basecolour\""), "{err}");
    let err = refused(&lua, "{ albedo = 'albedo', base_color = 'b' }");
    assert!(err.contains("base_color slot twice"), "{err}");
    let err = refused(&lua, "{ normal = 'nrm', base_color = 'ghost' }");
    assert!(err.contains("'ghost' not found"), "{err}");
    let written = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(written, 0, "a refused bake set writes nothing");
    std::fs::remove_dir_all(dir).ok();
}
