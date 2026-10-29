//! Non-square recipes from Lua (#1493): `size = {w, h}` bakes that shape, the
//! inspectors keep its aspect, and a side that isn't a power of two is refused.

use mlua::{Lua, Table};

use super::register;

const TRIM: &str = r#"{ size = {64, 16}, nodes = {
    { id = "b", op = "brick", rows = 2, cols = 8 },
    { id = "n", op = "bump_to_normal", strength = 0.5, inputs = {"b"} } } }"#;

/// A Lua VM with `Texture` registered and `OUT` a path named `file` to write to.
fn lua(file: &str) -> (Lua, std::path::PathBuf) {
    let lua = Lua::new();
    register(&lua).unwrap();
    let path = crate::test_temp::dir().join(file);
    lua.globals().set("OUT", path.to_str().unwrap()).unwrap();
    (lua, path)
}

#[test]
fn a_sized_recipe_bakes_its_own_shape() {
    let (lua, path) = lua("rusty_texture_trim.png");
    let script = format!(r#"return Texture.Bake({TRIM}, OUT, "normal")"#);
    lua.load(script).exec().unwrap();
    assert_eq!(image::image_dimensions(&path).unwrap(), (64, 16));
    std::fs::remove_file(path).ok();
}

#[test]
fn the_preview_keeps_the_aspect_and_stats_rescale_it() {
    let (lua, path) = lua("rusty_texture_trim_preview.png");
    let r: Table = lua
        .load(format!("return Texture.Preview({TRIM}, OUT, 32)"))
        .eval()
        .unwrap();
    assert_eq!(r.get::<u32>("columns").unwrap(), 2);
    // Two 32×8 tiles side by side.
    assert_eq!(image::image_dimensions(&path).unwrap(), (64, 8));
    let stats: Table = lua
        .load(format!("return Texture.Stats({TRIM}, 128)"))
        .eval()
        .unwrap();
    assert!(stats.contains_key("n").unwrap());
    let err = lua
        .load(format!("return Texture.Stats({TRIM}, 100)"))
        .exec()
        .unwrap_err()
        .to_string();
    assert!(err.contains("Texture.Stats: resolution 100"), "{err}");
    std::fs::remove_file(path).ok();
}

#[test]
fn a_side_that_is_not_a_power_of_two_is_refused() {
    let (lua, path) = lua("rusty_texture_bad_size.png");
    let script = r#"return Texture.Bake({ size = {64, 12}, nodes = {
        { id = "c", op = "constant", color = {1, 0, 0, 1} } } }, OUT, "base_color")"#;
    let err = lua.load(script).exec().unwrap_err().to_string();
    assert!(
        err.contains("size side 12 is not a positive power of two"),
        "{err}"
    );
    assert!(!path.exists(), "nothing written");
}
