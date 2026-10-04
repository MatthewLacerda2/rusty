//! The lightmap bake's ray-triangle acceleration structure (#438): a bounding-volume
//! hierarchy over every static triangle, split at the centroid median of the widest
//! axis. Plain glam, no physics engine: the bake needs the triangle and barycentrics
//! a hit landed on (to shade it), which a collider query does not hand back, and the
//! build is a pure function of the triangles, so the bake stays deterministic.

use glam::Vec3;

/// Triangles per leaf; past this a node splits.
const LEAF_SIZE: usize = 4;

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

    fn bounds(&self) -> (Vec3, Vec3) {
        let (b, c) = (self.a + self.e1, self.a + self.e2);
        (self.a.min(b).min(c), self.a.max(b).max(c))
    }

    /// Möller–Trumbore, two-sided: `(t, u, v)` for a hit in `(t_min, t_max)`.
    fn intersect(
        &self,
        origin: Vec3,
        dir: Vec3,
        t_min: f32,
        t_max: f32,
    ) -> Option<(f32, f32, f32)> {
        let p = dir.cross(self.e2);
        let det = self.e1.dot(p);
        if det.abs() < 1e-12 {
            return None;
        }
        let inv = 1.0 / det;
        let s = origin - self.a;
        let u = s.dot(p) * inv;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let q = s.cross(self.e1);
        let v = dir.dot(q) * inv;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let t = self.e2.dot(q) * inv;
        (t > t_min && t < t_max).then_some((t, u, v))
    }
}

/// The nearest hit along a ray: the triangle, and the barycentrics of `b` and `c`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Hit {
    pub tri: usize,
    pub u: f32,
    pub v: f32,
}

/// A node: its bounds, and either a leaf run of `order` or the index of its second
/// child (the first child always follows it directly).
struct Node {
    min: Vec3,
    max: Vec3,
    start: u32,
    count: u32,
    right: u32,
}

pub(super) struct Bvh {
    pub tris: Vec<Tri>,
    order: Vec<u32>,
    nodes: Vec<Node>,
}

impl Bvh {
    pub(super) fn build(tris: Vec<Tri>) -> Self {
        let mut bvh = Bvh {
            order: (0..tris.len() as u32).collect(),
            tris,
            nodes: Vec::new(),
        };
        if !bvh.tris.is_empty() {
            bvh.split(0, bvh.tris.len());
        }
        bvh
    }

    /// Build the node over `order[start..end]`, returning its index.
    fn split(&mut self, start: usize, end: usize) -> u32 {
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let (mut cmin, mut cmax) = (min, max);
        for &i in &self.order[start..end] {
            let (lo, hi) = self.tris[i as usize].bounds();
            (min, max) = (min.min(lo), max.max(hi));
            let c = (lo + hi) * 0.5;
            (cmin, cmax) = (cmin.min(c), cmax.max(c));
        }
        let index = self.nodes.len() as u32;
        let count = (end - start) as u32;
        self.nodes.push(Node {
            min,
            max,
            start: start as u32,
            count,
            right: 0,
        });
        let extent = cmax - cmin;
        if end - start <= LEAF_SIZE || extent.max_element() <= 0.0 {
            return index;
        }
        let axis = if extent.x >= extent.y && extent.x >= extent.z {
            0
        } else if extent.y >= extent.z {
            1
        } else {
            2
        };
        let mid = (start + end) / 2;
        let tris = &self.tris;
        let key = |i: &u32| {
            let (lo, hi) = tris[*i as usize].bounds();
            (lo[axis] + hi[axis], *i)
        };
        self.order[start..end].select_nth_unstable_by(mid - start, |a, b| {
            let (ka, kb) = (key(a), key(b));
            ka.0.total_cmp(&kb.0).then(ka.1.cmp(&kb.1))
        });
        self.nodes[index as usize].count = 0;
        self.split(start, mid);
        let right = self.split(mid, end);
        self.nodes[index as usize].right = right;
        index
    }

    /// The nearest hit along `origin + t * dir` with `t` in `(t_min, t_max)`.
    pub(super) fn closest(&self, origin: Vec3, dir: Vec3, t_min: f32, t_max: f32) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        let mut limit = t_max;
        self.walk(origin, dir, t_min, &mut limit, &mut |tri, _, u, v| {
            best = Some(Hit { tri, u, v });
            false
        });
        best
    }

    /// Whether anything blocks `origin + t * dir` for `t` in `(t_min, t_max)`.
    pub(super) fn occluded(&self, origin: Vec3, dir: Vec3, t_min: f32, t_max: f32) -> bool {
        let mut blocked = false;
        let mut limit = t_max;
        self.walk(origin, dir, t_min, &mut limit, &mut |_, _, _, _| {
            blocked = true;
            true
        });
        blocked
    }

    /// Visit every hit nearer than `limit`, shrinking it as hits land. `on_hit`
    /// returns `true` to stop the walk.
    fn walk(
        &self,
        origin: Vec3,
        dir: Vec3,
        t_min: f32,
        limit: &mut f32,
        on_hit: &mut dyn FnMut(usize, f32, f32, f32) -> bool,
    ) {
        if self.nodes.is_empty() {
            return;
        }
        let inv = dir.recip();
        let mut stack = vec![0u32];
        while let Some(n) = stack.pop() {
            let node = &self.nodes[n as usize];
            if !slab(node.min, node.max, origin, inv, *limit) {
                continue;
            }
            if node.count == 0 {
                stack.push(node.right);
                stack.push(n + 1);
                continue;
            }
            let run = node.start as usize..(node.start + node.count) as usize;
            for &i in &self.order[run] {
                if let Some((t, u, v)) = self.tris[i as usize].intersect(origin, dir, t_min, *limit)
                {
                    *limit = t;
                    if on_hit(i as usize, t, u, v) {
                        return;
                    }
                }
            }
        }
    }
}

/// Whether a ray (by its inverse direction) meets the box before `limit`.
fn slab(min: Vec3, max: Vec3, origin: Vec3, inv: Vec3, limit: f32) -> bool {
    let (a, b) = ((min - origin) * inv, (max - origin) * inv);
    let near = a.min(b).max_element().max(0.0);
    let far = a.max(b).min_element().min(limit);
    near <= far
}
