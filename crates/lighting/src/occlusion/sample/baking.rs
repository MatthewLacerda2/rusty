use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use glam::Vec3;

use super::clusters::{clusters, Cluster};
use super::eye::Eye;
use super::shaft::Polygons;
use super::targets::{chunks, target_points, Chunk};
use crate::lightmap::{Rays, Rng, Tri};
use crate::occlusion::gather::{Face, Object};
use crate::occlusion::progress::OcclusionProgress;
use crate::occlusion::OCCLUSION_SEED;
use rusty_scene::lighting::occlusion::Grid;

const CUBE_POINTS: usize = 6;
pub(super) const REACH_SLACK: f32 = 1e-3;

pub(super) type Targets = (Vec3, Vec3, Option<Vec<Chunk>>);

pub(in crate::occlusion) struct Baking {
    pub(super) rays: Rays,
    pub(super) faces: Vec<Face>,
    pub(super) polygons: Polygons,
    pub(super) targets: Vec<Targets>,
    pub(super) clusters: Vec<Cluster>,
    pub bounds: Option<(Vec3, Vec3)>,
}

impl Baking {
    pub fn never_culled(&self) -> usize {
        self.targets.iter().filter(|t| t.2.is_none()).count()
    }

    #[cfg(feature = "dev")]
    pub fn sheets(&self) -> (usize, usize) {
        self.polygons.sheet_bytes()
    }

    #[cfg(feature = "dev")]
    pub fn rays_cast(&self) -> u64 {
        self.rays.cast.load(Ordering::Relaxed)
    }

    #[cfg(feature = "dev")]
    pub fn counted(self) -> Self {
        Self {
            rays: self.rays.counted(),
            ..self
        }
    }

    pub fn of(objects: &[Object]) -> Self {
        let (mut tris, mut exact, mut faces) = (Vec::new(), Vec::new(), Vec::new());
        let mut bounds: Option<(Vec3, Vec3)> = None;
        let mut targets = Vec::with_capacity(objects.len());
        for (i, object) in objects.iter().enumerate() {
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

    pub fn cells(
        &self,
        grid: &Grid,
        threads: usize,
        progress: &OcclusionProgress,
        sampled: Option<&[bool]>,
    ) -> Option<Vec<Option<Vec<u64>>>> {
        let count = grid.cell_count();
        let todo: Vec<usize> = match sampled {
            Some(s) => (0..count).filter(|&c| s[c]).collect(),
            None => (0..count).collect(),
        };
        let next = AtomicUsize::new(0);
        let out = Mutex::new(vec![None; count]);
        std::thread::scope(|s| {
            for _ in 0..threads.min(todo.len()) {
                s.spawn(|| loop {
                    let at = next.fetch_add(1, Ordering::Relaxed);
                    if at >= todo.len() || progress.is_cancelled() {
                        break;
                    }
                    let cell = todo[at];
                    progress.cell_started(cell);
                    let bits = self.cell(grid, cell);
                    progress.cell_done(cell);
                    out.lock().unwrap_or_else(|e| e.into_inner())[cell] = Some(bits);
                });
            }
        });
        let out = out.into_inner().unwrap_or_else(|e| e.into_inner());
        (!progress.is_cancelled()).then_some(out)
    }

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

    fn hidden(&self, eye: &mut Eye, to: (Vec3, Vec3)) -> bool {
        // No object excluded: a polygon of an object inside `to` lies inside the box, so it never
        // separates it.
        self.proved(eye, eye.bounds, to, u32::MAX)
    }

    // Unity's backface threshold: inside when most axis rays first meet a back face.
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
