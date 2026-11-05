//! The occlusion bake's sampling (#1714): the static scene ready to trace, and what
//! one cell sees. Rays go from seeded points in the cell to each object's targets
//! (`targets`) through the lightmap bake's BVH (`lightmap::Rays`), after a shaft
//! test (`shaft`) that proves an object or a target hidden from a whole box at once.
//!
//! The eye keeps one density at every cell size: a cell is sampled as
//! [`Grid::subdivisions`] cubes of at most `CELL_SIZE` a side, each with its
//! centre and [`CUBE_POINTS`]` - 1` seeded points, so a gap a 2 m cell sees through
//! is seen through by a 3.9 m one.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use glam::Vec3;

use super::gather::{Face, Object};
use super::progress::OcclusionProgress;
use super::shaft::Polygons;
use super::targets::{target_points, Target};
use super::{Grid, OCCLUSION_SEED};
use crate::scene::lighting::lightmap::{Rays, Rng, Tri};

/// Points sampled in each sampling cube of a cell (its centre first, the rest
/// seeded in the cube).
const CUBE_POINTS: usize = 6;
/// How far short of a target point a ray stops, so the triangle it lies on (or
/// one touching it) does not hide it.
pub(super) const REACH_SLACK: f32 = 1e-3;

/// An object's world bounds and its targets (`None`: too large or not finite to
/// sample, so it stays in sight of every cell).
pub(super) type Targets = (Vec3, Vec3, Option<Vec<Target>>);

/// The scene ready to sample: the occluders' BVH, each triangle's drawn side and
/// the planar polygons they make, and per object its bounds and target points.
pub(super) struct Baking {
    pub rays: Rays,
    pub faces: Vec<Face>,
    pub polygons: Polygons,
    pub targets: Vec<Targets>,
    /// The bounds of every object that can be culled, `None` when none can: a
    /// never-culled one (a ground slab) needs no cell, so it never coarsens them.
    pub bounds: Option<(Vec3, Vec3)>,
}

impl Baking {
    pub fn of(objects: &[Object]) -> Self {
        let (mut tris, mut faces) = (Vec::new(), Vec::new());
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
                faces.push(face);
            }
            let corners = object.drawn.iter().flat_map(|(t, _)| t);
            let min = corners
                .clone()
                .fold(Vec3::splat(f32::MAX), |m, &p| m.min(p));
            let max = corners.fold(Vec3::splat(f32::MIN), |m, &p| m.max(p));
            let points = target_points(&object.drawn);
            if points.is_some() {
                bounds = Some(bounds.map_or((min, max), |(lo, hi)| (lo.min(min), hi.max(max))));
            }
            targets.push((min, max, points));
        }
        let polygons = Polygons::of(&tris, &faces);
        Self {
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
                    let bits = self.cell(grid, cell);
                    out.lock().unwrap_or_else(|e| e.into_inner())[cell] = bits;
                    progress.cell_done();
                });
            }
        });
        let out = out.into_inner().unwrap_or_else(|e| e.into_inner());
        (!progress.is_cancelled()).then_some(out)
    }

    /// Cell `index`'s sampling cubes, each with its points: the centre, then seeded
    /// ones from the cell's own stream.
    fn samples(&self, grid: &Grid, index: usize) -> Vec<((Vec3, Vec3), Vec<Vec3>)> {
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
            cubes.push(((lo, lo + Vec3::splat(edge)), points.collect::<Vec<_>>()));
        }
        // A point inside a wall or a floor sees out through the back faces round it,
        // which no camera does: it is dropped, unless every point is inside.
        let open = |p: &Vec3| !self.inside(*p);
        let kept: Vec<Vec<Vec3>> = cubes
            .iter()
            .map(|(_, p)| p.iter().copied().filter(open).collect())
            .collect();
        if kept.iter().any(|p| !p.is_empty()) {
            for (cube, kept) in cubes.iter_mut().zip(kept) {
                cube.1 = kept;
            }
            cubes.retain(|c| !c.1.is_empty());
        }
        cubes
    }

    /// What cell `index` sees on its own: an object whose bounds reach the cell or
    /// its neighbours outright, any other when a sampled ray reaches it.
    pub fn cell(&self, grid: &Grid, index: usize) -> Vec<u64> {
        let (min, max) = grid.bounds(index);
        let cubes = self.samples(grid, index);
        let near = (min - Vec3::splat(grid.cell), max + Vec3::splat(grid.cell));
        let mut bits = vec![0u64; self.targets.len().div_ceil(64)];
        for (i, (omin, omax, targets)) in self.targets.iter().enumerate() {
            let overlaps = omin.cmple(near.1).all() && omax.cmpge(near.0).all();
            let reached = |t: &Vec<Target>| {
                cubes.iter().any(|(cube, points)| {
                    !self
                        .polygons
                        .hides(self.parts(), *cube, (*omin, *omax), i as u32)
                        && self.reaches(*cube, points, t, i as u32)
                })
            };
            if overlaps || targets.as_ref().is_none_or(reached) {
                bits[i / 64] |= 1 << (i % 64);
            }
        }
        bits
    }

    /// The BVH and the drawn sides, as the shaft test reads them.
    fn parts(&self) -> (&Rays, &[Face]) {
        (&self.rays, &self.faces)
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

    /// Whether a ray from any of `from` (in the box `cube`) reaches any of
    /// `targets` (on object `object`) with no other object's occluder turned toward
    /// it in between. A target whose drawn side faces away from the whole cube, or
    /// that one occluder's polygon hides from all of it, is skipped.
    fn reaches(&self, cube: (Vec3, Vec3), from: &[Vec3], targets: &[Target], object: u32) -> bool {
        let facing = targets.iter().filter(|t| t.faces(cube));
        let mut open = facing.filter(|t| {
            !self
                .polygons
                .hides(self.parts(), cube, (t.at, t.at), object)
        });
        open.any(|target| {
            let to = target.at;
            let from = from.iter().filter(|&&o| target.drawn_toward(o));
            from.copied().any(|origin| {
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
