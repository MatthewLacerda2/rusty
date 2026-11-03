//! CPU binning (#434): which lights, and which decals (#638), touch which clusters
//! of one camera. Both are bounding spheres to the binner, a spotlight's trimmed by
//! its cone (#1712); they differ only in their [`Budget`].

use glam::Vec3;

use super::cone::{Bounds, Cone};
use super::grid::AabbCache;
use super::lists::{fill_lists, Pair};
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
    /// Spheres outside the camera's view, or lights that contribute nothing
    /// (#1712): never binned, so they cost nothing.
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

/// A sphere in view space, with the clusters its bounding box may touch, and the
/// cone in view space that trims it, for a spotlight.
struct Candidate {
    item: u32,
    center: Vec3,
    radius: f32,
    cone: Option<Cone>,
    tiles: [[u32; 2]; 2],
    slices: [u32; 2],
}

/// Bin `lights` into `grid`'s clusters: the [`MAX_VISIBLE_LIGHTS`] that contribute
/// most to the view, at most [`MAX_CLUSTER_LIGHTS`] per cluster (#834). A light
/// that contributes nothing (#1712) is left out, counted as culled; it keeps its
/// place in the array, so its index (and its shadow tiles) still resolve. `cache`
/// keeps the cluster boxes per lens.
pub(crate) fn bin(grid: &ClusterGrid, lights: &[LocalLight], cache: &mut AabbCache) -> Binned {
    let bounds: Vec<_> = lights
        .iter()
        .map(|l| match l.contributes() {
            true => l.bounds(),
            false => Bounds {
                sphere: (Vec3::from(l.position), 0.0),
                cone: None,
            },
        })
        .collect();
    let brightness: Vec<_> = lights.iter().map(LocalLight::brightness).collect();
    bin_bounds(grid, &bounds, &light_budget(&brightness), cache)
}

/// `lights`, each with whether it wants a shadow seen through `grid` (#873): it
/// asks for one and survives the camera's cut, so [`bin`] keeps it and it is
/// shaded. The shadow atlas plans these alone, so a light the budget drops never
/// wins tiles it would not use.
pub(crate) fn shadow_requests(
    grid: &ClusterGrid,
    lights: &[(LocalLight, bool)],
) -> Vec<(LocalLight, bool)> {
    let bounds: Vec<_> = lights.iter().map(|(l, _)| l.bounds()).collect();
    let brightness: Vec<_> = lights.iter().map(|(l, _)| l.brightness()).collect();
    let mut out: Vec<_> = lights.iter().map(|&(l, _)| (l, false)).collect();
    for c in camera_cut(grid, &bounds, &light_budget(&brightness)).0 {
        let i = c.item as usize;
        out[i].1 = lights[i].1;
    }
    out
}

/// The lights' budget (#834): [`MAX_VISIBLE_LIGHTS`] per camera and
/// [`MAX_CLUSTER_LIGHTS`] per cluster, ranked by `brightness`.
fn light_budget(brightness: &[f32]) -> Budget<'_> {
    Budget {
        per_camera: MAX_VISIBLE_LIGHTS,
        per_cluster: MAX_CLUSTER_LIGHTS,
        brightness: Some(brightness),
    }
}

/// Bin `spheres` (centre, radius) into `grid`'s clusters within `budget`: the
/// decals' path (#638), [`bin_bounds`] with no cones.
pub(crate) fn bin_spheres(
    grid: &ClusterGrid,
    spheres: &[(Vec3, f32)],
    budget: &Budget,
    cache: &mut AabbCache,
) -> Binned {
    let bounds: Vec<_> = spheres
        .iter()
        .map(|&sphere| Bounds { sphere, cone: None })
        .collect();
    bin_bounds(grid, &bounds, budget, cache)
}

/// Bin `bounds` into `grid`'s clusters within `budget`: a cluster lists one when
/// its box meets the sphere and, for a cone, the cone may touch the box's bounding
/// sphere (#1712). Each cluster lists its items in ascending index order; every cut
/// is deterministic, the index breaking ties.
fn bin_bounds(
    grid: &ClusterGrid,
    bounds: &[Bounds],
    budget: &Budget,
    cache: &mut AabbCache,
) -> Binned {
    let (candidates, culled, dropped) = camera_cut(grid, bounds, budget);
    let mut out = Binned {
        ranges: vec![[0, 0]; CLUSTER_COUNT],
        culled,
        dropped,
        ..Default::default()
    };
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
                    let in_cone = |cone: &Cone| cone.may_reach_box(lo, hi);
                    if d2 <= c.radius * c.radius && c.cone.as_ref().is_none_or(in_cone) {
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
    fill_lists(&mut out, &pairs, budget.per_cluster);
    out
}

/// The spheres in `grid`'s view that `budget`'s per-camera cut keeps, in index
/// order, with how many lay outside the view and how many the cut dropped.
fn camera_cut(
    grid: &ClusterGrid,
    bounds: &[Bounds],
    budget: &Budget,
) -> (Vec<Candidate>, u32, u32) {
    let mut candidates: Vec<Candidate> = (0..bounds.len() as u32)
        .filter_map(|i| candidate(grid, i, bounds[i as usize]))
        .collect();
    let culled = (bounds.len() - candidates.len()) as u32;
    let mut dropped = 0;
    if candidates.len() > budget.per_camera {
        let key = |c: &Candidate| budget.view_score(c);
        candidates.sort_by(|a, b| key(b).total_cmp(&key(a)).then(a.item.cmp(&b.item)));
        dropped = (candidates.len() - budget.per_camera) as u32;
        candidates.truncate(budget.per_camera);
        candidates.sort_by_key(|c| c.item);
    }
    (candidates, culled, dropped)
}

/// The sphere in view space and the tile/slice box it projects to, or `None` when
/// it lies wholly outside the frustum (or has no reach, a NaN radius included). A
/// spot's box is also cut to its range sphere's (#1712), so it never visits a
/// cluster the range sphere would not.
fn candidate(grid: &ClusterGrid, item: u32, bounds: Bounds) -> Option<Candidate> {
    let (center, radius) = bounds.sphere;
    let center = grid.view.transform_point3(center);
    let (mut tiles, mut slices) = footprint(grid, center, radius)?;
    let cone = bounds.cone.map(|c| c.transformed(grid.view));
    if let Some(cone) = &cone {
        let (t, s) = footprint(grid, cone.apex, cone.range)?;
        for axis in 0..2 {
            tiles[axis] = [
                tiles[axis][0].max(t[axis][0]),
                tiles[axis][1].min(t[axis][1]),
            ];
        }
        slices = [slices[0].max(s[0]), slices[1].min(s[1])];
    }
    Some(Candidate {
        item,
        center,
        radius,
        cone,
        tiles,
        slices,
    })
}

/// A cluster range: the tiles across and up (`[first, last]` each) and the slices.
type Footprint = ([[u32; 2]; 2], [u32; 2]);

/// The tiles and depth slices a view-space sphere may touch, or
/// `None` when it lies wholly outside the frustum or has no reach.
fn footprint(grid: &ClusterGrid, center: Vec3, radius: f32) -> Option<Footprint> {
    if !(radius > 0.0) {
        return None;
    }
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
    Some((
        [
            [tile(lo.x, GRID[0]), tile(hi.x, GRID[0])],
            [tile(lo.y, GRID[1]), tile(hi.y, GRID[1])],
        ],
        [slice(depth - radius), slice((depth + radius).min(grid.far))],
    ))
}
