//! What a saved navmesh was baked from (#1887), as the occlusion bake's fingerprint
//! (`scene::lighting::occlusion::gather::fingerprint`) is what its PVS was: one hash
//! of everything the bake reads and nothing time- or path-dependent. The static
//! colliders' world triangles (the very ones `raster` clips, so a mesh collider's
//! vertices count, not only its bounds), the carving obstacles, the modifier volumes,
//! the authored links (each with its entity id), the bake settings without the area
//! costs (read at search time, `NavMeshSettings::bakes_like`), the grid, and the
//! file's format.
//!
//! The same pass records the bake's inputs (`BakeInputs`) with the cells each
//! covers, so a restored graph rebakes incrementally exactly as a baked one does.

use super::super::super::NavigationGraph;
use super::super::inputs::{collider_keys, obstacle_keys, sources, BakeInputs, Source};
use super::super::modifiers::modifier_keys;
use super::super::{link_rect, raster};
use super::file::FORMAT;
use crate::navigation::offmesh::authored_keys;
use crate::navigation::NavMeshSettings;
use crate::physics::collider_world_triangles;
use crate::scene::Scene;

/// The inputs a bake of `scene` on `raw`'s grid reads, and their fingerprint.
/// `g` is the graph the authored links snap onto (the same grid).
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
    let mut colliders = Vec::new();
    for (id, key) in collider_keys(scene) {
        bytes.extend(id.to_le_bytes());
        let tris = collider_world_triangles(scene, id);
        let rect = tris.as_ref().and_then(|m| {
            bytes.push(u8::from(m.convex));
            bytes.extend((m.vertices.len() as u32).to_le_bytes());
            let coords = m.vertices.iter().flat_map(|v| v.to_array());
            coords.for_each(|f| bytes.extend(f.to_bits().to_le_bytes()));
            bytes.extend((m.triangles.len() as u32).to_le_bytes());
            let corners = m.triangles.iter().flatten();
            corners.for_each(|i| bytes.extend(i.to_le_bytes()));
            raster::footprint(raw, &m.vertices)
        });
        if tris.is_none() {
            bytes.push(2);
        }
        colliders.push(Source { id, key, rect });
    }
    let obstacles = obstacle_keys(scene);
    let modifiers = modifier_keys(scene);
    let links = authored_keys(scene);
    // The rest are small plain values: their `Debug` text names every field, and an
    // `f32`'s is its shortest round trip, so a field added later is hashed too.
    for (id, v) in &obstacles {
        word(&mut bytes, format!("o{id}{v:?}").as_bytes());
    }
    for (id, v) in &modifiers {
        word(&mut bytes, format!("m{id}{v:?}").as_bytes());
    }
    for (id, k) in &links {
        word(&mut bytes, format!("l{id}{k:?}").as_bytes());
    }
    let inputs = BakeInputs {
        settings: scene.nav_settings.clone(),
        colliders,
        obstacles: sources(obstacles, |v| {
            raster::footprint(raw, &v.triangles().concat())
        }),
        modifiers: sources(modifiers, |v| v.footprint(raw)),
        links: sources(links, |k| link_rect(raw, k)),
    };
    (inputs, crate::core::hash::content(&bytes))
}

/// `text` as a length-prefixed word of the fingerprint.
fn word(bytes: &mut Vec<u8>, text: &[u8]) {
    bytes.extend((text.len() as u32).to_le_bytes());
    bytes.extend(text);
}
