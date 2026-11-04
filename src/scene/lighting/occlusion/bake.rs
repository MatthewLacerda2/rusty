//! The occlusion bake (#1714): which baked objects each cell may see, by rays from
//! seeded points in the cell to seeded points on each object's triangles, through
//! the lightmap bake's BVH (`lightmap::Rays`), then dilated over each cell's
//! neighbours and folded into shared rows.
//!
//! A pure function of the scene and [`OCCLUSION_SEED`]: each cell draws from its own
//! stream and is baked alone, so the result is the same at any worker count.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use glam::Vec3;

use super::gather::{Face, Fingerprints, Object};
use super::{BakedObject, Grid, OcclusionData};
use crate::scene::lighting::lightmap::{Rays, Rng, Tri};
use crate::scene::Scene;

/// The bake's seed: one fixed stream, so a re-bake of an unchanged scene writes
/// the same bytes.
pub const OCCLUSION_SEED: u64 = 0x5056_5321;

/// Points sampled in each cell (its centre first, the rest seeded in the box).
const CELL_POINTS: usize = 6;
/// Points sampled on each object's triangles, area-weighted, beside its extremes.
const OBJECT_POINTS: usize = 8;
/// How far short of a target point a ray stops, so the triangle it lies on (or
/// one touching it) does not hide it.
const REACH_SLACK: f32 = 1e-3;

/// What one occlusion bake found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OcclusionBakeReport {
    /// Static meshes baked as occludees.
    pub objects: usize,
    /// Of them, the ones that occlude.
    pub occluders: usize,
    pub cells: usize,
    /// Distinct visible sets the cells share.
    pub rows: usize,
    /// Bytes of the stored data.
    pub bytes: usize,
}

/// Bake `scene`'s PVS on `threads` workers. `None` when there is nothing to cull:
/// no static mesh, or none that occludes.
pub fn bake_occlusion(
    scene: &Scene,
    threads: usize,
) -> (Option<OcclusionData>, OcclusionBakeReport) {
    let prints = Fingerprints::of_scene(scene);
    let mut ids = scene.world.ids_with_mesh();
    ids.sort_unstable();
    let objects: Vec<Object> = ids
        .iter()
        .filter_map(|&id| prints.object(scene, id))
        .collect();
    let occluders = objects.iter().filter(|o| !o.occluding.is_empty()).count();
    let mut report = OcclusionBakeReport {
        objects: objects.len(),
        occluders,
        ..Default::default()
    };
    if occluders == 0 {
        return (None, report);
    }
    let scene = Baking::of(&objects);
    let grid = Grid::over(scene.bounds.0, scene.bounds.1);
    let seen = scene.cells(&grid, threads.max(1));
    let (rows, cells) = fold_rows(&grid, &seen, objects.len().div_ceil(64));
    let baked = objects.iter().map(|o| BakedObject {
        entity: o.entity,
        fingerprint: o.fingerprint,
        occluder: !o.occluding.is_empty(),
    });
    let data = OcclusionData::new(grid, baked.collect(), rows, cells).expect("rows fit the grid");
    report.cells = grid.cell_count();
    report.rows = data.row_count();
    report.bytes = super::encode(&data).len();
    (Some(data), report)
}

/// The scene ready to sample: the occluders' BVH and each triangle's drawn side,
/// and per object its bounds and target points.
struct Baking {
    rays: Rays,
    faces: Vec<Face>,
    targets: Vec<(Vec3, Vec3, Vec<Vec3>)>,
    bounds: (Vec3, Vec3),
}

impl Baking {
    fn of(objects: &[Object]) -> Self {
        let (mut tris, mut faces) = (Vec::new(), Vec::new());
        let mut bounds = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let mut targets = Vec::with_capacity(objects.len());
        for (i, object) in objects.iter().enumerate() {
            for &([a, b, c], face) in &object.occluding {
                tris.push(Tri::new(a, b, c, i as u32, 0));
                faces.push(face);
            }
            let corners = object.drawn.iter().flatten();
            let min = corners
                .clone()
                .fold(Vec3::splat(f32::MAX), |m, &p| m.min(p));
            let max = corners.fold(Vec3::splat(f32::MIN), |m, &p| m.max(p));
            bounds = (bounds.0.min(min), bounds.1.max(max));
            targets.push((min, max, target_points(&object.drawn, i as u64)));
        }
        Self {
            rays: Rays::build(tris),
            faces,
            targets,
            bounds,
        }
    }

    /// Each cell's own visible bits, `words` per cell, baked on `threads` workers.
    fn cells(&self, grid: &Grid, threads: usize) -> Vec<Vec<u64>> {
        let count = grid.cell_count();
        let next = AtomicUsize::new(0);
        let out = Mutex::new(vec![Vec::new(); count]);
        std::thread::scope(|s| {
            for _ in 0..threads.min(count) {
                s.spawn(|| loop {
                    let cell = next.fetch_add(1, Ordering::Relaxed);
                    if cell >= count {
                        break;
                    }
                    let bits = self.cell(grid, cell);
                    out.lock().unwrap_or_else(|e| e.into_inner())[cell] = bits;
                });
            }
        });
        out.into_inner().unwrap_or_else(|e| e.into_inner())
    }

    /// What cell `index` sees on its own: an object whose bounds reach the cell or
    /// its neighbours outright, any other when a sampled ray reaches it.
    fn cell(&self, grid: &Grid, index: usize) -> Vec<u64> {
        let (min, max) = grid.bounds(index);
        let mut rng = Rng::new(OCCLUSION_SEED, u64::MAX, index as u64);
        let points: Vec<Vec3> = (0..CELL_POINTS)
            .map(|k| match k {
                0 => (min + max) * 0.5,
                _ => min + Vec3::new(rng.next_f32(), rng.next_f32(), rng.next_f32()) * grid.cell,
            })
            .collect();
        // A point inside a wall or a floor sees out through the back faces round it,
        // which no camera does: it is dropped, unless every point is inside.
        let open: Vec<Vec3> = points
            .iter()
            .copied()
            .filter(|&p| !self.inside(p))
            .collect();
        let points = if open.is_empty() { points } else { open };
        let near = (min - Vec3::splat(grid.cell), max + Vec3::splat(grid.cell));
        let mut bits = vec![0u64; self.targets.len().div_ceil(64)];
        for (i, (omin, omax, targets)) in self.targets.iter().enumerate() {
            let overlaps = omin.cmple(near.1).all() && omax.cmpge(near.0).all();
            if overlaps || self.reaches(&points, targets, i as u32) {
                bits[i / 64] |= 1 << (i % 64);
            }
        }
        bits
    }

    /// Whether `point` sits inside an occluder: most of the six axis rays from it
    /// first meet the back of an occluder's triangle (Unity's backface threshold).
    fn inside(&self, point: Vec3) -> bool {
        let axes = [
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
        ];
        let backs = axes.iter().filter(|&&dir| {
            self.rays
                .closest(point, dir, 0.0, f32::MAX)
                .is_some_and(|hit| {
                    let tri = &self.rays.tris[hit.tri];
                    !self.faces[hit.tri].stops(tri.normal(), dir)
                })
        });
        backs.count() > axes.len() / 2
    }

    /// Whether a ray from any of `from` reaches any of `targets` (on object
    /// `object`) with no other object's occluder turned toward it in between.
    fn reaches(&self, from: &[Vec3], targets: &[Vec3], object: u32) -> bool {
        targets.iter().any(|&to| {
            from.iter().any(|&origin| {
                let delta = to - origin;
                let reach = delta.length();
                if reach <= REACH_SLACK {
                    return true;
                }
                let dir = delta / reach;
                let blocks = |t: usize| {
                    let tri = &self.rays.tris[t];
                    tri.mesh != object && self.faces[t].stops(tri.normal(), dir)
                };
                !self
                    .rays
                    .occluded_by(origin, dir, (0.0, reach - REACH_SLACK), blocks)
            })
        })
    }
}

/// Where views to an object are aimed: its extremes along the eight box diagonals
/// (a corner peeking past a door frame), then [`OBJECT_POINTS`] seeded points on
/// its triangles, area-weighted, each nudged a hair toward its triangle's centre.
fn target_points(tris: &[[Vec3; 3]], object: u64) -> Vec<Vec3> {
    let inward = |p: Vec3, t: &[Vec3; 3]| p.lerp((t[0] + t[1] + t[2]) / 3.0, 0.01);
    let mut points = Vec::with_capacity(8 + OBJECT_POINTS);
    for k in 0..8 {
        let axis = |bit: i32| if k & bit == 0 { -1.0 } else { 1.0 };
        let diagonal = Vec3::new(axis(1), axis(2), axis(4));
        let best = tris.iter().flat_map(|t| t.iter().map(move |&p| (p, t)));
        let best = best.max_by(|a, b| a.0.dot(diagonal).total_cmp(&b.0.dot(diagonal)));
        if let Some((p, t)) = best {
            points.push(inward(p, t));
        }
    }
    let areas: Vec<f32> = tris
        .iter()
        .map(|t| (t[1] - t[0]).cross(t[2] - t[0]).length())
        .collect();
    let total: f32 = areas.iter().sum();
    let mut rng = Rng::new(OCCLUSION_SEED, object, 0);
    for _ in 0..OBJECT_POINTS {
        let mut pick = rng.next_f32() * total;
        let at = areas.iter().position(|&a| {
            pick -= a;
            pick <= 0.0
        });
        let t = &tris[at.unwrap_or(tris.len() - 1)];
        let (mut u, mut v) = (rng.next_f32(), rng.next_f32());
        if u + v > 1.0 {
            (u, v) = (1.0 - u, 1.0 - v);
        }
        points.push(inward(t[0] + (t[1] - t[0]) * u + (t[2] - t[0]) * v, t));
    }
    points
}

/// Each cell's set widened by its neighbours' (the dilation), then the distinct
/// sets as rows in first-seen cell order and each cell's row.
fn fold_rows(grid: &Grid, seen: &[Vec<u64>], words: usize) -> (Vec<u64>, Vec<u32>) {
    let mut ids: BTreeMap<Vec<u64>, u32> = BTreeMap::new();
    let (mut rows, mut cells) = (Vec::new(), Vec::with_capacity(seen.len()));
    for cell in 0..seen.len() {
        let mut bits = vec![0u64; words];
        for n in grid.neighbourhood(cell) {
            bits.iter_mut().zip(&seen[n]).for_each(|(b, s)| *b |= s);
        }
        let next = ids.len() as u32;
        let row = *ids.entry(bits).or_insert_with_key(|bits| {
            rows.extend_from_slice(bits);
            next
        });
        cells.push(row);
    }
    (rows, cells)
}
