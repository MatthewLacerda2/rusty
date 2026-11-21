use std::path::Path;

use mlua::Lua;

use super::register;

fn lua(file: &str) -> (Lua, std::path::PathBuf) {
    let lua = Lua::new();
    register(&lua).unwrap();
    crate::api::assets::register(&lua).unwrap();
    let path = rusty_test_support::temp::dir().join(file);
    lua.globals().set("OUT", path.to_str().unwrap()).unwrap();
    (lua, path)
}

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

#[test]
fn a_get_edit_set_round_trip_leaves_max_size_the_engines() {
    let (lua, path) = lua("rusty_texture_round_trip.png");
    bake(&lua, 256).unwrap();
    let edit = r#"local s = Assets.GetTextureImportSettings(OUT)
        s.wrapMode = "Clamp"
        Assets.SetTextureImportSettings(OUT, s)"#;
    lua.load(edit).exec().unwrap();
    bake(&lua, 4096).unwrap();
    assert_eq!(max_size(&path), 4096, "the round trip chose nothing");
    lua.load("Assets.SetTextureImportSettings(OUT, { maxSize = 1024 })")
        .exec()
        .unwrap();
    let chosen: bool = lua
        .load("return Assets.GetTextureImportSettings(OUT).maxSizeChosen")
        .eval()
        .unwrap();
    assert!(chosen, "a changed maxSize is the developer's");
    lua.load("Assets.SetTextureImportSettings(OUT, { maxSizeChosen = false })")
        .exec()
        .unwrap();
    bake(&lua, 8192).unwrap();
    assert_eq!(max_size(&path), 8192, "handed back to the engine");
}

#[test]
fn a_4096_mesh_map_bake_ships_at_4096() {
    let (lua, path) = lua("rusty_mesh_maps_size/l_position.png");
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir).unwrap();
    let quad = "v 0 0 0\nv 1 0 0\nv 1 0 1\nv 0 0 1\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\n\
                vn 0 1 0\nf 1/1/1 3/3/1 2/2/1\nf 1/1/1 4/4/1 3/3/1\n";
    std::fs::write(dir.join("l.obj"), quad).unwrap();
    let globals = lua.globals();
    globals
        .set("MODEL", dir.join("l.obj").to_str().unwrap())
        .unwrap();
    globals
        .set("PREFIX", format!("{}/l_", dir.to_str().unwrap()))
        .unwrap();
    let bake = r#"Texture.BakeMeshMaps(MODEL, { "position" }, PREFIX, { size = { 4096, 4 } })"#;
    lua.load(bake).exec().unwrap();
    assert_eq!(max_size(&path), 4096);
    let imported =
        rusty_asset::texture_import::imported_size(&path, &rusty_asset::sidecar::texture(&path));
    assert_eq!(
        imported,
        Some(((4096, 4), (4096, 4))),
        "imported at its size"
    );
}
