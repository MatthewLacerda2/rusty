//! No entity id is hashed, nor the order inputs are visited in: each input is its own chunk and
//! every kind's chunks are sorted by bytes before joining, so a scene whose ids were renumbered
//! still matches the file it was baked to.

use super::super::super::NavigationGraph;
use super::super::inputs::{collider_keys, obstacle_keys, BakeInputs, ColliderKey, Source};
use super::super::modifiers::modifier_keys;
use super::super::{raster, record_inputs};
use super::file::FORMAT;
use crate::offmesh::authored_keys;
use crate::NavMeshSettings;
use rusty_physics::collider_world_triangles;
use rusty_scene::Scene;

pub(super) fn read(scene: &Scene, raw: &NavigationGraph) -> (BakeInputs, u64) {
    let mut bytes = Vec::with_capacity(4096);
    bytes.extend(FORMAT.to_le_bytes());
    let settings = NavMeshSettings {
        areas: Vec::new(),
        ..scene.nav_settings.clone()
    };
    // serde_json writes an `f32` as its shortest round trip: the same text for the
    // same bits, on every platform.
    word(
        &mut bytes,
        serde_json::to_string(&settings)
            .unwrap_or_default()
            .as_bytes(),
    );
    let grid = [raw.min_x, raw.max_x, raw.min_z, raw.max_z, raw.grid_spacing];
    grid.iter()
        .for_each(|f| bytes.extend(f.to_bits().to_le_bytes()));
    let colliders = colliders(scene, raw, &mut bytes);
    let obstacles = obstacle_keys(scene);
    let modifiers = modifier_keys(scene);
    let links = authored_keys(scene);
    // `Debug` names every field and prints an `f32` as its shortest round trip, so a field
    // added later is hashed too.
    let text = |v: &dyn std::fmt::Debug| format!("{v:?}").into_bytes();
    join(&mut bytes, obstacles.iter().map(|(_, v)| text(v)).collect());
    join(&mut bytes, modifiers.iter().map(|(_, v)| text(v)).collect());
    join(&mut bytes, links.iter().map(|(_, k)| text(k)).collect());
    let keys = (obstacles, modifiers, links);
    let inputs = record_inputs(raw, &scene.nav_settings, colliders, keys);
    (inputs, rusty_core::hash::content(&bytes))
}

fn colliders(
    scene: &Scene,
    raw: &NavigationGraph,
    bytes: &mut Vec<u8>,
) -> Vec<Source<ColliderKey>> {
    let mut out = Vec::new();
    let mut chunks = Vec::new();
    for (id, key) in collider_keys(scene) {
        let mut chunk = Vec::new();
        match key.area {
            None => chunk.push(0),
            Some(a) => chunk.extend([1, a]),
        }
        let rect = match collider_world_triangles(scene, id) {
            None => {
                chunk.push(2);
                None
            }
            Some(m) => {
                chunk.push(u8::from(m.convex));
                chunk.extend((m.vertices.len() as u32).to_le_bytes());
                let coords = m.vertices.iter().flat_map(|v| v.to_array());
                coords.for_each(|f| chunk.extend(f.to_bits().to_le_bytes()));
                chunk.extend((m.triangles.len() as u32).to_le_bytes());
                let corners = m.triangles.iter().flatten();
                corners.for_each(|i| chunk.extend(i.to_le_bytes()));
                raster::footprint(raw, &m.vertices)
            }
        };
        chunks.push(chunk);
        out.push(Source { id, key, rect });
    }
    join(bytes, chunks);
    out
}

fn join(bytes: &mut Vec<u8>, mut chunks: Vec<Vec<u8>>) {
    chunks.sort_unstable();
    bytes.extend((chunks.len() as u32).to_le_bytes());
    chunks.iter().for_each(|c| word(bytes, c));
}

fn word(bytes: &mut Vec<u8>, text: &[u8]) {
    bytes.extend((text.len() as u32).to_le_bytes());
    bytes.extend(text);
}
