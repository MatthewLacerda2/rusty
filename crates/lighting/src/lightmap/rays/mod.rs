use glam::Vec3;
use rapier3d::parry::partitioning::{BvhNode, TraversalAction};
use rapier3d::parry::query::details::local_ray_intersection_with_triangle;
use rapier3d::parry::query::Ray;
use rapier3d::parry::shape::TriMesh;

#[derive(Clone, Copy)]
pub(crate) struct Tri {
    pub a: Vec3,
    pub e1: Vec3,
    pub e2: Vec3,
    pub mesh: u32,
    pub first: u32,
    pub seen: bool,
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

    pub(super) fn with(self, seen: bool, blocks: bool) -> Self {
        Tri {
            seen,
            blocks,
            ..self
        }
    }

    pub(crate) fn normal(&self) -> Vec3 {
        self.e1.cross(self.e2)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Hit {
    pub tri: usize,
    pub u: f32,
    pub v: f32,
}

pub(crate) struct Rays {
    pub tris: Vec<Tri>,
    mesh: Option<TriMesh>,
    pub cast: std::sync::atomic::AtomicU64,
    pub direct: std::sync::atomic::AtomicU64,
    counting: bool,
}

impl Rays {
    pub(crate) fn build(tris: Vec<Tri>) -> Self {
        if tris.is_empty() {
            return Rays {
                tris,
                mesh: None,
                cast: Default::default(),
                direct: Default::default(),
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
        let mesh = TriMesh::new(vertices, indices).expect("a non-empty triangle soup");
        Rays {
            tris,
            mesh: Some(mesh),
            cast: Default::default(),
            direct: Default::default(),
            // lint: allow(cfg-test-switch) scene's own tests; another crate counts through `Rays::counted`
            counting: cfg!(test),
        }
    }

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

    pub(super) fn occluded(&self, origin: Vec3, dir: Vec3, t_min: f32, t_max: f32) -> bool {
        self.occluded_by(origin, dir, (t_min, t_max), |i| self.tris[i].blocks)
    }

    pub(crate) fn occluded_by(
        &self,
        origin: Vec3,
        dir: Vec3,
        range: (f32, f32),
        blocks: impl Fn(usize) -> bool,
    ) -> bool {
        self.blocker(origin, dir, range, blocks).is_some()
    }

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

    #[cfg(feature = "dev")]
    pub(crate) fn counted(self) -> Self {
        Self {
            counting: true,
            ..self
        }
    }

    pub(crate) fn count_direct(&self) {
        if self.counting {
            self.direct
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn count(&self) {
        if self.counting {
            self.cast.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct NodeRay {
    origin: [f32; 3],
    zero: [bool; 3],
    inv: [f32; 3],
}

// Keep parry's operations in parry's order (no fused multiply-add):
// `node_tests` compares it bit for bit with parry's own node test.
impl NodeRay {
    pub(super) fn new(ray: &Ray) -> Self {
        let d = ray.dir.to_array();
        NodeRay {
            origin: ray.origin.to_array(),
            zero: d.map(|c| c == 0.0),
            inv: d.map(|c| 1.0 / c),
        }
    }

    pub(super) fn cast(&self, node: &BvhNode, max_toi: f32) -> f32 {
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

fn hit(mesh: &TriMesh, i: u32, ray: &Ray, reach: f32) -> Option<(f32, Vec3)> {
    let t = mesh.triangle(i);
    let (inter, weights) = local_ray_intersection_with_triangle(t.a, t.b, t.c, ray)?;
    (inter.time_of_impact < reach).then_some((inter.time_of_impact, weights))
}

#[cfg(test)]
mod node_tests;
#[cfg(test)]
mod tests;
