//! `Texture.BakeMeshMaps` end to end (#1849): a model file in, PNGs out, read back
//! by a recipe's `image` op.

use std::path::{Path, PathBuf};

use mlua::Lua;

use super::*;

/// An open L as an `.obj`: a floor quad and a wall quad, each in its own half of UV0.
const L_OBJ: &str = "\
v 0 0 0\nv 0 0 1\nv 1 0 1\nv 1 0 0\nv 0 1 0\nv 0 1 1\n\
vt 0.05 0.05\nvt 0.45 0.05\nvt 0.45 0.95\nvt 0.05 0.95\n\
vt 0.55 0.05\nvt 0.95 0.05\nvt 0.95 0.95\nvt 0.55 0.95\n\
vn 0 1 0\nvn 1 0 0\n\
f 1/1/1 2/2/1 3/3/1\nf 1/1/1 3/3/1 4/4/1\n\
f 1/5/2 5/6/2 6/7/2\nf 1/5/2 6/7/2 2/8/2\n";

/// A fresh folder holding the L as `l.obj`.
fn project(name: &str) -> (PathBuf, String) {
    let dir = crate::test_temp::dir().join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let model = dir.join("l.obj");
    std::fs::write(&model, L_OBJ).unwrap();
    (dir, model.to_str().unwrap().to_string())
}

fn all() -> Vec<String> {
    [
        "curvature",
        "ao",
        "world_normal",
        "position",
        "thickness",
        "id",
    ]
    .map(String::from)
    .to_vec()
}

#[test]
fn every_map_is_written_beside_the_prefix_in_a_new_folder() {
    let (dir, model) = project("mesh_maps_all");
    let prefix = format!("{}/maps/l_", dir.to_str().unwrap());
    let written = bake_mesh_maps(&model, &all(), &prefix, 16, &dir).unwrap();
    assert_eq!(written.len(), 6);
    for (map, path) in &written {
        assert_eq!(path, &format!("{prefix}{map}.png"));
        let img = image::open(path).expect("a map decodes");
        assert_eq!(image::GenericImageView::dimensions(&img), (16, 16), "{map}");
    }
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_prefix_ending_in_a_slash_gets_its_folder_made() {
    let (dir, model) = project("mesh_maps_slash");
    let prefix = format!("{}/maps/kolvik/", dir.to_str().unwrap());
    let written = bake_mesh_maps(&model, &["id".to_string()], &prefix, 8, &dir).unwrap();
    assert!(Path::new(&written["id"]).is_file(), "{written:?}");
    assert!(written["id"].ends_with("/maps/kolvik/id.png"));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_bad_call_names_the_verb_and_the_model() {
    let (dir, model) = project("mesh_maps_bad");
    let prefix = format!("{}/l_", dir.to_str().unwrap());
    let bad =
        |model: &str, maps: &[String]| bake_mesh_maps(model, maps, &prefix, 8, &dir).unwrap_err();
    let e = bad(&model, &["dirt".to_string()]);
    assert!(
        e.starts_with(&format!("Texture.BakeMeshMaps: {model}:")),
        "{e}"
    );
    assert!(e.contains("\"dirt\""), "{e}");
    assert!(bad(&model, &[]).contains("no maps"));
    assert!(bad(&format!("{model}::Nope"), &all()).contains("no sub-object \"Nope\""));
    assert!(bad(&format!("{model}.missing.obj"), &all()).contains("missing"));
    assert!(
        !Path::new(&format!("{prefix}ao.png")).exists(),
        "nothing written"
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_recipe_reads_a_baked_map_through_the_image_op() {
    let (dir, model) = project("mesh_maps_recipe");
    let lua = Lua::new();
    super::super::register(&lua).unwrap();
    lua.globals().set("MODEL", model).unwrap();
    lua.globals()
        .set("PREFIX", format!("{}/l_", dir.to_str().unwrap()))
        .unwrap();
    lua.globals()
        .set("OUT", dir.join("wear.png").to_str().unwrap())
        .unwrap();
    let script = r#"
        local maps = Texture.BakeMeshMaps(MODEL, { "curvature", "ao" }, PREFIX, 32)
        return Texture.Bake({ resolution = 32, nodes = {
          { id = "c", op = "image", path = maps.curvature, encoding = "linear" },
        } }, OUT, "data")
    "#;
    let out: String = lua.load(script).eval().unwrap();
    let img = image::open(&out).unwrap().to_luma16();
    // The floor's texel beside the wall sits in the concave crease: darker than 0.5.
    // u ≈ 0.39, v ≈ 0.92 (the .obj flips v): the floor, by its x = 0 edge.
    let (x, y) = (12, 29);
    assert!(img.get_pixel(x, y)[0] < 30_000, "{:?}", img.get_pixel(x, y));
    std::fs::remove_dir_all(dir).ok();
}
