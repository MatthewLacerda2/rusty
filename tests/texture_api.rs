//! Integration coverage for the `Texture` scripting namespace (#270): an agent
//! composes a recipe table, bakes it to a PNG, and that PNG path drops straight into
//! a material map slot via the `Material` namespace — the wired-up day-one consumer.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::scene::Scene;

fn tmp(name: &str) -> String {
    // Forward slashes: a Windows temp path (`C:\Users\…`) interpolated into a Lua
    // string literal trips Lua's escape parser (`\U`). Windows accepts `/` for std/
    // `image` file I/O, and the same normalized string is used in the assertions, so
    // the round-trip still matches on every platform.
    crate::temp::dir()
        .join(name)
        .to_str()
        .unwrap()
        .replace('\\', "/")
}

#[test]
fn baked_texture_feeds_a_material_slot() {
    let lua = Lua::new();
    let scene = Rc::new(RefCell::new(Scene::new()));
    let id = scene.borrow_mut().add_entity("Crate".to_string());
    let path = tmp("rusty_texture_api_albedo.png");

    rusty::api::texture::register(&lua).unwrap();
    lua.scope(|s| {
        rusty::api::material::register(&lua, s, &scene).unwrap();
        let script = format!(
            r#"
            local out = Texture.Bake({{
              resolution = 32, seed = 7,
              nodes = {{
                {{ id = "k", op = "checker", tiles = 4,
                   color_a = {{0.1,0.1,0.1,1}}, color_b = {{0.8,0.8,0.8,1}} }},
              }},
            }}, "{path}", "base_color")
            Material.SetTexture({id}, out)
        "#
        );
        lua.load(&script).exec().unwrap();
        Ok(())
    })
    .unwrap();

    // The slot now references the baked file, and the file is a real PNG.
    let sc = scene.borrow();
    let mat = sc.material_asset_of(id).unwrap();
    assert_eq!(mat.base_color_map.as_deref(), Some(path.as_str()));
    let img = image::open(&path).expect("baked PNG decodes");
    assert_eq!(image::GenericImageView::dimensions(&img), (32, 32));
    std::fs::remove_file(path).ok();
}

#[test]
fn bake_json_matches_bake_table() {
    let lua = Lua::new();
    rusty::api::texture::register(&lua).unwrap();
    let from_table = tmp("rusty_texture_table.png");
    let from_json = tmp("rusty_texture_json.png");

    let script = format!(
        r#"
        local recipe = {{
          resolution = 24, seed = 3,
          nodes = {{ {{ id = "w", op = "white_noise" }} }},
        }}
        Texture.Bake(recipe, "{from_table}", "data")
        local json = Texture.ToJson(recipe)
        Texture.Bake(json, "{from_json}", "data")
    "#
    );
    lua.load(&script).exec().unwrap();
    assert_eq!(
        std::fs::read(&from_table).unwrap(),
        std::fs::read(&from_json).unwrap(),
        "a table bake and the JSON re-bake must be byte-identical"
    );
    std::fs::remove_file(from_table).ok();
    std::fs::remove_file(from_json).ok();
}

#[test]
fn the_documented_riveted_plate_bakes() {
    let lua = Lua::new();
    rusty::api::texture::register(&lua).unwrap();
    let path = tmp("rusty_texture_api_plate_n.png");
    // docs/api/Texture.md's hard-surface example, at a test-sized resolution.
    let script = format!(
        r#"return Texture.Bake({{
          resolution = 32, seed = 3,
          nodes = {{
            {{ id = "panel", op = "shape", kind = "rounded_rect", size = {{0.92, 0.92}}, softness = 0.03 }},
            {{ id = "dot", op = "shape", kind = "circle", size = {{0.3, 0.3}}, softness = 0.12 }},
            {{ id = "rivets", op = "tile", count = 8, jitter = 0.1, inputs = {{"dot"}} }},
            {{ id = "height", op = "mix", mode = "add", factor = 0.5, inputs = {{"panel", "rivets"}} }},
            {{ id = "n", op = "bump_to_normal", strength = 0.4, inputs = {{"height"}} }},
          }},
          output = "n",
        }}, "{path}", "normal")"#
    );
    let out: String = lua.load(&script).eval().unwrap();
    assert_eq!(out, path);
    assert!(image::open(&path).is_ok(), "baked PNG decodes");
    std::fs::remove_file(path).ok();
}
