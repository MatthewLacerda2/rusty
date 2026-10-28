//! `Texture.Stats` / `Texture.Preview` from Lua (#1486).

use mlua::{Lua, Table};

use super::register;

const RECIPE: &str = r#"{
  resolution = 1024, seed = 3,
  nodes = {
    { id = "c", op = "constant", color = {0.2, 0.4, 0.6, 1} },
    { id = "n", op = "noise", kind = "fbm", scale = 8, octaves = 4 },
    { id = "m", op = "mix", mode = "multiply", inputs = {"c", "n"} },
  },
}"#;

fn lua() -> Lua {
    let lua = Lua::new();
    register(&lua).unwrap();
    lua
}

fn channel(stats: &Table, node: &str, field: &str, c: usize) -> f32 {
    let node: Table = stats.get(node).unwrap();
    node.get::<Table>(field).unwrap().get(c).unwrap()
}

#[test]
fn a_constant_is_its_colour_with_no_spread() {
    let lua = lua();
    let stats: Table = lua
        .load(format!("return Texture.Stats({RECIPE}, 32)"))
        .eval()
        .unwrap();
    for (c, want) in [0.2, 0.4, 0.6, 1.0].into_iter().enumerate() {
        for field in ["min", "max", "mean", "p1", "p99"] {
            assert_eq!(channel(&stats, "c", field, c + 1), want, "{field}[{c}]");
        }
        assert_eq!(channel(&stats, "c", "std", c + 1), 0.0);
    }
}

#[test]
fn a_four_octave_fbm_sits_in_the_middle_third() {
    let lua = lua();
    let stats: Table = lua
        .load(format!("return Texture.Stats({RECIPE}, 256)"))
        .eval()
        .unwrap();
    let (p1, p99) = (
        channel(&stats, "n", "p1", 1),
        channel(&stats, "n", "p99", 1),
    );
    assert!(p1 > 0.3 && p99 < 0.7 && p1 < p99, "p1 {p1}, p99 {p99}");
    assert!(
        channel(&stats, "m", "max", 1) <= 0.2,
        "every node is reported"
    );
}

#[test]
fn preview_writes_one_tile_per_node_in_node_order() {
    let lua = lua();
    let path = crate::test_temp::dir().join("rusty_texture_preview.png");
    lua.globals().set("OUT", path.to_str().unwrap()).unwrap();
    let r: Table = lua
        .load(format!(
            "local r = {RECIPE}; r.resolution = 32; return Texture.Preview(r, OUT, 16)"
        ))
        .eval()
        .unwrap();
    let order: Vec<String> = r.get("order").unwrap();
    assert_eq!(order, ["c", "n", "m"]);
    let columns: u32 = r.get("columns").unwrap();
    let (w, h) = image::GenericImageView::dimensions(&image::open(&path).unwrap());
    assert_eq!((columns, w, h), (2, 32, 32), "3 tiles in a 2×2 grid");
    let rows = h / 16;
    assert!(columns * rows >= 3 && columns * (rows - 1) < 3);
    std::fs::remove_file(path).ok();
}

#[test]
fn preview_refuses_a_tile_larger_than_the_recipe() {
    let lua = lua();
    let path = crate::test_temp::dir().join("huge.png");
    lua.globals().set("OUT", path.to_str().unwrap()).unwrap();
    let script = format!(
        "local r = {RECIPE}; r.resolution = 32; return Texture.Preview(r, OUT, 4000000000)"
    );
    let err = lua.load(script).exec().unwrap_err().to_string();
    assert!(err.contains("Texture.Preview: size 4000000000"), "{err}");
    assert!(!path.exists(), "nothing written");
}

#[test]
fn preview_defaults_to_the_recipe_resolution_when_it_is_under_128() {
    let lua = lua();
    let path = crate::test_temp::dir().join("small.png");
    lua.globals().set("OUT", path.to_str().unwrap()).unwrap();
    let script = format!("local r = {RECIPE}; r.resolution = 8; return Texture.Preview(r, OUT)");
    lua.load(script).exec().unwrap();
    let (w, _) = image::GenericImageView::dimensions(&image::open(&path).unwrap());
    assert_eq!(w, 16, "2 columns of 8² tiles");
    std::fs::remove_file(path).ok();
}
