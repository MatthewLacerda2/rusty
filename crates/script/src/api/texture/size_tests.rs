use mlua::{Lua, Table};

use super::register;

const TRIM: &str = r#"{ size = {64, 16}, nodes = {
    { id = "b", op = "brick", rows = 2, cols = 8 },
    { id = "n", op = "bump_to_normal", strength = 0.5, inputs = {"b"} } } }"#;

fn lua(file: &str) -> (Lua, std::path::PathBuf) {
    let lua = Lua::new();
    register(&lua).unwrap();
    let path = rusty_test_support::temp::dir().join(file);
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

fn strip(w: u32) -> String {
    format!(
        r#"{{ size = {{{w}, 4}}, nodes = {{ {{ id = "c", op = "constant", color = {{1, 0, 0, 1}} }} }},
        outputs = {{ base_color = "c", roughness = "c" }} }}"#
    )
}

fn max_size(path: &std::path::Path) -> (u32, bool) {
    let meta = rusty_asset::sidecar::meta_path(path).exists();
    (rusty_asset::sidecar::texture(path).max_size, meta)
}

#[test]
fn a_bake_larger_than_the_default_ships_at_its_size() {
    let (lua, path) = lua("rusty_texture_wide.png");
    let script = format!(r#"return Texture.Bake({}, OUT, "base_color")"#, strip(4096));
    lua.load(script).exec().unwrap();
    assert_eq!(max_size(&path), (4096, true), "no .meta before, 4096 after");
    let small = path.with_file_name("rusty_texture_narrow.png");
    lua.globals().set("OUT", small.to_str().unwrap()).unwrap();
    let script = format!(r#"return Texture.Bake({}, OUT, "base_color")"#, strip(64));
    lua.load(script).exec().unwrap();
    assert_eq!(
        max_size(&small),
        (2048, false),
        "a bake that fits writes none"
    );
}

#[test]
fn a_set_bake_keeps_every_output_at_its_size() {
    let (lua, _) = lua("rusty_texture_wide_set");
    let script = format!("return Texture.BakeSet({}, OUT)", strip(8192));
    let paths: Table = lua.load(script).eval().unwrap();
    for slot in ["base_color", "roughness"] {
        let out = std::path::PathBuf::from(paths.get::<String>(slot).unwrap());
        assert_eq!(max_size(&out), (8192, true), "{slot}");
    }
}
