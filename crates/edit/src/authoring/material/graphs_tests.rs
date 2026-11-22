use serde_json::{json, Value};

use super::*;
use crate::authoring::material::{load_recipe, rebake_maps, MaterialLibrary};
use rusty_scene::MaterialAsset;

fn flat(color: [f32; 4]) -> Value {
    json!({ "resolution": 4, "nodes": [{ "id": "c", "op": "constant", "color": color }] })
}

fn project(test: &str, files: &[(&str, Value)]) -> std::path::PathBuf {
    let root = rusty_test_support::temp::dir().join(test);
    for (path, doc) in files {
        let file = root.join(path);
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
    let loaded = load_recipe(
        &instancing("textures/outer.graph.json"),
        &ContentRoots::new(&root),
    )
    .unwrap();
    let ids: Vec<&str> = loaded.recipe.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["g/inner/c"]);
    assert_eq!(loaded.recipe.outputs["albedo"], "g/inner/c");
}

#[test]
fn only_a_graph_file_is_instanced_and_a_missing_one_is_named() {
    let root = project("rusty_graphs_ext_1744", &[("plain.json", flat([1.0; 4]))]);
    let err = load_recipe(&instancing("plain.json"), &ContentRoots::new(&root)).unwrap_err();
    assert!(
        err.contains("'g'") && err.contains(".graph.json file"),
        "{err}"
    );
    let err = load_recipe(&instancing("gone.graph.json"), &ContentRoots::new(&root)).unwrap_err();
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
            ("target/d.graph.json", doc.clone()),
            ("levels/target/e.graph.json", doc),
        ],
    );
    assert_eq!(
        project_graphs(&root),
        [
            "a.graph.json",
            "levels/target/e.graph.json",
            "textures/b.graph.json"
        ]
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
    std::fs::write(root.join(path), flat([0.0, 0.0, 1.0, 1.0]).to_string()).unwrap();
    rebake_maps(&mut materials, "tint", None, &root, &root).unwrap();
    assert_eq!(pixel(&materials), [0, 0, 255, 255]);
    assert_eq!(materials["tint"].maps_recipe, Some(instancing(path)));
}

fn with_engine(
    test: &str,
    project_files: &[(&str, Value)],
    engine_files: &[(&str, Value)],
) -> ContentRoots {
    let base = rusty_test_support::temp::dir().join(test);
    let roots = ContentRoots::with_engine(&base.join("game"), &base.join("engine"));
    for (root, files) in [
        (&roots.project, project_files),
        (&roots.engine, engine_files),
    ] {
        for (path, doc) in files {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, doc.to_string()).unwrap();
        }
    }
    std::fs::create_dir_all(&roots.project).unwrap();
    roots
}

const RUST: &str = "engine/textures/library/rust.graph.json";

#[test]
fn an_engine_graph_is_read_from_the_engine_folder_nested_ones_too_and_nothing_warns() {
    let outer = json!({ "resolution": 4,
        "nodes": [{ "id": "inner", "op": "graph", "path": "engine/textures/library/flat.graph.json" }] });
    let roots = with_engine(
        "rusty_graphs_engine_2185",
        &[(RUST, flat([1.0, 0.0, 0.0, 1.0]))],
        &[
            ("textures/library/rust.graph.json", outer),
            (
                "textures/library/flat.graph.json",
                flat([0.0, 1.0, 0.0, 1.0]),
            ),
        ],
    );
    let loaded = load_recipe(&instancing(RUST), &roots).unwrap();
    assert_eq!(loaded.recipe.outputs["albedo"], "g/inner/c");
    let image = rusty_procgen::evaluate_with(&loaded.recipe, &loaded.sources).unwrap();
    assert_eq!(
        image.pixels()[0],
        [0.0, 1.0, 0.0, 1.0],
        "the engine's file, not the project's"
    );
    let outside =
        |path: &str| crate::authoring::material::sources::warn_if_outside(&roots, "graph", path);
    assert!(!outside(RUST));
    assert!(outside(
        "engine/../game/engine/textures/library/rust.graph.json"
    ));
}

#[test]
fn a_missing_engine_graph_is_an_error_naming_the_node_and_the_path() {
    let roots = with_engine("rusty_graphs_engine_missing_2185", &[], &[]);
    let err = load_recipe(
        &instancing("engine/textures/library/gone.graph.json"),
        &roots,
    )
    .unwrap_err();
    assert!(
        err.contains("'g'") && err.contains("\"engine/textures/library/gone.graph.json\""),
        "{err}"
    );
}

#[test]
fn an_engine_instance_bakes_as_its_nodes_pasted_by_hand_and_keeps_its_spelling() {
    let graph = json!({ "resolution": 8, "nodes": [
        { "id": "n", "op": "noise", "kind": "fbm", "scale": 4, "octaves": 3 },
        { "id": "i", "op": "invert", "inputs": ["n"] } ] });
    let roots = with_engine(
        "rusty_graphs_engine_bake_2185",
        &[],
        &[("textures/library/rust.graph.json", graph.clone())],
    );
    let mut by_hand = graph;
    by_hand["outputs"] = json!({ "albedo": "i" });
    by_hand["resolution"] = json!(4);
    let by_hand: TextureRecipe = serde_json::from_value(by_hand).unwrap();
    let instanced = instancing(RUST);
    let bake = |recipe: &TextureRecipe| {
        let loaded = load_recipe(recipe, &roots).unwrap();
        rusty_procgen::evaluate_with(&loaded.recipe, &loaded.sources)
            .unwrap()
            .pixels()
            .to_vec()
    };
    assert_eq!(bake(&instanced), bake(&by_hand));
    assert!(instanced
        .to_json()
        .unwrap()
        .contains(&format!("\"{RUST}\"")));
}

#[test]
fn the_engine_graphs_are_listed_after_the_projects_by_their_engine_path() {
    let doc = flat([1.0; 4]);
    let roots = with_engine(
        "rusty_graphs_engine_list_2185",
        &[
            ("a.graph.json", doc.clone()),
            ("engine/shadowed.graph.json", doc.clone()),
        ],
        &[
            ("textures/library/rust.graph.json", doc.clone()),
            ("materials/b.graph.json", doc.clone()),
            ("target/t.graph.json", doc),
        ],
    );
    let want = [
        "a.graph.json",
        "engine/materials/b.graph.json",
        "engine/target/t.graph.json",
        RUST,
    ];
    assert_eq!(listed_graphs(&roots), want);
}
