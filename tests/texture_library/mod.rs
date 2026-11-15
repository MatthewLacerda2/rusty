//! The engine's material library (`engine/textures/library/`): every graph there is
//! held to one interface and one bar, found by walking the folder, so a new material
//! is covered with nothing added here. Parts (`parts/`) are building blocks with no
//! fixed interface: each bakes, deterministically and seamlessly, and that is all.
//!
//! Cost: each material is baked five times at 256² (wear 0, 0.3 twice, 1, another
//! seed), the materials side by side on their own threads.

mod bar;
mod maps;

use std::path::PathBuf;

use rusty::core::project::{engine_dir, ContentRoots};
use rusty::edit::authoring::material::load_recipe;
use rusty::procgen::recipe::TextureRecipe;
use rusty::procgen::{runner::evaluate_many, Image};
use serde_json::{json, Value};

pub const RES: u32 = 256;
/// Every material's named outputs; `metallic` is the one a material may add.
pub const OUTPUTS: [&str; 5] = ["base_color", "normal", "roughness", "height", "occlusion"];

pub fn library() -> PathBuf {
    engine_dir().join("textures/library")
}

/// The `.graph.json` files directly in `dir`, by `engine/…` path, sorted.
pub fn graphs_in(dir: &std::path::Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .expect("the library folder exists")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".graph.json"))
        .map(|p| {
            let rel = p.strip_prefix(engine_dir()).expect("under engine/");
            format!("engine/{}", rel.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    found.sort();
    found
}

/// The graph file at an `engine/…` path, parsed as JSON.
pub fn doc(path: &str) -> Value {
    let file = engine_dir().join(path.trim_start_matches("engine/"));
    let text = std::fs::read_to_string(&file).expect("the graph reads");
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: not JSON: {e}"))
}

/// `path` instanced once at 256² with `params` and `seed`, each of `outputs` baked
/// (the graph's own `output` for `""`).
pub fn bake(path: &str, params: Value, seed: u64, outputs: &[&str]) -> Vec<Image> {
    // A declared input reads a tiling noise: a part's inputs are a height or a mask.
    let wired = doc(path)["inputs"].as_array().map_or(0, Vec::len);
    let probe = json!({ "id": "probe", "op": "noise", "kind": "fbm", "scale": 6, "octaves": 4 });
    let node = json!({ "id": "m", "op": "graph", "path": path, "params": params, "seed": seed,
        "inputs": vec!["probe"; wired] });
    // Each output named in the outer `outputs`, which the expansion rewrites to an
    // inner node id; `""` is the graph's own output.
    let named: serde_json::Map<String, Value> = outputs
        .iter()
        .map(|o| {
            let key = if o.is_empty() { "self" } else { o };
            let at = if o.is_empty() {
                "m".into()
            } else {
                format!("m.{o}")
            };
            (key.to_string(), Value::String(at))
        })
        .collect();
    let outer = json!({ "resolution": RES, "nodes": [probe, node], "outputs": named });
    let recipe: TextureRecipe = serde_json::from_value(outer).expect("the instance parses");
    let roots = ContentRoots::new(&crate::temp::dir());
    let loaded = load_recipe(&recipe, &roots).unwrap_or_else(|e| panic!("{path}: {e}"));
    let keys = outputs
        .iter()
        .map(|o| if o.is_empty() { "self" } else { o });
    let targets: Vec<&str> = keys.map(|k| loaded.recipe.outputs[k].as_str()).collect();
    evaluate_many(&loaded.recipe, &loaded.sources, &targets)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn every_library_material_meets_the_bar() {
    let materials = graphs_in(&library());
    assert!(
        !materials.is_empty(),
        "engine/textures/library holds no graph"
    );
    let failures: Vec<String> = std::thread::scope(|s| {
        let runs: Vec<_> = materials
            .iter()
            .map(|path| s.spawn(move || bar::material(path)))
            .collect();
        runs.into_iter()
            .flat_map(|r| r.join().unwrap_or_else(|_| vec!["panicked".into()]))
            .collect()
    });
    assert!(
        failures.is_empty(),
        "the library misses its bar:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_part_bakes_deterministically_and_tiles() {
    for path in graphs_in(&library().join("parts")) {
        let a = bake(&path, json!({}), 0, &[""]).remove(0);
        let b = bake(&path, json!({}), 0, &[""]).remove(0);
        assert!(a.pixels() == b.pixels(), "{path}: two bakes differ");
        if let Some(seam) = maps::seam(&a) {
            panic!("{path}: {seam}");
        }
    }
}
