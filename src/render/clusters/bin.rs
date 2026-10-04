//! CPU binning (#434): which lights, and which decals (#638), touch which clusters
//! of one camera. Both are bounding spheres to the binner.

use glam::Vec3;

use super::grid::AabbCache;
use super::{ClusterGrid, LocalLight, CLUSTER_COUNT, GRID, MAX_VISIBLE_LIGHTS};

/// One camera's binned spheres (lights or decals): per cluster an `[offset, count]`
/// into `indices`, the flat list of indices (into the frame's light or decal
/// array), and the tallies.
#[derive(Debug, Default)]
pub(crate) struct Binned {
    pub ranges: Vec<[u32; 2]>,
    pub indices: Vec<u32>,
    /// Spheres binned into at least one cluster.
    pub visible: u32,
    /// Spheres outside the camera's view: never binned, so they cost nothing.
    pub culled: u32,
    /// Spheres in view but past the budget (the farthest ones).
    pub dropped: u32,
}

/// A sphere in view space, with the clusters its bounding box may touch.
struct Candidate {
    item: u32,
    center: Vec3,
    radius: f32,
    tiles: [[u32; 2]; 2],
    slices: [u32; 2],
}

/// Bin `lights` into `grid`'s clusters, the nearest [`MAX_VISIBLE_LIGHTS`] of them;
/// `cache` keeps the cluster boxes per lens.
pub(crate) fn bin(grid: &ClusterGrid, lights: &[LocalLight], cache: &mut AabbCache) -> Binned {
    let spheres: Vec<_> = lights.iter().map(LocalLight::sphere).collect();
    bin_spheres(grid, &spheres, MAX_VISIBLE_LIGHTS, cache)
}

/// Bin `spheres` (centre, radius) into `grid`'s clusters, the nearest `budget` of
/// them. Each cluster lists its spheres in ascending index order.
pub(crate) fn bin_spheres(
    grid: &ClusterGrid,
    spheres: &[(Vec3, f32)],
    budget: usize,
    cache: &mut AabbCache,
) -> Binned {
    let mut candidates: Vec<Candidate> = (0..spheres.len() as u32)
        .filter_map(|i| candidate(grid, i, spheres[i as usize]))
        .collect();
    let mut out = Binned {
        ranges: vec![[0, 0]; CLUSTER_COUNT],
        culled: (spheres.len() - candidates.len()) as u32,
        ..Default::default()
    };
    if candidates.len() > budget {
        // Nearest surface first; the index breaks ties, so the cut is deterministic.
        let key = |c: &Candidate| c.center.length() - c.radius;
        candidates.sort_by(|a, b| key(a).total_cmp(&key(b)).then(a.item.cmp(&b.item)));
        out.dropped = (candidates.len() - budget) as u32;
        candidates.truncate(budget);
        candidates.sort_by_key(|c| c.item);
    }
    if candidates.is_empty() {
        return out;
    }
    let aabbs = cache.get(grid);
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    for c in &candidates {
        let before = pairs.len();
        for k in c.slices[0]..=c.slices[1] {
            for j in c.tiles[1][0]..=c.tiles[1][1] {
                for i in c.tiles[0][0]..=c.tiles[0][1] {
                    let cluster = i + GRID[0] * (j + GRID[1] * k);
                    let (lo, hi) = aabbs[cluster as usize];
                    if c.center.clamp(lo, hi).distance_squared(c.center) <= c.radius * c.radius {
                        pairs.push((cluster, c.item));
                    }
                }
            }
        }
        if pairs.len() > before {
            out.visible += 1;
        } else {
            out.culled += 1;
        }
    }
    fill_lists(&mut out, &pairs);
    out
}

/// Counting-sort `(cluster, item)` pairs into per-cluster ranges; stable, so each
/// cluster lists its items in ascending order.
fn fill_lists(out: &mut Binned, pairs: &[(u32, u32)]) {
    for &(cluster, _) in pairs {
        out.ranges[cluster as usize][1] += 1;
    }
    let mut offset = 0;
    for range in &mut out.ranges {
        range[0] = offset;
        offset += range[1];
    }
    let mut cursor: Vec<u32> = out.ranges.iter().map(|r| r[0]).collect();
    out.indices = vec![0; pairs.len()];
    for &(cluster, item) in pairs {
        let at = &mut cursor[cluster as usize];
        out.indices[*at as usize] = item;
        *at += 1;
    }
}

/// The sphere in view space and the tile/slice box it projects to, or `None` when
/// it lies wholly outside the frustum (or has no reach).
fn candidate(grid: &ClusterGrid, item: u32, (center, radius): (Vec3, f32)) -> Option<Candidate> {
    if radius <= 0.0 {
        return None;
    }
    let center = grid.view.transform_point3(center);
    let depth = -center.z;
    if depth + radius < grid.near || depth - radius > grid.far {
        return None;
    }
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for corner in 0..8 {
        let sign = |bit: u32| if corner & bit == 0 { -1.0 } else { 1.0 };
        let p = center + Vec3::new(sign(1), sign(2), sign(4)) * radius;
        let ndc = grid.ndc(p);
        (lo, hi) = (lo.min(ndc), hi.max(ndc));
    }
    if hi.x < -1.0 || hi.y < -1.0 || lo.x > 1.0 || lo.y > 1.0 {
        return None;
    }
    let tile =
        |ndc: f32, n: u32| (((ndc * 0.5 + 0.5) * n as f32).floor().max(0.0) as u32).min(n - 1);
    let slice = |d: f32| (grid.slice_of(d).floor().max(0.0) as u32).min(GRID[2] - 1);
    Some(Candidate {
        item,
        center,
        radius,
        tiles: [
            [tile(lo.x, GRID[0]), tile(hi.x, GRID[0])],
            [tile(lo.y, GRID[1]), tile(hi.y, GRID[1])],
        ],
        slices: [slice(depth - radius), slice((depth + radius).min(grid.far))],
    })
}
