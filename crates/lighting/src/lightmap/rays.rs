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
//!
//! The BVH's node test is our own copy of parry's ([`NodeRay`]), with the
//! per-axis reciprocal of the direction worked out once per ray instead of at
//! every node: the same operations in the same order, so the same bits.

use glam::Vec3;
use rapier3d::parry::partitioning::{BvhNode, TraversalAction};
use rapier3d::parry::query::details::local_ray_intersection_with_triangle;
use rapier3d::parry::query::Ray;
use rapier3d::parry::shape::TriMesh;

/// One triangle: a corner and the two edges from it, plus where it came from.
#[derive(Clone, Copy)]
pub(crate) struct Tri {
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
    pub(crate) fn new(a: Vec3, b: Vec3, c: Vec3, mesh: u32, first: u32) -> Self {
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
    pub(crate) fn normal(&self) -> Vec3 {
        self.e1.cross(self.e2)
    }
}

/// The nearest hit along a ray: the triangle, and the barycentrics of `b` and `c`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hit {
    pub tri: usize,
    pub u: f32,
    pub v: f32,
}

/// The static triangles and the mesh that traces them; `tris[i]` is the mesh's
/// triangle `i`. `None` when there is nothing to hit (parry refuses an empty mesh).
pub(crate) struct Rays {
    pub tris: Vec<Tri>,
    mesh: Option<TriMesh>,
    /// How many rays were cast, either kind: what a cost test counts instead of
    /// timing (#1883), so it holds on any machine. Counted only when `counting`:
    /// in a test build, and for `dev::bake`'s bench ([`Rays::counted`]), so a bake
    /// shares no counter across its workers.
    pub cast: std::sync::atomic::AtomicU64,
    counting: bool,
}

impl Rays {
    pub(crate) fn build(tris: Vec<Tri>) -> Self {
        if tris.is_empty() {
            return Rays {
                tris,
                mesh: None,
                cast: Default::default(),
                // lint: allow(cfg-test-switch) scene's own tests; another crate counts through `Rays::counted`
                counting: cfg!(test),
            };
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
            cast: Default::default(),
            // lint: allow(cfg-test-switch) scene's own tests; another crate counts through `Rays::counted`
            counting: cfg!(test),
        }
    }

    /// The nearest triangle it can see along `origin + t * dir` with `t` in
    /// `[t_min, t_max)`.
    pub(crate) fn closest(&self, origin: Vec3, dir: Vec3, t_min: f32, t_max: f32) -> Option<Hit> {
        self.count();
        let mesh = self.mesh.as_ref()?;
        let ray = Ray::new(origin + dir * t_min, dir);
        let boxes = NodeRay::new(&ray);
        let (tri, (_, weights)) = mesh.bvh().find_best(
            t_max - t_min,
            |node, best| boxes.cast(node, best),
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
        self.occluded_by(origin, dir, (t_min, t_max), |i| self.tris[i].blocks)
    }

    /// Whether a triangle `blocks` accepts (by its index) lies on `origin + t * dir`
    /// for `t` in `[t_min, t_max)`: [`Self::occluded`] with the caller's say on
    /// which triangles stop the ray (the occlusion bake's facing test, #1714).
    pub(crate) fn occluded_by(
        &self,
        origin: Vec3,
        dir: Vec3,
        range: (f32, f32),
        blocks: impl Fn(usize) -> bool,
    ) -> bool {
        self.blocker(origin, dir, range, blocks).is_some()
    }

    /// The first triangle met in the BVH's walk (not the nearest) that `blocks`
    /// accepts and that lies on `origin + t * dir` for `t` in `[t_min, t_max)`:
    /// [`Self::occluded_by`] naming its blocker, so the occlusion bake can try it
    /// first on the next ray (#1714).
    pub(crate) fn blocker(
        &self,
        origin: Vec3,
        dir: Vec3,
        (t_min, t_max): (f32, f32),
        blocks: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        self.count();
        let mesh = self.mesh.as_ref()?;
        let ray = Ray::new(origin + dir * t_min, dir);
        let reach = t_max - t_min;
        let boxes = NodeRay::new(&ray);
        let mut blocker = None;
        mesh.bvh().traverse(|node| {
            if boxes.cast(node, reach) == f32::MAX {
                return TraversalAction::Prune;
            }
            if let Some(i) = node.leaf_data() {
                if blocks(i as usize) && hit(mesh, i, &ray, reach).is_some() {
                    blocker = Some(i as usize);
                    return TraversalAction::EarlyExit;
                }
            }
            TraversalAction::Continue
        });
        blocker
    }

    /// These rays, counting every cast into `cast` (the cost bench's).
    #[cfg(feature = "dev")]
    pub(crate) fn counted(self) -> Self {
        Self {
            counting: true,
            ..self
        }
    }

    /// One more ray cast, when counting; nothing otherwise.
    fn count(&self) {
        if self.counting {
            self.cast.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// A ray made ready for the BVH's node test: its origin, and per axis whether
/// its direction is zero and the direction's reciprocal.
#[derive(Clone, Copy)]
pub(super) struct NodeRay {
    origin: [f32; 3],
    zero: [bool; 3],
    inv: [f32; 3],
    /// The ray itself, for the test build's parry path ([`PARRY_NODES`]).
    #[cfg(test)]
    ray: Ray,
}

/// Test builds only: when set, [`NodeRay::cast`] answers with parry's own
/// `BvhNode::cast_ray`, so a test can bake both ways and compare the bytes; and
/// how many nodes it tested. Under nextest every test is its own process, so one
/// test's switch reaches no other (and the two paths agree to the bit anyway).
#[cfg(test)]
pub(super) static PARRY_NODES: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(test)]
pub(super) static NODES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl NodeRay {
    pub(super) fn new(ray: &Ray) -> Self {
        let d = ray.dir.to_array();
        NodeRay {
            origin: ray.origin.to_array(),
            // parry's `is_zero()`: `== 0.0`, so -0.0 too, and NaN is not zero.
            zero: d.map(|c| c == 0.0),
            inv: d.map(|c| 1.0 / c),
            #[cfg(test)]
            ray: *ray,
        }
    }

    /// Where the ray enters `node`'s box short of `max_toi`, 0 when it starts
    /// inside, `f32::MAX` when it misses: parry 0.31's `BvhNode::cast_ray`, which
    /// is `Aabb::cast_local_ray(ray, max_toi, true)` (`query/ray/ray_aabb.rs`)
    /// with `1.0 / dir` hoisted out of the node loop. Copied because parry's own
    /// inverse-ray test (`BvhNode::cast_inv_ray_simd`) is private, and it is SIMD.
    /// Keep it parry's operations in parry's order (the zero test and slab check,
    /// the swap, `max` then `min`, the early miss; no fused multiply-add): the
    /// property test in `ray_box_tests` compares the two bit for bit, so a parry
    /// upgrade that changes its test fails there rather than drifting the bake.
    pub(super) fn cast(&self, node: &BvhNode, max_toi: f32) -> f32 {
        #[cfg(test)]
        {
            use std::sync::atomic::Ordering::Relaxed;
            NODES.fetch_add(1, Relaxed);
            if PARRY_NODES.load(Relaxed) {
                return node.cast_ray(&self.ray, max_toi);
            }
        }
        let (mins, maxs) = (node.mins().to_array(), node.maxs().to_array());
        let mut tmin: f32 = 0.0;
        let mut tmax = max_toi;
        for i in 0..3 {
            let o = self.origin[i];
            if self.zero[i] {
                if o < mins[i] || o > maxs[i] {
                    return f32::MAX;
                }
            } else {
                let mut near = (mins[i] - o) * self.inv[i];
                let mut far = (maxs[i] - o) * self.inv[i];
                if near > far {
                    std::mem::swap(&mut near, &mut far);
                }
                tmin = tmin.max(near);
                tmax = tmax.min(far);
                if tmin > tmax {
                    return f32::MAX;
                }
            }
        }
        tmin
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
