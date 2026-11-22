//! A sub-mesh's indices are **reordered** so each cluster is a plain index range, and no
//! cluster crosses a [`MaterialRange`]. A skinned mesh gets none: its vertices move past
//! any bind-pose box.

use std::ops::Range;
use std::sync::Arc;

use super::material_range::MaterialRange;
use super::mesh_data::SubMesh;

/// Below it the whole-mesh instance test culls as well, and costs no extra draw.
pub const MIN_TRIANGLES: usize = 4096;
// meshoptimizer's recommended pair for GPU culling.
pub const MAX_TRIANGLES: usize = 124;
pub const MAX_VERTICES: usize = 64;
const CONE_WEIGHT: f32 = 0.25;
/// Cutoff of a cluster too wide for a cone: no dot reaches it, so it is never culled by facing.
pub const NO_CONE: f32 = 2.0;
/// Equals `render::passes::hiz::clusters::plan::MAX_OUT_INDICES`; a longer run could never be
/// drawn by cluster.
pub const MAX_RUN_INDICES: usize = 1 << 23;

#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    pub indices: Range<u32>,
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// The cone's apex and axis (unit): every triangle faces away from an eye
    /// where `dot(normalize(apex - eye), axis) >= cutoff`.
    pub apex: [f32; 3],
    pub axis: [f32; 3],
    pub cutoff: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clusters {
    pub list: Vec<Cluster>,
}

pub fn apply(sub: &mut SubMesh) {
    if sub.skin.is_some() {
        return;
    }
    let positions: Vec<[f32; 3]> = sub.vertices.iter().map(|v| v.position).collect();
    sub.clusters = build(&positions, sub.indices.edit(), &sub.ranges).map(Arc::new);
}

pub fn build(
    positions: &[[f32; 3]],
    indices: &mut [u32],
    runs: &[MaterialRange],
) -> Option<Clusters> {
    let runs: Vec<Range<u32>> = match runs.is_empty() {
        true => std::iter::once(0..indices.len() as u32).collect(),
        false => runs.iter().map(|r| r.indices.clone()).collect(),
    };
    let fits = indices.len() / 3 >= MIN_TRIANGLES
        && indices.len().is_multiple_of(3)
        && covers(&runs, indices.len() as u32)
        && runs.iter().all(|r| r.len() <= MAX_RUN_INDICES)
        && indices.iter().all(|&i| (i as usize) < positions.len())
        && positions.iter().flatten().all(|c| c.is_finite());
    if !fits {
        return None;
    }
    let bytes: Vec<u8> = positions
        .iter()
        .flatten()
        .flat_map(|c| c.to_ne_bytes())
        .collect();
    let adapter = meshopt::VertexDataAdapter::new(&bytes, 12, 0).ok()?;
    let mut list = Vec::new();
    let mut reordered = Vec::with_capacity(indices.len());
    for run in &runs {
        let source = &indices[run.start as usize..run.end as usize];
        split_run(source, (positions, &adapter), &mut reordered, &mut list);
        if reordered.len() != run.end as usize {
            return None;
        }
    }
    indices.copy_from_slice(&reordered);
    Some(Clusters { list })
}

fn covers(runs: &[Range<u32>], count: u32) -> bool {
    let mut at = 0;
    for run in runs {
        if run.start != at || run.end < run.start || !(run.end - run.start).is_multiple_of(3) {
            return false;
        }
        at = run.end;
    }
    at == count
}

fn split_run(
    source: &[u32],
    (positions, adapter): (&[[f32; 3]], &meshopt::VertexDataAdapter),
    out: &mut Vec<u32>,
    list: &mut Vec<Cluster>,
) {
    if source.is_empty() {
        return;
    }
    let meshlets =
        meshopt::build_meshlets(source, adapter, MAX_VERTICES, MAX_TRIANGLES, CONE_WEIGHT);
    for meshlet in meshlets.iter() {
        let start = out.len() as u32;
        let local = meshlet
            .triangles
            .iter()
            .map(|&t| meshlet.vertices[t as usize]);
        out.extend(local);
        let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in meshlet.vertices.iter().map(|&v| positions[v as usize]) {
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let bounds = meshopt::compute_meshlet_bounds(meshlet, adapter);
        let cone = bounds.cone_cutoff < 1.0 && bounds.cone_axis.iter().all(|a| a.is_finite());
        list.push(Cluster {
            indices: start..out.len() as u32,
            min,
            max,
            apex: bounds.cone_apex,
            axis: bounds.cone_axis,
            cutoff: if cone { bounds.cone_cutoff } else { NO_CONE },
        });
    }
}

#[cfg(test)]
#[path = "clusters_tests.rs"]
mod tests;
