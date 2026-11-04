//! The occlusion bake (#1714): which baked objects each cell may see, by rays from
//! seeded points in the cell to seeded points on each object's triangles, through
//! the lightmap bake's BVH (`lightmap::Rays`), then dilated over each cell's
//! neighbours and folded into shared rows. Gathered on the caller's thread
//! ([`gather_occlusion`]) and baked on any ([`OcclusionInput::bake`]), so the editor
//! bakes on a worker and a script in line, the same bytes either way.
//!
//! A pure function of the scene and [`OCCLUSION_SEED`]: each cell draws from its own
//! stream and is baked alone, so the result is the same at any worker count.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use glam::Vec3;

use super::gather::{Face, Fingerprints, Object};
use super::progress::OcclusionProgress;
use super::targets::{target_points, Target};
use super::{BakedObject, Grid, OcclusionData};
use crate::scene::lighting::lightmap::{Rays, Rng, Tri};
use crate::scene::Scene;

/// The bake's seed: one fixed stream, so a re-bake of an unchanged scene writes
/// the same bytes.
pub const OCCLUSION_SEED: u64 = 0x5056_5321;

/// Points sampled in each cell (its centre first, the rest seeded in the box).
const CELL_POINTS: usize = 6;
/// How far short of a target point a ray stops, so the triangle it lies on (or
/// one touching it) does not hide it.
const REACH_SLACK: f32 = 1e-3;

/// What one occlusion bake found. All zero when it baked nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OcclusionBakeReport {
    /// Static meshes baked as occludees.
    pub objects: usize,
    /// Of them, the ones that occlude.
    pub occluders: usize,
    /// Of them, the ones too large to sample (`targets::MAX_TARGETS`), kept in
    /// sight of every cell.
    pub never_culled: usize,
    pub cells: usize,
    /// Distinct visible sets the cells share.
    pub rows: usize,
    /// Bytes of the stored data.
    pub bytes: usize,
}

/// The static scene as the bake reads it, owned, so a worker thread can bake it
/// while the scene goes on ([`gather_occlusion`]).
pub struct OcclusionInput {
    objects: Vec<Object>,
}

/// What `scene`'s occlusion bake reads, gathered on the caller's thread.
pub fn gather_occlusion(scene: &Scene) -> OcclusionInput {
    let prints = Fingerprints::of_scene(scene);
    let mut ids = scene.world.ids_with_mesh();
    ids.sort_unstable();
    let objects = ids.iter().filter_map(|&id| prints.object(scene, id));
    OcclusionInput {
        objects: objects.collect(),
    }
}

/// Bake `scene`'s PVS on `threads` workers, in line. `None` when there is nothing
/// to cull: no static mesh, or none that occludes.
pub fn bake_occlusion(
    scene: &Scene,
    threads: usize,
) -> (Option<OcclusionData>, OcclusionBakeReport) {
    let input = gather_occlusion(scene);
    input
        .bake(threads, &OcclusionProgress::default())
        .expect("nobody cancels it")
}

impl OcclusionInput {
    /// Bake the PVS on `threads` workers, counting into `progress`; `None` when it
    /// was cancelled, else the data (`None` when nothing occludes) and the report.
    pub fn bake(
        &self,
        threads: usize,
        progress: &OcclusionProgress,
    ) -> Option<(Option<OcclusionData>, OcclusionBakeReport)> {
        let objects = &self.objects;
        let occluders = objects.iter().filter(|o| !o.occluding.is_empty()).count();
        if occluders == 0 {
            return Some((None, OcclusionBakeReport::default()));
        }
        let scene = Baking::of(objects);
        let grid = Grid::over(scene.bounds.0, scene.bounds.1);
        progress.start(grid.cell_count());
        let seen = scene.cells(&grid, threads.max(1), progress)?;
        let (rows, cells) = fold_rows(&grid, &seen, objects.len().div_ceil(64));
        let baked = objects.iter().map(|o| BakedObject {
            entity: o.entity,
            fingerprint: o.fingerprint,
            occluder: !o.occluding.is_empty(),
        });
        let data = OcclusionData::new(grid, baked.collect(), rows, cells);
        let data = data.expect("rows fit the grid");
        let report = OcclusionBakeReport {
            objects: objects.len(),
            occluders,
            never_culled: scene.targets.iter().filter(|t| t.2.is_none()).count(),
            cells: grid.cell_count(),
            rows: data.row_count(),
            bytes: super::encode(&data).len(),
        };
        Some((Some(data), report))
    }
}

/// The scene ready to sample: the occluders' BVH and each triangle's drawn side,
/// and per object its bounds and target points.
struct Baking {
    rays: Rays,
    faces: Vec<Face>,
    /// Per object its bounds and target points, `None` when it is too large to
    /// sample and stays in sight.
    targets: Vec<(Vec3, Vec3, Option<Vec<Target>>)>,
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
            let corners = object.drawn.iter().flat_map(|(t, _)| t);
            let min = corners
                .clone()
                .fold(Vec3::splat(f32::MAX), |m, &p| m.min(p));
            let max = corners.fold(Vec3::splat(f32::MIN), |m, &p| m.max(p));
            bounds = (bounds.0.min(min), bounds.1.max(max));
            targets.push((min, max, target_points(&object.drawn)));
        }
        Self {
            rays: Rays::build(tris),
            faces,
            targets,
            bounds,
        }
    }

    /// Each cell's own visible bits, `words` per cell, baked on `threads` workers;
    /// `None` when `progress` was cancelled.
    fn cells(
        &self,
        grid: &Grid,
        threads: usize,
        progress: &OcclusionProgress,
    ) -> Option<Vec<Vec<u64>>> {
        let count = grid.cell_count();
        let next = AtomicUsize::new(0);
        let out = Mutex::new(vec![Vec::new(); count]);
        std::thread::scope(|s| {
            for _ in 0..threads.min(count) {
                s.spawn(|| loop {
                    let cell = next.fetch_add(1, Ordering::Relaxed);
                    if cell >= count || progress.is_cancelled() {
                        break;
                    }
                    let bits = self.cell(grid, cell);
                    out.lock().unwrap_or_else(|e| e.into_inner())[cell] = bits;
                    progress.cell_done();
                });
            }
        });
        let out = out.into_inner().unwrap_or_else(|e| e.into_inner());
        (!progress.is_cancelled()).then_some(out)
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
            // A target whose drawn side faces away from the whole cell is seen
            // from no point in it.
            let reached = |t: &Vec<Target>| {
                let facing = t.iter().filter(|t| t.faces((min, max))).map(|t| t.at);
                self.reaches(&points, facing, i as u32)
            };
            if overlaps || targets.as_ref().is_none_or(reached) {
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
    fn reaches(&self, from: &[Vec3], mut targets: impl Iterator<Item = Vec3>, object: u32) -> bool {
        targets.any(|to| {
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
