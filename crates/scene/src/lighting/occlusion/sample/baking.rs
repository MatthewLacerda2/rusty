//! The occlusion bake's sampling (#1714): the static scene ready to trace, and what
//! one cell sees. Rays go from seeded points in the cell to each object's targets
//! (`targets`) through the lightmap bake's BVH (`lightmap::Rays`), once no
//! occluder face, alone (`shaft`) or fused with the others on its plane
//! (`sheets`), proves them hidden (`sight`).
//!
//! The eye keeps one density at every cell size: a cell is sampled as
//! [`Grid::subdivisions`] cubes of at most `CELL_SIZE` a side, each with its
//! centre and [`CUBE_POINTS`]` - 1` seeded points, so a gap a 2 m cell sees through
//! is seen through by a 3.9 m one.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use glam::Vec3;

use super::clusters::{clusters, Cluster};
use super::eye::Eye;
use super::shaft::Polygons;
use super::targets::{chunks, target_points, Chunk};
use crate::lighting::lightmap::{Rays, Rng, Tri};
use crate::lighting::occlusion::gather::{Face, Object};
use crate::lighting::occlusion::progress::OcclusionProgress;
use crate::lighting::occlusion::{Grid, OCCLUSION_SEED};

/// Points sampled in each sampling cube of a cell (its centre first, the rest
/// seeded in the cube).
const CUBE_POINTS: usize = 6;
/// How far short of a target point a ray stops, so the triangle it lies on (or
/// one touching it) does not hide it.
pub(super) const REACH_SLACK: f32 = 1e-3;

/// An object's world bounds and its targets (`None`: too large or not finite to
/// sample, so it stays in sight of every cell).
pub(super) type Targets = (Vec3, Vec3, Option<Vec<Chunk>>);

/// The scene ready to sample: the occluders' BVH, each triangle's drawn side and
/// the planar polygons they make, and per object its bounds and target points.
pub(in crate::lighting::occlusion) struct Baking {
    pub(super) rays: Rays,
    pub(super) faces: Vec<Face>,
    pub(super) polygons: Polygons,
    pub(super) targets: Vec<Targets>,
    pub(super) clusters: Vec<Cluster>,
    /// The bounds of every object that can be culled, `None` when none can: a
    /// never-culled one (a ground slab) needs no cell, so it never coarsens them.
    pub bounds: Option<(Vec3, Vec3)>,
}

impl Baking {
    /// How many objects are too large or not finite to sample, and so are kept
    /// in sight of every cell.
    pub fn never_culled(&self) -> usize {
        self.targets.iter().filter(|t| t.2.is_none()).count()
    }

    /// How many sheets the occluders fused into, and the bytes they take.
    #[cfg(all(test, feature = "dev"))]
    pub fn sheets(&self) -> (usize, usize) {
        self.polygons.sheet_bytes()
    }

    /// How many rays it has cast, for a cost measurement.
    #[cfg(all(test, feature = "dev"))]
    pub fn rays_cast(&self) -> u64 {
        self.rays.cast.load(Ordering::Relaxed)
    }

    pub fn of(objects: &[Object]) -> Self {
        let (mut tris, mut exact, mut faces) = (Vec::new(), Vec::new(), Vec::new());
        let mut bounds: Option<(Vec3, Vec3)> = None;
        let mut targets = Vec::with_capacity(objects.len());
        for (i, object) in objects.iter().enumerate() {
            // A corner that is not finite stops nothing; dropping it only shows more.
            let finite = object
                .occluding
                .iter()
                .filter(|t| t.0.iter().all(|p| p.is_finite()));
            for &([a, b, c], face) in finite {
                tris.push(Tri::new(a, b, c, i as u32, 0));
                exact.push([a, b, c]);
                faces.push(face);
            }
            let corners = object.drawn.iter().flat_map(|(t, _)| t);
            let min = corners
                .clone()
                .fold(Vec3::splat(f32::MAX), |m, &p| m.min(p));
            let max = corners.fold(Vec3::splat(f32::MIN), |m, &p| m.max(p));
            let points = target_points(&object.drawn).map(chunks);
            if points.is_some() {
                bounds = Some(bounds.map_or((min, max), |(lo, hi)| (lo.min(min), hi.max(max))));
            }
            targets.push((min, max, points));
        }
        let polygons = Polygons::of(&tris, &exact, &faces);
        Self {
            clusters: clusters(&targets),
            rays: Rays::build(tris),
            faces,
            polygons,
            targets,
            bounds,
        }
    }

    /// Each cell's own visible bits, `words` per cell, baked on `threads` workers;
    /// `None` when `progress` was cancelled.
    pub fn cells(
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
                    progress.cell_started(cell);
                    let bits = self.cell(grid, cell);
                    progress.cell_done(cell);
                    out.lock().unwrap_or_else(|e| e.into_inner())[cell] = bits;
                });
            }
        });
        let out = out.into_inner().unwrap_or_else(|e| e.into_inner());
        (!progress.is_cancelled()).then_some(out)
    }

    /// Cell `index`'s sampling cubes, each as its points: the centre, then seeded
    /// ones from the cell's own stream.
    fn samples(&self, grid: &Grid, index: usize) -> Vec<Vec<Vec3>> {
        let (min, _) = grid.bounds(index);
        let n = grid.subdivisions();
        let edge = grid.cell / n as f32;
        let mut rng = Rng::new(OCCLUSION_SEED, u64::MAX, index as u64);
        let mut cubes = Vec::with_capacity((n * n * n) as usize);
        for at in (0..n * n * n).map(|i| glam::UVec3::new(i % n, i / (n * n), (i / n) % n)) {
            let lo = min + at.as_vec3() * edge;
            let points = (0..CUBE_POINTS).map(|k| match k {
                0 => lo + Vec3::splat(edge * 0.5),
                _ => lo + Vec3::new(rng.next_f32(), rng.next_f32(), rng.next_f32()) * edge,
            });
            cubes.push(points.collect::<Vec<_>>());
        }
        // A point inside a wall or a floor sees out through the back faces round it,
        // which no camera does: it is dropped, unless every point is inside.
        let open = |p: &Vec3| !self.inside(*p);
        let kept: Vec<Vec<Vec3>> = cubes
            .iter()
            .map(|p| p.iter().copied().filter(open).collect())
            .collect();
        if kept.iter().any(|p| !p.is_empty()) {
            cubes = kept;
            cubes.retain(|c| !c.is_empty());
        }
        cubes
    }

    /// What cell `index` sees on its own: an object whose bounds reach the cell or
    /// its neighbours outright, any other when a sampled ray reaches it.
    pub fn cell(&self, grid: &Grid, index: usize) -> Vec<u64> {
        let (min, max) = grid.bounds(index);
        let samples = self.samples(grid, index).into_iter();
        let mut eyes: Vec<Eye> = samples.map(Eye::new).collect();
        let near = (min - Vec3::splat(grid.cell), max + Vec3::splat(grid.cell));
        let mut bits = vec![0u64; self.targets.len().div_ceil(64)];
        let set = |bits: &[u64], i: u32| bits[i as usize / 64] & (1 << (i % 64)) != 0;
        for (i, (omin, omax, targets)) in self.targets.iter().enumerate() {
            let overlaps = omin.cmple(near.1).all() && omax.cmpge(near.0).all();
            if overlaps || targets.is_none() {
                bits[i / 64] |= 1 << (i % 64);
            }
        }
        // Eye by eye, cluster by cluster: nearby objects one after another, so
        // the polygons an eye remembers keep hiding them (`eye`).
        for eye in &mut eyes {
            for cluster in &self.clusters {
                if cluster.objects.iter().all(|&i| set(&bits, i))
                    || self.hidden(eye, cluster.bounds)
                {
                    continue;
                }
                for &i in &cluster.objects {
                    let (lo, hi, targets) = &self.targets[i as usize];
                    let chunks = targets.as_deref().unwrap_or_default();
                    if !set(&bits, i) && self.sees(eye, (i, (*lo, *hi)), chunks) {
                        bits[i as usize / 64] |= 1 << (i % 64);
                    }
                }
            }
        }
        bits
    }

    /// Whether one polygon hides the box `to`, which holds whole objects, from
    /// all of `eye`. Any polygon will do: one of those objects' own lies inside
    /// the box, so it never separates it.
    fn hidden(&self, eye: &mut Eye, to: (Vec3, Vec3)) -> bool {
        self.proved(eye, eye.bounds, to, u32::MAX)
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
}
