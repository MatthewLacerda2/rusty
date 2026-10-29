//! The lightmap bake's ray queries (#438, #1330): every static triangle in one
//! parry `TriMesh`, whose BVH answers the bake's two questions — the nearest
//! triangle a ray hits, with the barycentrics to shade it, and whether anything
//! blocks a shadow ray. A hit names its triangle (the BVH leaf it came from)
//! and its barycentrics, both from parry's own ray–triangle test.
//!
//! Deterministic without rapier's `enhanced-determinism`: the mesh's BVH build is a
//! pure function of the triangles, and the ray path walks it on one thread with
//! scalar `f32` arithmetic (no SIMD lanes, no libm), so it gives the same bits on
//! every machine.

use glam::Vec3;
use rapier3d::parry::partitioning::TraversalAction;
use rapier3d::parry::query::details::local_ray_intersection_with_triangle;
use rapier3d::parry::query::Ray;
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
    /// Whether a bounce ray can hit it ([`Rays::closest`]).
    pub seen: bool,
    /// Whether it stops a shadow ray ([`Rays::occluded`]).
    pub blocks: bool,
}

impl Tri {
    pub(super) fn new(a: Vec3, b: Vec3, c: Vec3, mesh: u32, first: u32) -> Self {
        Tri {
            a,
            e1: b - a,
            e2: c - a,
            mesh,
            first,
            seen: true,
            blocks: true,
        }
    }

    /// The triangle with who sees it and what it blocks (#1571).
    pub(super) fn with(self, seen: bool, blocks: bool) -> Self {
        Tri {
            seen,
            blocks,
            ..self
        }
    }

    /// The unnormalized geometric normal (winding order).
    pub(super) fn normal(&self) -> Vec3 {
        self.e1.cross(self.e2)
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
        if tris.is_empty() {
            return Rays { tris, mesh: None };
        }
        let vertices = tris
            .iter()
            .flat_map(|t| [t.a, t.a + t.e1, t.a + t.e2])
            .collect();
        let indices = (0..tris.len() as u32)
            .map(|i| [3 * i, 3 * i + 1, 3 * i + 2])
            .collect();
        // Only an empty index list is refused, and that case returned above: any
        // other error is a parry change that must fail loudly, not bake unshadowed.
        let mesh = TriMesh::new(vertices, indices).expect("a non-empty triangle soup");
        Rays {
            tris,
            mesh: Some(mesh),
        }
    }

    /// The nearest triangle it can see along `origin + t * dir` with `t` in
    /// `[t_min, t_max)`.
    pub(super) fn closest(&self, origin: Vec3, dir: Vec3, t_min: f32, t_max: f32) -> Option<Hit> {
        let mesh = self.mesh.as_ref()?;
        let ray = Ray::new(origin + dir * t_min, dir);
        let (tri, (_, weights)) = mesh.bvh().find_best(
            t_max - t_min,
            |node, best| node.cast_ray(&ray, best),
            |i, best| match self.tris[i as usize].seen {
                true => hit(mesh, i, &ray, best),
                false => None,
            },
        )?;
        Some(Hit {
            tri: tri as usize,
            u: weights.y,
            v: weights.z,
        })
    }

    /// Whether a blocking triangle lies on `origin + t * dir` for `t` in
    /// `[t_min, t_max)`. Stops
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
                if self.tris[i as usize].blocks && hit(mesh, i, &ray, reach).is_some() {
                    blocked = true;
                    return TraversalAction::EarlyExit;
                }
            }
            TraversalAction::Continue
        });
        blocked
    }
}

/// Where `ray` meets the mesh's triangle `i` short of `reach`, from either side: the
/// time of impact and the barycentric weights of its corners. parry solves both from
/// the ray, so a sliver triangle still gives finite weights at the point it was hit.
fn hit(mesh: &TriMesh, i: u32, ray: &Ray, reach: f32) -> Option<(f32, Vec3)> {
    let t = mesh.triangle(i);
    let (inter, weights) = local_ray_intersection_with_triangle(t.a, t.b, t.c, ray)?;
    (inter.time_of_impact < reach).then_some((inter.time_of_impact, weights))
}
