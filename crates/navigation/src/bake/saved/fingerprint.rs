//! No entity id or visiting order is hashed: chunks are hashed sorted. A tile's fingerprint covers
//! the inputs within its halo and the part of that halo inside the bounds, in world cells, so it
//! holds under other bounds and changes when they cut the halo differently.

use super::super::super::offmesh::LinkParams;
use super::super::super::{NavigationGraph, TileKey};
use super::super::erosion;
use super::super::inputs::{collider_keys, obstacle_keys, BakeInputs, ColliderKey, Source};
use super::super::modifiers::modifier_keys;
use super::super::region::CellRect;
use super::super::{raster, record_inputs};
use super::file::FORMAT;
use crate::offmesh::authored_keys;
use crate::NavMeshSettings;
use rusty_core::hash::content;
use rusty_physics::collider_world_triangles;
use rusty_scene::Scene;
use std::collections::BTreeMap;

pub(super) struct Prints {
    pub whole: u64,
    pub tiles: BTreeMap<TileKey, u64>,
}

pub(super) fn read(scene: &Scene, raw: &NavigationGraph) -> (BakeInputs, Prints) {
    let mut head = Vec::with_capacity(256);
    head.extend(FORMAT.to_le_bytes());
    let settings = NavMeshSettings {
        areas: Vec::new(),
        bounds: None,
        ..scene.nav_settings.clone()
    };
    // serde_json and `Debug` print an `f32` as its shortest round trip, on every platform.
    let text = serde_json::to_string(&settings).unwrap_or_default();
    word(&mut head, text.as_bytes());
    head.extend(raw.grid_spacing.to_bits().to_le_bytes());
    head.extend(raw.tile_side().to_le_bytes());
    let mut chunks: Vec<(u64, Option<CellRect>)> = Vec::new();
    let colliders = colliders(scene, raw, &mut chunks);
    let obstacles = obstacle_keys(scene);
    let modifiers = modifier_keys(scene);
    let links = authored_keys(scene);
    let keys = (obstacles, modifiers, links);
    let inputs = record_inputs(raw, &scene.nav_settings, colliders, keys);
    let debug = |tag: u8, v: &dyn std::fmt::Debug| {
        content(&[&[tag][..], format!("{v:?}").as_bytes()].concat())
    };
    for s in &inputs.obstacles {
        chunks.push((debug(1, &s.key), s.rect));
    }
    for s in &inputs.modifiers {
        chunks.push((debug(2, &s.key), s.rect));
    }
    let mut whole = head.clone();
    let mut all: Vec<u64> = chunks.iter().map(|c| c.0).collect();
    all.extend(inputs.links.iter().map(|s| debug(3, &s.key)));
    join(&mut whole, all);
    let tiles = tile_prints(
        raw,
        &head,
        &chunks,
        halo(&scene.nav_settings, raw.grid_spacing),
    );
    let prints = Prints {
        whole: content(&whole),
        tiles,
    };
    (inputs, prints)
}

/// The cells past a tile whose spans its erosion or its links read.
fn halo(s: &NavMeshSettings, spacing: f32) -> i32 {
    let links = LinkParams::new(s, spacing).map_or(0, |p| p.halo());
    erosion::reach(s.agent_radius, spacing) + links
}

fn tile_prints(
    raw: &NavigationGraph,
    head: &[u8],
    chunks: &[(u64, Option<CellRect>)],
    halo: i32,
) -> BTreeMap<TileKey, u64> {
    let mut near: BTreeMap<TileKey, Vec<u64>> = BTreeMap::new();
    for &(hash, rect) in chunks {
        let Some(r) = rect.and_then(|r| r.grown(halo).clamped(raw)) else {
            continue;
        };
        for key in raw.tiles.keys_over(r) {
            near.entry(key).or_default().push(hash);
        }
    }
    let origin = raw.tiles.origin;
    near.into_iter()
        .filter_map(|(key, hashes)| {
            raw.tiles.rect(key)?;
            // Erosion reads the grid's edge as a wall.
            let r = raw.tiles.span(key).grown(halo).clamped(raw)?;
            let r = r.shifted(origin.0, origin.1);
            let mut bytes = head.to_vec();
            [key.0, key.1, r.x0, r.z0, r.x1, r.z1]
                .iter()
                .for_each(|v| bytes.extend(v.to_le_bytes()));
            join(&mut bytes, hashes);
            Some((key, content(&bytes)))
        })
        .collect()
}

fn colliders(
    scene: &Scene,
    raw: &NavigationGraph,
    chunks: &mut Vec<(u64, Option<CellRect>)>,
) -> Vec<Source<ColliderKey>> {
    let mut out = Vec::new();
    for (id, key) in collider_keys(scene) {
        let mut chunk = vec![0];
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
        chunks.push((content(&chunk), rect));
        out.push(Source { id, key, rect });
    }
    out
}

fn join(bytes: &mut Vec<u8>, mut hashes: Vec<u64>) {
    hashes.sort_unstable();
    bytes.extend((hashes.len() as u32).to_le_bytes());
    hashes.iter().for_each(|h| bytes.extend(h.to_le_bytes()));
}

fn word(bytes: &mut Vec<u8>, text: &[u8]) {
    bytes.extend((text.len() as u32).to_le_bytes());
    bytes.extend(text);
}
