//! Tests for the folder a `Texture` bake writes into (#1850).

use mlua::Lua;

use super::super::{bake, bake_whole_set, from_lua::parse_recipe, register};
use super::make_parents;

const SET: &str = r#"return { resolution = 8,
  nodes = { { id = "c", op = "constant", color = { 1, 1, 1, 1 } } },
  outputs = { base_color = "c" } }"#;

fn recipe() -> crate::procgen::TextureRecipe {
    let lua = Lua::new();
    parse_recipe(&lua.load(SET).eval().unwrap()).unwrap()
}

#[test]
fn bake_makes_a_missing_nested_folder_inside_the_project() {
    let root = crate::test_temp::dir();
    let path = root.join("textures").join("new").join("a.png");
    let p = path.to_str().unwrap();
    assert_eq!(bake(&recipe(), p, "base_color", &root).unwrap(), p);
    assert!(path.is_file());
}

#[test]
fn bake_set_and_a_relative_path_make_missing_nested_folders() {
    let root = crate::test_temp::dir();
    // Relative, as a game writes it: read against the root. The write itself reads
    // against the working directory, so the prefix is made absolute for it here.
    make_parents("assets/set/sub/brick", &root).unwrap();
    assert!(root.join("assets").join("set").join("sub").is_dir());
    let prefix = root.join("deep").join("er").join("brick");
    let written = bake_whole_set(&recipe(), prefix.to_str().unwrap(), &root).unwrap();
    assert!(std::path::Path::new(&written["base_color"]).is_file());
}

#[test]
fn preview_makes_a_missing_nested_folder_inside_the_project() {
    let root = crate::test_temp::dir();
    let path = root.join("previews").join("new").join("sheet.png");
    let p = path.to_str().unwrap();
    let lua = Lua::new();
    let t = super::super::inspect::preview_table(&lua, &recipe(), p, Some(8), &root).unwrap();
    assert_eq!(t.get::<String>("path").unwrap(), p);
    assert!(path.is_file());
}

#[test]
fn a_folder_outside_the_project_is_not_made() {
    let root = crate::test_temp::dir().join("project");
    std::fs::create_dir_all(&root).unwrap();
    let outside = root.parent().unwrap().join("elsewhere").join("x.png");
    make_parents(outside.to_str().unwrap(), &root).unwrap();
    assert!(!outside.parent().unwrap().exists());
    make_parents("../climb/x.png", &root).unwrap();
    assert!(!root.parent().unwrap().join("climb").exists());
}

#[test]
fn a_file_where_a_folder_should_be_fails_naming_the_path() {
    let lua = Lua::new();
    register(&lua).unwrap();
    let blocker = crate::test_temp::dir().join("blocker");
    std::fs::write(&blocker, b"not a folder").unwrap();
    let target = blocker.join("x");
    lua.globals().set("OUT", target.to_str().unwrap()).unwrap();
    let set = format!("local r = (function() {SET} end)()\n");
    let bake_err = lua
        .load(format!(
            "{set}return Texture.Bake(r, OUT .. '.png', 'base_color')"
        ))
        .eval::<String>()
        .unwrap_err()
        .to_string();
    assert!(
        bake_err.contains("Texture.Bake: cannot write"),
        "{bake_err}"
    );
    assert!(
        bake_err.contains(&format!("{}.png", target.display())),
        "{bake_err}"
    );
    let set_err = lua
        .load(format!("{set}return Texture.BakeSet(r, OUT)"))
        .eval::<mlua::Table>()
        .unwrap_err()
        .to_string();
    assert!(set_err.contains("Texture.BakeSet: cannot"), "{set_err}");
    assert!(
        set_err.contains(&blocker.display().to_string()),
        "{set_err}"
    );
}

#[test]
fn inside_the_project_a_file_in_the_way_names_the_folder() {
    let root = crate::test_temp::dir();
    std::fs::write(root.join("taken"), b"").unwrap();
    let err = make_parents("taken/x.png", &root).unwrap_err();
    assert!(
        err.contains("cannot make the folder") && err.contains("taken"),
        "{err}"
    );
}
