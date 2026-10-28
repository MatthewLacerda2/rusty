//! `image` nodes through the Lua surface (#1491): every verb reads the file first.

use mlua::Lua;

use super::register;

/// A Lua VM with `Texture` registered, `SRC` naming a 2×2 PNG of linear-raw 0.25
/// grey (byte 64) and `OUT` a path to bake to.
fn lua_with_source() -> Lua {
    let lua = Lua::new();
    register(&lua).unwrap();
    let dir = crate::test_temp::dir();
    let src = dir.join("grey.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([64, 64, 64, 255]))
        .save(&src)
        .unwrap();
    let globals = lua.globals();
    globals.set("SRC", src.to_str().unwrap()).unwrap();
    globals
        .set("OUT", dir.join("out.png").to_str().unwrap())
        .unwrap();
    lua
}

#[test]
fn bake_reads_the_image_file_and_writes_16_bit_data() {
    let lua = lua_with_source();
    let out: String = lua
        .load(
            r#"return Texture.Bake({ resolution = 4, nodes = {
                { id = "src", op = "image", path = SRC, encoding = "linear" },
                { id = "inv", op = "invert", inputs = { "src" } } } }, OUT, "data")"#,
        )
        .eval()
        .unwrap();
    let back = image::open(out).unwrap().to_rgba16();
    let expected = 65535 - 64 * 257;
    assert_eq!(
        back.get_pixel(1, 2).0,
        [expected, expected, expected, 65535]
    );
}

#[test]
fn stats_read_the_image_file() {
    let lua = lua_with_source();
    let mean: f32 = lua
        .load(
            r#"local s = Texture.Stats({ resolution = 4, nodes = {
                { id = "src", op = "image", path = SRC, encoding = "linear" } } })
               return s.src.mean[1]"#,
        )
        .eval()
        .unwrap();
    assert!((mean - 64.0 / 255.0).abs() < 1e-6, "{mean}");
}

#[test]
fn a_missing_image_file_fails_the_bake_naming_node_and_path() {
    let lua = lua_with_source();
    let err = lua
        .load(
            r#"return Texture.Bake({ resolution = 4, nodes = {
                { id = "photo", op = "image", path = "textures/no_such_1491.png" } } },
                OUT, "base_color")"#,
        )
        .eval::<String>()
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("'photo'") && err.contains("textures/no_such_1491.png"),
        "{err}"
    );
}

#[test]
fn bake_set_without_outputs_says_so_before_reading_files() {
    let lua = lua_with_source();
    let err = lua
        .load(
            r#"return Texture.BakeSet({ resolution = 4, nodes = {
                { id = "photo", op = "image", path = "textures/no_such_1491.png" } } }, OUT)"#,
        )
        .exec()
        .unwrap_err()
        .to_string();
    assert!(err.contains("no `outputs`"), "{err}");
}
