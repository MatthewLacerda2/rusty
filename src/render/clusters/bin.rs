//! CPU light binning (#434): which lights touch which clusters of one camera.

use glam::Vec3;

use super::grid::AabbCache;
use super::{ClusterGrid, LocalLight, CLUSTER_COUNT, GRID, MAX_VISIBLE_LIGHTS};

/// One camera's binned lights: per cluster an `[offset, count]` into `indices`, the
/// flat list of light indices (into the frame's light array), and the tallies.
#[derive(Debug, Default)]
pub(crate) struct Binned {
    pub ranges: Vec<[u32; 2]>,
    pub indices: Vec<u32>,
    /// Lights binned into at least one cluster.
    pub visible: u32,
    /// Lights outside the camera's view: never binned, so they cost nothing.
    pub culled: u32,
    /// Lights in view but past [`MAX_VISIBLE_LIGHTS`] (the farthest ones).
    pub dropped: u32,
}

/// A light in view space, with the clusters its bounding box may touch.
struct Candidate {
    light: u32,
    center: Vec3,
    radius: f32,
    tiles: [[u32; 2]; 2],
    slices: [u32; 2],
}

/// Bin `lights` into `grid`'s clusters; `cache` keeps the cluster boxes per lens.
pub(crate) fn bin(grid: &ClusterGrid, lights: &[LocalLight], cache: &mut AabbCache) -> Binned {
    let mut candidates: Vec<Candidate> = (0..lights.len() as u32)
        .filter_map(|i| candidate(grid, i, lights[i as usize].sphere()))
        .collect();
    let mut out = Binned {
        ranges: vec![[0, 0]; CLUSTER_COUNT],
        culled: (lights.len() - candidates.len()) as u32,
        ..Default::default()
    };
    if candidates.len() > MAX_VISIBLE_LIGHTS {
        // Nearest surface first; the index breaks ties, so the cut is deterministic.
        let key = |c: &Candidate| c.center.length() - c.radius;
        candidates.sort_by(|a, b| key(a).total_cmp(&key(b)).then(a.light.cmp(&b.light)));
        out.dropped = (candidates.len() - MAX_VISIBLE_LIGHTS) as u32;
        candidates.truncate(MAX_VISIBLE_LIGHTS);
        candidates.sort_by_key(|c| c.light);
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
                        pairs.push((cluster, c.light));
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

/// Counting-sort `(cluster, light)` pairs into per-cluster ranges; stable, so each
/// cluster lists its lights in ascending order.
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
    for &(cluster, light) in pairs {
        let at = &mut cursor[cluster as usize];
        out.indices[*at as usize] = light;
        *at += 1;
    }
}

/// The light's view-space sphere and the tile/slice box it projects to, or `None`
/// when it lies wholly outside the frustum (or has no reach).
fn candidate(grid: &ClusterGrid, light: u32, (center, radius): (Vec3, f32)) -> Option<Candidate> {
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
        light,
        center,
        radius,
        tiles: [
            [tile(lo.x, GRID[0]), tile(hi.x, GRID[0])],
            [tile(lo.y, GRID[1]), tile(hi.y, GRID[1])],
        ],
        slices: [slice(depth - radius), slice((depth + radius).min(grid.far))],
    })
}
