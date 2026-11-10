//! The occlusion bake's report (#1980): the grid, the objects baked with their
//! triangles (the biggest occluders first), and how well the finished PVS culls:
//! the mean and most objects a cell sees, the share culled, and the objects every
//! cell sees. All of it is read from the bake's report and its PVS, outside the
//! bake: the time per cell and the slowest cells need hooks inside it, and stay
//! `null` until they have them (`slowestCells`, #1987).

use serde_json::{json, Value};

use super::{entity_name, Kind, Report, Stages, TOP};
use crate::scene::lighting::occlusion::{
    OcclusionBakeReport, OcclusionData, CELL_SIZE, MAX_CELL, OCCLUSION_SEED,
};
use crate::scene::Scene;

/// The report of a bake of `scene` that reported `r`, baked `data` and wrote
/// `output_bytes`.
pub fn report(
    scene: &Scene,
    (r, data): (&OcclusionBakeReport, Option<&OcclusionData>),
    stages: &Stages,
    output_bytes: u64,
) -> Report {
    let settings = json!({
        "seed": OCCLUSION_SEED,
        "cellSize": CELL_SIZE,
        "maxCell": MAX_CELL,
    });
    let fingerprint = data.map_or(0, fingerprint);
    Report::new(Kind::Occlusion, stages, settings, fingerprint, output_bytes).with(json!({
        "cells": r.cells,
        "cellSize": r.cell_size,
        "tooLarge": r.too_large,
        "rows": r.rows,
        "dataBytes": r.bytes,
        "objects": r.objects,
        "occluders": r.occluders,
        "neverCulled": r.never_culled,
        "occluderTriangles": data.map_or(0, |d| occluder_triangles(scene, d).0),
        "biggestOccluders": data.map_or(json!([]), |d| occluder_triangles(scene, d).1),
        "visibility": data.map_or(Value::Null, |d| visibility(scene, d)),
        "slowestCells": Value::Null,
    }))
}

/// Write the report of an occlusion bake now on `scene` (#1980): it reported `r`
/// and took `ms`, and writing its file beside `path` took `saved` (`None` when
/// the file waits for a scene save, so nothing is written yet).
pub fn keep(
    scene: &Scene,
    path: &str,
    (r, ms): (&OcclusionBakeReport, u64),
    saved: Option<std::time::Duration>,
) {
    let mut stages = Stages::start();
    stages.push("bake", std::time::Duration::from_millis(ms));
    let mut written = 0;
    if let Some(took) = saved {
        stages.push("write", took);
        written = super::bytes_under(&crate::scene::lighting::occlusion::occlusion_path(path));
    }
    let data = scene.occlusion.as_deref();
    let report = report(scene, (r, data), &stages, written);
    super::keep(path, report);
}

/// What the PVS was baked against: every object's entity and fingerprint.
fn fingerprint(data: &OcclusionData) -> u64 {
    let mut hash = crate::core::hash::Content::default();
    for o in &data.objects {
        hash.write(&o.entity.to_le_bytes());
        hash.write(&o.fingerprint.to_le_bytes());
    }
    hash.finish()
}

/// A baked object's drawn triangles in the scene as it is.
fn triangles(scene: &Scene, entity: u32) -> usize {
    scene.world.mesh(entity).map_or(0, |m| m.indices.len() / 3)
}

/// The occluders' triangles added up, and the [`TOP`] biggest (ties to the lower id).
fn occluder_triangles(scene: &Scene, data: &OcclusionData) -> (usize, Value) {
    let occluders = data.objects.iter().filter(|o| o.occluder);
    let mut weighed: Vec<(u32, usize)> = occluders
        .map(|o| (o.entity, triangles(scene, o.entity)))
        .collect();
    let total = weighed.iter().map(|w| w.1).sum();
    weighed.sort_by_key(|&(id, n)| (std::cmp::Reverse(n), id));
    let top = weighed
        .iter()
        .take(TOP)
        .map(|&(id, n)| json!({ "entity": id, "name": entity_name(scene, id), "triangles": n }));
    (total, Value::Array(top.collect()))
}

/// How many objects each cell sees: the mean and the most, the share of the
/// objects culled on average, and the objects every cell sees (at most [`TOP`]
/// named; the count holds them all).
pub(super) fn visibility(scene: &Scene, data: &OcclusionData) -> Value {
    let (rows, cells) = data.parts();
    let objects = data.objects.len();
    let words = objects.div_ceil(64);
    if words == 0 || cells.is_empty() {
        return Value::Null;
    }
    // How many cells use each row: one pass over the rows then counts every cell.
    let mut uses = vec![0u64; rows.len() / words];
    for &row in cells {
        uses[row as usize] += 1;
    }
    let (mut seen_by, mut sum, mut most) = (vec![0u64; objects], 0u64, 0u32);
    for (row, bits) in rows.chunks_exact(words).enumerate() {
        let n: u32 = bits.iter().map(|w| w.count_ones()).sum();
        if uses[row] == 0 {
            continue;
        }
        (sum, most) = (sum + n as u64 * uses[row], most.max(n));
        // Only the set bits: a cell sees a fraction of a big level.
        for (w, &word) in bits.iter().enumerate() {
            let mut word = word;
            while word != 0 {
                let i = w * 64 + word.trailing_zeros() as usize;
                if let Some(seen) = seen_by.get_mut(i) {
                    *seen += uses[row];
                }
                word &= word - 1;
            }
        }
    }
    let mean = sum as f64 / cells.len() as f64;
    let every = (data.objects.iter().zip(&seen_by)).filter(|(_, &n)| n == cells.len() as u64);
    let every: Vec<u32> = every.map(|(o, _)| o.entity).collect();
    let named = every.iter().take(TOP).map(|&id| entity_name(scene, id));
    json!({
        "meanVisible": (mean * 100.0).round() / 100.0,
        "maxVisible": most,
        "culledFraction": ((1.0 - mean / objects as f64) * 1000.0).round() / 1000.0,
        "alwaysVisible": every.len(),
        "alwaysVisibleNames": named.collect::<Vec<_>>(),
    })
}
