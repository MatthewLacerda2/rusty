use glam::Vec3;

use super::cone::{Bounds, Cone};
use super::footprint::{footprint, Footprint};
use super::grid::AabbCache;
use super::lists::{fill_lists, Pair};
use super::{ClusterGrid, LocalLight, CLUSTER_COUNT, GRID, MAX_CLUSTER_LIGHTS, MAX_VISIBLE_LIGHTS};
use rusty_components::{falloff_window_sq, FALLOFF_EPSILON_SQ};

#[derive(Debug, Default)]
pub(crate) struct Binned {
    pub ranges: Vec<[u32; 2]>,
    pub indices: Vec<u32>,
    pub visible: u32,
    pub culled: u32,
    /// Spheres in view but past the per-camera budget (the least important).
    pub dropped: u32,
    pub cluster_dropped: u32,
}

pub(crate) struct Budget<'a> {
    pub per_camera: usize,
    pub per_cluster: usize,
    pub brightness: Option<&'a [f32]>,
}

impl Budget<'_> {
    pub(crate) fn nearest(n: usize) -> Self {
        Self {
            per_camera: n,
            per_cluster: usize::MAX,
            brightness: None,
        }
    }

    fn view_score(&self, c: &Candidate) -> f32 {
        let d2 = c.center.length_squared();
        match self.brightness {
            Some(b) => b[c.item as usize] * c.radius * c.radius / (c.radius * c.radius + d2),
            None => c.radius - d2.sqrt(),
        }
    }

    fn cluster_score(&self, item: u32, d2: f32, radius: f32, size2: f32) -> f32 {
        self.brightness.map_or(0.0, |b| {
            b[item as usize] * falloff_window_sq(d2, radius) / d2.max(size2).max(FALLOFF_EPSILON_SQ)
        })
    }

    fn lists(&self, item: u32) -> bool {
        self.brightness.is_none_or(|b| b[item as usize] > 0.0)
    }
}

struct Candidate {
    item: u32,
    center: Vec3,
    radius: f32,
    tiles: [[u32; 2]; 2],
    slices: [u32; 2],
}

pub(crate) fn bin(grid: &ClusterGrid, lights: &[LocalLight], cache: &mut AabbCache) -> Binned {
    let bounds: Vec<_> = lights.iter().map(LocalLight::bounds).collect();
    let brightness: Vec<_> = lights.iter().map(LocalLight::brightness).collect();
    bin_bounds(grid, &bounds, &light_budget(&brightness), cache)
}

pub(crate) fn shadow_requests(
    grid: &ClusterGrid,
    lights: &[(LocalLight, bool)],
) -> Vec<(LocalLight, bool)> {
    let spheres: Vec<_> = lights.iter().map(|(l, _)| l.sphere()).collect();
    let brightness: Vec<_> = lights.iter().map(|(l, _)| l.brightness()).collect();
    let mut out: Vec<_> = lights.iter().map(|&(l, _)| (l, false)).collect();
    for c in camera_cut(grid, &spheres, &light_budget(&brightness)).0 {
        let i = c.item as usize;
        out[i].1 = lights[i].1;
    }
    out
}

fn light_budget(brightness: &[f32]) -> Budget<'_> {
    Budget {
        per_camera: MAX_VISIBLE_LIGHTS,
        per_cluster: MAX_CLUSTER_LIGHTS,
        brightness: Some(brightness),
    }
}

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

fn bin_bounds(
    grid: &ClusterGrid,
    bounds: &[Bounds],
    budget: &Budget,
    cache: &mut AabbCache,
) -> Binned {
    let spheres: Vec<_> = bounds.iter().map(|b| b.sphere).collect();
    let (candidates, culled, dropped) = camera_cut(grid, &spheres, budget);
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
        let Some((cone, (tiles, slices))) = reach(grid, c, &bounds[c.item as usize], budget) else {
            out.culled += 1;
            continue;
        };
        for k in slices[0]..=slices[1] {
            for j in tiles[1][0]..=tiles[1][1] {
                for i in tiles[0][0]..=tiles[0][1] {
                    let cluster = i + GRID[0] * (j + GRID[1] * k);
                    let (lo, hi) = aabbs[cluster as usize];
                    let d2 = c.center.clamp(lo, hi).distance_squared(c.center);
                    let in_cone = |k: &Cone| k.may_touch_box(lo, hi);
                    if d2 <= c.radius * c.radius && cone.as_ref().is_none_or(in_cone) {
                        let size2 = ((hi - lo) * 0.5).length_squared();
                        let score = budget.cluster_score(c.item, d2, c.radius, size2);
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

fn camera_cut(
    grid: &ClusterGrid,
    spheres: &[(Vec3, f32)],
    budget: &Budget,
) -> (Vec<Candidate>, u32, u32) {
    let mut candidates: Vec<Candidate> = (0..spheres.len() as u32)
        .filter_map(|i| candidate(grid, i, spheres[i as usize]))
        .collect();
    let culled = (spheres.len() - candidates.len()) as u32;
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

fn candidate(grid: &ClusterGrid, item: u32, (center, radius): (Vec3, f32)) -> Option<Candidate> {
    let center = grid.view.transform_point3(center);
    let (tiles, slices) = footprint(grid, center, radius)?;
    Some(Candidate {
        item,
        center,
        radius,
        tiles,
        slices,
    })
}

fn reach(
    grid: &ClusterGrid,
    c: &Candidate,
    bounds: &Bounds,
    budget: &Budget,
) -> Option<(Option<Cone>, Footprint)> {
    if !budget.lists(c.item) {
        return None;
    }
    let cone = bounds.cone.map(|k| k.transformed(grid.view));
    Some((cone, listed_box(grid, c, cone.as_ref())?))
}

fn listed_box(grid: &ClusterGrid, c: &Candidate, cone: Option<&Cone>) -> Option<Footprint> {
    let Some(cone) = cone else {
        return Some((c.tiles, c.slices));
    };
    let (center, radius) = cone.bounding_sphere();
    let (t, s) = footprint(grid, center, radius)?;
    let cut = |a: [u32; 2], b: [u32; 2]| [a[0].max(b[0]), a[1].min(b[1])];
    let tiles = [cut(c.tiles[0], t[0]), cut(c.tiles[1], t[1])];
    let slices = cut(c.slices, s);
    let empty = tiles.iter().chain([&slices]).any(|r| r[0] > r[1]);
    (!empty).then_some((tiles, slices))
}
