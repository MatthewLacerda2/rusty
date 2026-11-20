//! `Assets.GetTextureImportSettings` on a texture a model's conversion extracted
//! says what its import uses (the slot's type), by its file's path and by its
//! `model.fbx#tex/<name>` reference, and `SetTextureImportSettings` refuses it.

use mlua::Lua;
use rusty_asset::convert::{cached_gltf, texture_ref};
use rusty_test_support::models::{embedded_fbx, png, Embedded};

/// A VM with `Assets`, and the two spellings of an FBX's embedded normal map.
fn normal_map() -> (Lua, [String; 2]) {
    let lua = Lua::new();
    super::super::register(&lua).unwrap();
    let fbx = rusty_test_support::temp::dir().join("m.fbx");
    let bumps = png(8, 5);
    let maps = [Embedded {
        name: "Bumps",
        property: "NormalMap",
        png: &bumps,
    }];
    std::fs::write(&fbx, embedded_fbx(&maps)).unwrap();
    cached_gltf(&fbx).expect("the model converts");
    let reference = texture_ref::reference(&fbx.to_string_lossy(), "Bumps");
    let file = texture_ref::file(&reference).expect("extracted");
    (lua, [file.to_string_lossy().into_owned(), reference])
}

#[test]
fn get_says_an_extracted_normal_map_is_one_by_either_spelling() {
    let (lua, spellings) = normal_map();
    for path in spellings {
        lua.globals().set("P", path.as_str()).unwrap();
        let typed: String = lua
            .load("return Assets.GetTextureImportSettings(P).textureType")
            .eval()
            .unwrap();
        assert_eq!(typed, "NormalMap", "{path}");
    }
}

#[test]
fn set_refuses_an_extracted_texture_by_either_spelling() {
    let (lua, spellings) = normal_map();
    for path in spellings {
        lua.globals().set("P", path.as_str()).unwrap();
        let set = lua
            .load(r#"Assets.SetTextureImportSettings(P, { filterMode = "Point" })"#)
            .exec();
        let why = set.expect_err(&path).to_string();
        assert!(why.contains("extracted from a model"), "{why}");
    }
}

#[test]
fn a_reference_the_cache_lacks_is_an_error() {
    let (lua, [_, reference]) = normal_map();
    let missing = reference.replace("#tex/Bumps", "#tex/Nope");
    lua.globals().set("P", missing).unwrap();
    let got = lua.load("Assets.GetTextureImportSettings(P)").exec();
    assert!(got.is_err());
}
