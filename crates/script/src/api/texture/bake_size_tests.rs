//! A rebake moves the Max Size the engine chose and never the developer's
//! (#2259), seen from Lua: `Texture.Bake` again at a larger size ships at it, a
//! `maxSize` set through `Assets.SetTextureImportSettings` holds, the import
//! warning once that the file is shrunk, and a bake above 8192 is refused.

use std::path::Path;

use mlua::Lua;

use super::register;

/// A Lua VM with `Texture` and `Assets`, `OUT` a fresh path named `file`.
fn lua(file: &str) -> (Lua, std::path::PathBuf) {
    let lua = Lua::new();
    register(&lua).unwrap();
    crate::api::assets::register(&lua).unwrap();
    let path = rusty_test_support::temp::dir().join(file);
    lua.globals().set("OUT", path.to_str().unwrap()).unwrap();
    (lua, path)
}

/// `Texture.Bake` of a one-colour `{w}`×4 recipe to `OUT`.
fn bake(lua: &Lua, w: u32) -> mlua::Result<()> {
    let recipe = format!(
        r#"{{ size = {{{w}, 4}}, nodes = {{ {{ id = "c", op = "constant", color = {{1, 0, 0, 1}} }} }} }}"#
    );
    let script = format!(r#"Texture.Bake({recipe}, OUT, "base_color")"#);
    lua.load(script).exec()
}

fn max_size(path: &Path) -> u32 {
    rusty_asset::sidecar::texture(path).max_size
}

#[test]
fn a_rebake_at_8192_ships_at_8192() {
    let (lua, path) = lua("rusty_texture_rebake_wide.png");
    bake(&lua, 4096).unwrap();
    assert_eq!(max_size(&path), 4096);
    bake(&lua, 8192).unwrap();
    assert_eq!(max_size(&path), 8192, "the engine's Max Size follows");
}

#[test]
fn a_max_size_set_by_the_developer_holds_through_a_rebake_and_warns_once() {
    rusty_test_support::logs::init();
    let (lua, path) = lua("rusty_texture_rebake_set.png");
    bake(&lua, 256).unwrap();
    lua.load("Assets.SetTextureImportSettings(OUT, { maxSize = 1024 })")
        .exec()
        .unwrap();
    bake(&lua, 4096).unwrap();
    assert_eq!(max_size(&path), 1024, "the developer's Max Size is kept");
    for _ in 0..2 {
        rusty_asset::texture_import::refresh(&path, "").unwrap();
    }
    let named = format!(
        "[Import] {}: 4096x4 is larger than its Max Size 1024",
        path.display()
    );
    let warned = rusty_test_support::logs::take();
    let warned = warned.iter().filter(|m| m.starts_with(&named)).count();
    assert_eq!(warned, 1, "said once across re-imports");
}

#[test]
fn a_bake_above_the_largest_max_size_is_refused() {
    let (lua, path) = lua("rusty_texture_too_wide.png");
    let err = bake(&lua, 16384).unwrap_err().to_string();
    assert!(
        err.contains("Texture.Bake: a 16384x4 bake is larger than"),
        "{err}"
    );
    assert!(!path.exists(), "nothing written");
}
