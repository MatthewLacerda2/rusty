//! #1741's flood fill in a graph: thresholded Voronoi cells, each its own flat
//! grey, one value across the tile's edge, mostly the same grey at 128² and
//! 1024², and thousands of islands kept apart.

use std::collections::BTreeMap;

use rusty::procgen::recipe::TextureRecipe;
use rusty::procgen::{evaluate, Image};

/// Voronoi cells at `scale`, shrunk off their edges into islands, a grey each;
/// `cells` names each cell by its own colour. The gap between islands is wide
/// enough (a few pixels at the coarsest resolution a test bakes) that no two
/// cells meet through a corner.
fn recipe(scale: u32, resolution: u32, output: &str) -> TextureRecipe {
    // 16 px per cell at 128² and 8 at 512²: a gap of about 3 px each way.
    let threshold = if scale > 8 { 0.4 } else { 0.2 };
    let json = format!(
        r#"{{ "resolution": {resolution}, "seed": 5, "output": "{output}", "nodes": [
          {{ "id": "edges", "op": "voronoi", "scale": {scale}, "output": "edges" }},
          {{ "id": "cells", "op": "voronoi", "scale": {scale}, "output": "cells" }},
          {{ "id": "fill", "op": "flood_fill", "threshold": {threshold}, "inputs": ["edges"] }},
          {{ "id": "grey", "op": "flood_random", "seed": 2, "inputs": ["fill"] }}
        ] }}"#
    );
    serde_json::from_str(&json).expect("recipe parses")
}

/// Per Voronoi cell (keyed by its colour's bits), the greys its island pixels read.
fn greys_by_cell(scale: u32, resolution: u32) -> BTreeMap<u32, Vec<f32>> {
    let bake = |output| evaluate(&recipe(scale, resolution, output)).expect("bakes");
    let (cells, fill, grey): (Image, Image, Image) = (bake("cells"), bake("fill"), bake("grey"));
    let mut by_cell: BTreeMap<u32, Vec<f32>> = BTreeMap::new();
    for ((c, f), g) in cells.pixels().iter().zip(fill.pixels()).zip(grey.pixels()) {
        if f[3] > 0.5 {
            let greys = by_cell.entry(c[0].to_bits()).or_default();
            if !greys.contains(&g[0]) {
                greys.push(g[0]);
            }
        }
    }
    by_cell
}

/// How many cells read one flat grey, and how many distinct greys they read.
fn flat_and_distinct(by_cell: &BTreeMap<u32, Vec<f32>>) -> (usize, usize) {
    let flat = by_cell.values().filter(|g| g.len() == 1).count();
    let mut greys: Vec<u32> = by_cell.values().map(|g| g[0].to_bits()).collect();
    greys.sort_unstable();
    greys.dedup();
    (flat, greys.len())
}

#[test]
fn each_voronoi_cell_reads_its_own_flat_grey() {
    let by_cell = greys_by_cell(8, 128);
    assert!(by_cell.len() >= 60, "{} cells", by_cell.len());
    assert_eq!(flat_and_distinct(&by_cell), (by_cell.len(), by_cell.len()));
}

#[test]
fn thousands_of_islands_stay_apart() {
    let by_cell = greys_by_cell(64, 512);
    let (flat, distinct) = flat_and_distinct(&by_cell);
    assert!(by_cell.len() > 3000, "{} cells", by_cell.len());
    // A thin cell the threshold pinches in two is two islands, so two greys.
    assert!(
        flat * 100 >= by_cell.len() * 99,
        "{flat} of {} flat",
        by_cell.len()
    );
    // Every island draws its own grey: only chance collisions of a 24-bit value.
    assert!(
        distinct * 1000 >= by_cell.len() * 999,
        "{distinct} of {}",
        by_cell.len()
    );
}

#[test]
fn most_cells_keep_their_grey_across_resolutions() {
    let (lo, hi) = (greys_by_cell(8, 128), greys_by_cell(8, 1024));
    let same = lo.iter().filter(|(c, g)| hi.get(c) == Some(g)).count();
    assert!(same * 10 >= lo.len() * 8, "{same} of {}", lo.len());
}
