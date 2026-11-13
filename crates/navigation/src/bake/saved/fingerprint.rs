//! What a saved navmesh was baked from (#1887), as the occlusion bake's fingerprint
//! (`scene::lighting::occlusion::gather::fingerprint`) is what its PVS was: one hash
//! of everything the bake reads and nothing time- or path-dependent. The static
//! colliders' world triangles (the very ones `raster` clips, so a mesh collider's
//! vertices count, not only its bounds) and the area a `NavMeshModifier` gives each
//! (#1959; an ignored one is not read at all), the carving obstacles, the modifier volumes,
//! the authored links' endpoints, the bake settings without the area costs (read at
//! search time, `NavMeshSettings::bakes_like`), the grid, and the file's format.
//!
//! **No entity id is hashed, and neither is the order inputs are visited in** (#2127):
//! each input is hashed as its own chunk, and every kind's chunks are sorted by their
//! bytes before they are joined. What the bake reads is order-free (`heightfield`
//! sorts the solids by content, `modifiers` resolves overlaps by area), so a scene
//! whose ids were renumbered (a `DontDestroyOnLoad` survivor moving an incoming id,
//! `scene::runtime::load::make_room`) still matches the file it was baked to. The
//! authored links need no id: they are re-snapped onto the restored spans.
//!
//! The same pass records the bake's inputs (`BakeInputs`) with the cells each
//! covers, through the builder a full bake uses (`bake::record_inputs`), so a
//! restored graph rebakes incrementally exactly as a baked one does.

use super::super::super::NavigationGraph;
use super::super::inputs::{collider_keys, obstacle_keys, BakeInputs, ColliderKey, Source};
use super::super::modifiers::modifier_keys;
use super::super::{raster, record_inputs};
use super::file::FORMAT;
use crate::offmesh::authored_keys;
use crate::NavMeshSettings;
use rusty_physics::collider_world_triangles;
use rusty_scene::Scene;

/// The inputs a bake of `scene` on `raw`'s grid reads, and their fingerprint.
/// Authored links enter only by their endpoints; `restore` re-snaps them.
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
    // The rest are small plain values: their `Debug` text names every field, and an
    // `f32`'s is its shortest round trip, so a field added later is hashed too.
    let text = |v: &dyn std::fmt::Debug| format!("{v:?}").into_bytes();
    join(&mut bytes, obstacles.iter().map(|(_, v)| text(v)).collect());
    join(&mut bytes, modifiers.iter().map(|(_, v)| text(v)).collect());
    join(&mut bytes, links.iter().map(|(_, k)| text(k)).collect());
    let keys = (obstacles, modifiers, links);
    let inputs = record_inputs(raw, &scene.nav_settings, colliders, keys);
    (inputs, rusty_core::hash::content(&bytes))
}

/// The static colliders' records, each with its cells on `raw`'s grid, hashing
/// each one's area and world triangles onto `bytes`, one chunk each ([`join`]).
fn colliders(
    scene: &Scene,
    raw: &NavigationGraph,
    bytes: &mut Vec<u8>,
) -> Vec<Source<ColliderKey>> {
    let mut out = Vec::new();
    let mut chunks = Vec::new();
    for (id, key) in collider_keys(scene) {
        let mut chunk = Vec::new();
        // A `NavMeshModifier`'s override (#1959), tagged so area 0 doesn't read as no override.
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

/// One kind of input's `chunks`, sorted by their bytes so neither an entity id nor
/// the order they were read in reaches the fingerprint, as words after their count.
fn join(bytes: &mut Vec<u8>, mut chunks: Vec<Vec<u8>>) {
    chunks.sort_unstable();
    bytes.extend((chunks.len() as u32).to_le_bytes());
    chunks.iter().for_each(|c| word(bytes, c));
}

/// `text` as a length-prefixed word of the fingerprint.
fn word(bytes: &mut Vec<u8>, text: &[u8]) {
    bytes.extend((text.len() as u32).to_le_bytes());
    bytes.extend(text);
}
