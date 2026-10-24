//! The lightmap bake's ray queries (#438, #1330): every static triangle in one
//! parry `TriMesh`, whose BVH answers the bake's two questions — the nearest
//! triangle a ray hits, with the barycentrics to shade it, and whether anything
//! blocks a shadow ray. A hit names its triangle (the intersection's `subshape`);
//! the barycentrics come back from the hit point and the triangle's corners.
//!
//! Deterministic without rapier's `enhanced-determinism`: the mesh's BVH build is a
//! pure function of the triangles, and the ray path walks it on one thread with
//! scalar `f32` arithmetic (no SIMD lanes, no libm), so it gives the same bits on
//! every machine.

use glam::Vec3;
use rapier3d::parry::partitioning::TraversalAction;
use rapier3d::parry::query::{Ray, RayCast};
use rapier3d::parry::shape::TriMesh;

/// One triangle: a corner and the two edges from it, plus where it came from.
#[derive(Clone, Copy)]
pub(super) struct Tri {
    pub a: Vec3,
    pub e1: Vec3,
    pub e2: Vec3,
    /// Index into `BakeScene::meshes`.
    pub mesh: u32,
    /// Offset of the triangle's first index in that mesh's index list.
    pub first: u32,
}

impl Tri {
    pub(super) fn new(a: Vec3, b: Vec3, c: Vec3, mesh: u32, first: u32) -> Self {
        Tri {
            a,
            e1: b - a,
            e2: c - a,
            mesh,
            first,
        }
    }

    /// The unnormalized geometric normal (winding order).
    pub(super) fn normal(&self) -> Vec3 {
        self.e1.cross(self.e2)
    }

    /// The barycentrics `(u, v)` of `b` and `c` at `point`, on the triangle's plane.
    fn barycentrics(&self, point: Vec3) -> (f32, f32) {
        let p = point - self.a;
        let (d11, d12, d22) = (
            self.e1.dot(self.e1),
            self.e1.dot(self.e2),
            self.e2.dot(self.e2),
        );
        let (p1, p2) = (p.dot(self.e1), p.dot(self.e2));
        let inv = 1.0 / (d11 * d22 - d12 * d12);
        ((d22 * p1 - d12 * p2) * inv, (d11 * p2 - d12 * p1) * inv)
    }
}

/// The nearest hit along a ray: the triangle, and the barycentrics of `b` and `c`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Hit {
    pub tri: usize,
    pub u: f32,
    pub v: f32,
}

/// The static triangles and the mesh that traces them; `tris[i]` is the mesh's
/// triangle `i`. `None` when there is nothing to hit (parry refuses an empty mesh).
pub(super) struct Rays {
    pub tris: Vec<Tri>,
    mesh: Option<TriMesh>,
}

impl Rays {
    pub(super) fn build(tris: Vec<Tri>) -> Self {
        let vertices = tris
            .iter()
            .flat_map(|t| [t.a, t.a + t.e1, t.a + t.e2])
            .collect();
        let indices = (0..tris.len() as u32)
            .map(|i| [3 * i, 3 * i + 1, 3 * i + 2])
            .collect();
        let mesh = TriMesh::new(vertices, indices).ok();
        Rays { tris, mesh }
    }

    /// The nearest hit along `origin + t * dir` with `t` in `(t_min, t_max]`.
    pub(super) fn closest(&self, origin: Vec3, dir: Vec3, t_min: f32, t_max: f32) -> Option<Hit> {
        let ray = Ray::new(origin + dir * t_min, dir);
        let hit = self
            .mesh
            .as_ref()?
            .cast_local_ray_and_get_normal(&ray, t_max - t_min, false)?;
        let tri = hit.subshape as usize;
        let (u, v) = self.tris[tri].barycentrics(ray.point_at(hit.time_of_impact));
        Some(Hit { tri, u, v })
    }

    /// Whether anything blocks `origin + t * dir` for `t` in `(t_min, t_max]`. Stops
    /// at the first blocker rather than looking for the nearest.
    pub(super) fn occluded(&self, origin: Vec3, dir: Vec3, t_min: f32, t_max: f32) -> bool {
        let Some(mesh) = &self.mesh else {
            return false;
        };
        let ray = Ray::new(origin + dir * t_min, dir);
        let reach = t_max - t_min;
        let mut blocked = false;
        mesh.bvh().traverse(|node| {
            if node.cast_ray(&ray, reach) == f32::MAX {
                return TraversalAction::Prune;
            }
            if let Some(i) = node.leaf_data() {
                if mesh.triangle(i).intersects_local_ray(&ray, reach) {
                    blocked = true;
                    return TraversalAction::EarlyExit;
                }
            }
            TraversalAction::Continue
        });
        blocked
    }
}
