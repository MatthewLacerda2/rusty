//! src/asset/clusters.rs — a big mesh split into clusters at import (#1968), the
//! culling half of UE5's Nanite without its virtualised geometry.
//!
//! A sub-mesh of at least [`MIN_TRIANGLES`] triangles is split, run by run, into
//! clusters of at most [`MAX_TRIANGLES`] triangles and [`MAX_VERTICES`] vertices
//! (meshoptimizer's `buildMeshlets`, through the `meshopt` bindings), and its
//! index list is **reordered** so each cluster's triangles sit together: the
//! renderer draws a cluster as a plain index range, and a frame that culls
//! nothing draws the same triangles in the same order as one that culls. A
//! cluster never crosses a material run ([`MaterialRange`]), so a run the
//! renderer draws on its own (a material slot, a lightmapped mesh's runs) is
//! a whole number of clusters.
//!
//! Each cluster carries its local bounds and meshoptimizer's normal cone (apex,
//! axis, cutoff): the GPU occlusion cull (`render::passes::hiz`) tests it by the
//! frustum, by whether every triangle in it faces away, and by the depth pyramid.
//!
//! Built once per parse, so the import cache (`asset::imported`) keeps it with the
//! model and an instance or a scene load builds nothing. A skinned mesh gets none:
//! its vertices move past any bind-pose box. Pure data, no GPU types.

use std::ops::Range;
use std::sync::Arc;

use super::material_range::MaterialRange;
use super::mesh_data::SubMesh;

/// The fewest triangles a mesh is split at. Below it the per-instance test the
/// whole mesh already gets is as good: a cluster test costs a draw per instance
/// and a pass per batch, which pays only when there are many clusters to drop.
pub const MIN_TRIANGLES: usize = 4096;
/// A cluster's most triangles: meshoptimizer's recommendation for GPU culling.
pub const MAX_TRIANGLES: usize = 124;
/// A cluster's most vertices.
pub const MAX_VERTICES: usize = 64;
/// How much meshoptimizer favours tight normal cones over full clusters.
const CONE_WEIGHT: f32 = 0.25;
/// The cutoff of a cluster whose triangles face too many ways for a cone: no
/// direction's dot reaches it, so it is never culled by facing.
pub const NO_CONE: f32 = 2.0;

/// One cluster: its index range in the reordered list, its local bounds, and the
/// cone every triangle's normal falls in.
#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    pub indices: Range<u32>,
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// The cone's apex and axis (unit): every triangle faces away from an eye
    /// where `dot(normalize(apex - eye), axis) >= cutoff`.
    pub apex: [f32; 3],
    pub axis: [f32; 3],
    /// [`NO_CONE`] when no cone holds them.
    pub cutoff: f32,
}

/// A mesh's clusters, in index order, covering every index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clusters {
    pub list: Vec<Cluster>,
}

/// Split `sub` into clusters, reordering its indices, when it is big enough and
/// not skinned ([module docs](self)).
pub fn apply(sub: &mut SubMesh) {
    if sub.skin.is_some() {
        return;
    }
    let positions: Vec<[f32; 3]> = sub.vertices.iter().map(|v| v.position).collect();
    sub.clusters = build(&positions, &mut sub.indices, &sub.ranges).map(Arc::new);
}

/// The clusters of `indices` over `positions`, each run of `runs` split on its
/// own, with `indices` reordered to match. `None`, and `indices` untouched, for a
/// mesh under [`MIN_TRIANGLES`], one whose runs don't cover its indices in order,
/// one naming a vertex it doesn't have, or one with a position that isn't finite.
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
            return None; // meshoptimizer dropped or added a triangle
        }
    }
    indices.copy_from_slice(&reordered);
    Some(Clusters { list })
}

/// Whether `runs` cover `0..count` in order, each a whole number of triangles.
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

/// Split one run's `source` indices into clusters, appending their reordered
/// indices to `out` and the clusters to `list`.
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
