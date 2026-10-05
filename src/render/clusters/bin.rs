//! CPU binning (#434): which lights, and which decals (#638), touch which clusters
//! of one camera. Both are bounding spheres to the binner; they differ only in their
//! [`Budget`].

use glam::Vec3;

use super::grid::AabbCache;
use super::{ClusterGrid, LocalLight, CLUSTER_COUNT, GRID, MAX_CLUSTER_LIGHTS, MAX_VISIBLE_LIGHTS};

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
    /// Spheres in view but past the per-camera budget (the least important).
    pub dropped: u32,
    /// (cluster, sphere) entries cut by the per-cluster budget (#834).
    pub cluster_dropped: u32,
}

/// What one binning keeps when more is in view than it shades.
pub(crate) struct Budget<'a> {
    /// The most spheres the camera keeps.
    pub per_camera: usize,
    /// The most spheres one cluster lists.
    pub per_cluster: usize,
    /// Each sphere's brightness (a light's intensity × colour luminance, #834), to
    /// rank by estimated contribution; `None` ranks by nearest surface instead.
    pub brightness: Option<&'a [f32]>,
}

impl Budget<'_> {
    /// Keep the `n` spheres whose surfaces are nearest the camera, with no cap per
    /// cluster: the decals' budget (#638).
    pub(crate) fn nearest(n: usize) -> Self {
        Self {
            per_camera: n,
            per_cluster: usize::MAX,
            brightness: None,
        }
    }

    /// How much sphere `c` matters to the whole view; higher wins. A light scores
    /// its brightness times `r² / (r² + d²)` — the share of the view its reach
    /// covers, saturating to 1 once the camera is inside it — so a bright light
    /// across the room beats a dim one beside the camera.
    fn view_score(&self, c: &Candidate) -> f32 {
        let d2 = c.center.length_squared();
        match self.brightness {
            Some(b) => b[c.item as usize] * c.radius * c.radius / (c.radius * c.radius + d2),
            None => c.radius - d2.sqrt(),
        }
    }

    /// How much sphere `item` matters to one cluster whose nearest point lies
    /// `d2` (squared) from its centre: the most it delivers there, with the shader's
    /// `1 / (d² + 1)` falloff.
    fn cluster_score(&self, item: u32, d2: f32) -> f32 {
        self.brightness
            .map_or(0.0, |b| b[item as usize] / (d2 + 1.0))
    }
}

/// One cluster's entry for one sphere, and how much it matters there.
struct Pair {
    cluster: u32,
    item: u32,
    score: f32,
}

/// A sphere in view space, with the clusters its bounding box may touch.
struct Candidate {
    item: u32,
    center: Vec3,
    radius: f32,
    tiles: [[u32; 2]; 2],
    slices: [u32; 2],
}

/// Bin `lights` into `grid`'s clusters: the [`MAX_VISIBLE_LIGHTS`] that contribute
/// most to the view, at most [`MAX_CLUSTER_LIGHTS`] per cluster (#834). `cache`
/// keeps the cluster boxes per lens.
pub(crate) fn bin(grid: &ClusterGrid, lights: &[LocalLight], cache: &mut AabbCache) -> Binned {
    let spheres: Vec<_> = lights.iter().map(LocalLight::sphere).collect();
    let brightness: Vec<_> = lights.iter().map(LocalLight::brightness).collect();
    let budget = Budget {
        per_camera: MAX_VISIBLE_LIGHTS,
        per_cluster: MAX_CLUSTER_LIGHTS,
        brightness: Some(&brightness),
    };
    bin_spheres(grid, &spheres, &budget, cache)
}

/// Bin `spheres` (centre, radius) into `grid`'s clusters within `budget`. Each
/// cluster lists its spheres in ascending index order; every cut is deterministic,
/// the index breaking ties.
pub(crate) fn bin_spheres(
    grid: &ClusterGrid,
    spheres: &[(Vec3, f32)],
    budget: &Budget,
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
    if candidates.len() > budget.per_camera {
        let key = |c: &Candidate| budget.view_score(c);
        candidates.sort_by(|a, b| key(b).total_cmp(&key(a)).then(a.item.cmp(&b.item)));
        out.dropped = (candidates.len() - budget.per_camera) as u32;
        candidates.truncate(budget.per_camera);
        candidates.sort_by_key(|c| c.item);
    }
    if candidates.is_empty() {
        return out;
    }
    let aabbs = cache.get(grid);
    let mut pairs: Vec<Pair> = Vec::new();
    for c in &candidates {
        let before = pairs.len();
        for k in c.slices[0]..=c.slices[1] {
            for j in c.tiles[1][0]..=c.tiles[1][1] {
                for i in c.tiles[0][0]..=c.tiles[0][1] {
                    let cluster = i + GRID[0] * (j + GRID[1] * k);
                    let (lo, hi) = aabbs[cluster as usize];
                    let d2 = c.center.clamp(lo, hi).distance_squared(c.center);
                    if d2 <= c.radius * c.radius {
                        let score = budget.cluster_score(c.item, d2);
                        pairs.push(Pair {
                            cluster,
                            item: c.item,
                            score,
                        });
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
    out.cluster_dropped = cut_clusters(&mut pairs, budget.per_cluster);
    fill_lists(&mut out, &pairs);
    out
}

/// Counting-sort the pairs into per-cluster ranges; stable, so each cluster lists
/// its items in ascending order.
fn fill_lists(out: &mut Binned, pairs: &[Pair]) {
    for p in pairs {
        out.ranges[p.cluster as usize][1] += 1;
    }
    let mut offset = 0;
    for range in &mut out.ranges {
        range[0] = offset;
        offset += range[1];
    }
    let mut cursor: Vec<u32> = out.ranges.iter().map(|r| r[0]).collect();
    out.indices = vec![0; pairs.len()];
    for p in pairs {
        let at = &mut cursor[p.cluster as usize];
        out.indices[*at as usize] = p.item;
        *at += 1;
    }
}

/// Cut every cluster listing more than `cap` items down to the `cap` that score
/// highest there, the index breaking ties; returns the entries cut. The survivors
/// keep each cluster's ascending order. Free when no cluster is over.
fn cut_clusters(pairs: &mut Vec<Pair>, cap: usize) -> u32 {
    if pairs.len() <= cap {
        return 0;
    }
    let mut counts = vec![0usize; CLUSTER_COUNT];
    pairs.iter().for_each(|p| counts[p.cluster as usize] += 1);
    if counts.iter().all(|&n| n <= cap) {
        return 0;
    }
    let before = pairs.len();
    pairs.sort_by(|a, b| {
        let by_score = b.score.total_cmp(&a.score).then(a.item.cmp(&b.item));
        a.cluster.cmp(&b.cluster).then(by_score)
    });
    let mut run = (u32::MAX, 0);
    pairs.retain(|p| {
        run = if run.0 == p.cluster {
            (p.cluster, run.1 + 1)
        } else {
            (p.cluster, 1)
        };
        run.1 <= cap
    });
    pairs.sort_by_key(|p| (p.cluster, p.item));
    (before - pairs.len()) as u32
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
