use std::path::{Path, PathBuf};

use mlua::Lua;

fn baked(file: &str) -> (Lua, PathBuf) {
    let lua = Lua::new();
    crate::api::texture::register(&lua).unwrap();
    super::super::register(&lua).unwrap();
    let path = rusty_test_support::temp::dir().join(file);
    lua.globals().set("OUT", path.to_str().unwrap()).unwrap();
    bake(&lua, 256);
    (lua, path)
}

fn bake(lua: &Lua, w: u32) {
    let script = format!(
        r#"Texture.Bake({{ size = {{{w}, 4}}, nodes = {{ {{ id = "c", op = "constant",
            color = {{1, 0, 0, 1}} }} }} }}, OUT, "base_color")"#
    );
    lua.load(script).exec().unwrap();
}

fn round_trip(lua: &Lua, edit: &str) {
    let script = format!(
        "local s = Assets.GetTextureImportSettings(OUT) {edit} \
         Assets.SetTextureImportSettings(OUT, s)"
    );
    lua.load(script).exec().unwrap();
}

fn max_size(path: &Path) -> u32 {
    rusty_asset::sidecar::texture(path).max_size
}

#[test]
fn a_max_size_changed_in_a_round_trip_holds_through_a_bake() {
    let (lua, path) = baked("rusty_round_trip_max_size.png");
    round_trip(&lua, "s.maxSize = 1024");
    assert!(rusty_asset::sidecar::texture(&path).max_size_chosen);
    bake(&lua, 4096);
    assert_eq!(max_size(&path), 1024, "the developer's Max Size is kept");
}

#[test]
fn a_round_trip_editing_another_setting_leaves_max_size_the_engines() {
    let (lua, path) = baked("rusty_round_trip_wrap.png");
    round_trip(&lua, r#"s.wrapMode = "Clamp""#);
    assert!(!rusty_asset::sidecar::texture(&path).max_size_chosen);
    bake(&lua, 4096);
    assert_eq!(
        max_size(&path),
        4096,
        "the engine's Max Size follows the bake"
    );
}

#[test]
fn max_size_chosen_false_hands_a_chosen_max_size_back() {
    let (lua, path) = baked("rusty_round_trip_hand_back.png");
    round_trip(&lua, "s.maxSize = 1024");
    round_trip(&lua, "s.maxSizeChosen = false");
    assert!(!rusty_asset::sidecar::texture(&path).max_size_chosen);
    bake(&lua, 4096);
    assert_eq!(max_size(&path), 4096, "handed back to the engine");
}

#[test]
fn stream_mipmaps_is_on_by_default_and_turns_off_through_a_round_trip() {
    let (lua, path) = baked("rusty_round_trip_stream.png");
    let on: bool = lua
        .load("return Assets.GetTextureImportSettings(OUT).streamMipmaps")
        .eval()
        .unwrap();
    assert!(on, "a texture streams unless it opts out");
    round_trip(&lua, "s.streamMipmaps = false");
    assert!(!rusty_asset::sidecar::texture(&path).stream_mipmaps);
}
