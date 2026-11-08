//! Tests for reading a recipe's graph files (#1744): nested ones followed, the
//! extension required, the project's graphs found, and a material's rebake
//! reading an edited graph.

use serde_json::{json, Value};

use super::*;
use crate::scene::authoring::material::{load_recipe, rebake_maps, MaterialLibrary};
use crate::scene::MaterialAsset;

/// A graph whose output is a flat `color`.
fn flat(color: [f32; 4]) -> Value {
    json!({ "resolution": 4, "nodes": [{ "id": "c", "op": "constant", "color": color }] })
}

fn project(test: &str, files: &[(&str, Value)]) -> std::path::PathBuf {
    let root = crate::test_temp::dir().join(test);
    for (path, doc) in files {
        let file = root.join(native(path));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, doc.to_string()).unwrap();
    }
    root
}

fn instancing(path: &str) -> TextureRecipe {
    let node = json!({ "id": "g", "op": "graph", "path": path });
    serde_json::from_value(
        json!({ "resolution": 4, "nodes": [node], "outputs": { "albedo": "g" } }),
    )
    .unwrap()
}

#[test]
fn nested_graphs_are_read_and_expanded_before_the_bake() {
    let outer = json!({ "resolution": 4,
        "nodes": [{ "id": "inner", "op": "graph", "path": "textures/flat.graph.json" }] });
    let root = project(
        "rusty_graphs_nested_1744",
        &[
            ("textures/outer.graph.json", outer),
            ("textures/flat.graph.json", flat([0.0, 1.0, 0.0, 1.0])),
        ],
    );
    let loaded = load_recipe(&instancing("textures/outer.graph.json"), &root).unwrap();
    let ids: Vec<&str> = loaded.recipe.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["g/inner/c"]);
    assert_eq!(loaded.recipe.outputs["albedo"], "g/inner/c");
}

#[test]
fn only_a_graph_file_is_instanced_and_a_missing_one_is_named() {
    let root = project("rusty_graphs_ext_1744", &[("plain.json", flat([1.0; 4]))]);
    let err = load_recipe(&instancing("plain.json"), &root).unwrap_err();
    assert!(
        err.contains("'g'") && err.contains(".graph.json file"),
        "{err}"
    );
    let err = load_recipe(&instancing("gone.graph.json"), &root).unwrap_err();
    assert!(err.contains("\"gone.graph.json\""), "{err}");
}

#[test]
fn the_project_graphs_are_found_outside_hidden_and_build_folders() {
    let doc = flat([1.0; 4]);
    let root = project(
        "rusty_graphs_list_1744",
        &[
            ("a.graph.json", doc.clone()),
            ("textures/b.graph.json", doc.clone()),
            (".cache/c.graph.json", doc.clone()),
            ("target/d.graph.json", doc),
        ],
    );
    assert_eq!(
        project_graphs(&root),
        ["a.graph.json", "textures/b.graph.json"]
    );
}

#[test]
fn a_rebake_reads_the_edited_graph_and_the_asset_keeps_the_instance() {
    let path = "textures/tint.graph.json";
    let root = project(
        "rusty_graphs_rebake_1744",
        &[(path, flat([1.0, 0.0, 0.0, 1.0]))],
    );
    let mut materials = MaterialLibrary::default();
    let asset = MaterialAsset {
        maps_recipe: Some(instancing(path)),
        ..MaterialAsset::default()
    };
    materials.insert("tint".into(), asset);
    let pixel = |materials: &MaterialLibrary| {
        let map = materials["tint"].base_color_map.clone().unwrap();
        image::open(map.file().unwrap())
            .unwrap()
            .to_rgba8()
            .get_pixel(0, 0)
            .0
    };
    rebake_maps(&mut materials, "tint", None, &root, &root).unwrap();
    assert_eq!(pixel(&materials), [255, 0, 0, 255]);
    std::fs::write(
        root.join(native(path)),
        flat([0.0, 0.0, 1.0, 1.0]).to_string(),
    )
    .unwrap();
    rebake_maps(&mut materials, "tint", None, &root, &root).unwrap();
    assert_eq!(pixel(&materials), [0, 0, 255, 255]);
    assert_eq!(materials["tint"].maps_recipe, Some(instancing(path)));
}
