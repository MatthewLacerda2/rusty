//! One camera's cluster grid (#434): which view-space box each cluster covers, and
//! the numbers a shader needs to find a point's cluster. The shader-side twin is
//! `cluster_index` in `common.wgsl`; the two must agree on tiles and slices.

use glam::{Mat4, Vec2, Vec3};

use super::GRID;
use crate::components::Projection;
use crate::render::gpu::uniforms::ClusterUniform;
use crate::scene::Camera;

/// A camera's view of the cluster grid. Depth is distance along the view axis.
pub(crate) struct ClusterGrid {
    pub view: Mat4,
    proj: Mat4,
    inv_proj: Mat4,
    pub near: f32,
    pub far: f32,
    /// Exponential depth slices (perspective) rather than linear (orthographic).
    log_slices: bool,
}

impl ClusterGrid {
    pub(crate) fn new(cam: &Camera, aspect: f32) -> Self {
        let proj = cam.projection_matrix(aspect);
        Self {
            view: cam.view_matrix(),
            proj,
            inv_proj: proj.inverse(),
            near: cam.near,
            far: cam.far.max(cam.near + 0.001),
            log_slices: matches!(cam.projection, Projection::Perspective),
        }
    }

    /// `(scale, bias)` with `slice = f(depth) * scale + bias`, where `f` is `log2`
    /// for exponential slices and the identity for linear ones.
    fn slice_scale_bias(&self) -> (f32, f32) {
        let z = GRID[2] as f32;
        if self.log_slices {
            let scale = z / (self.far / self.near).log2();
            (scale, -self.near.log2() * scale)
        } else {
            let scale = z / (self.far - self.near);
            (scale, -self.near * scale)
        }
    }

    /// The depth slice (fractional, unclamped) a view depth falls in.
    pub(crate) fn slice_of(&self, depth: f32) -> f32 {
        let (scale, bias) = self.slice_scale_bias();
        let d = depth.max(self.near);
        let f = if self.log_slices { d.log2() } else { d };
        f * scale + bias
    }

    /// The view depth where slice `k` starts (`k == GRID[2]` is the far plane).
    pub(crate) fn slice_depth(&self, k: u32) -> f32 {
        let t = k as f32 / GRID[2] as f32;
        if self.log_slices {
            self.near * (self.far / self.near).powf(t)
        } else {
            self.near + (self.far - self.near) * t
        }
    }

    /// The cache key of this grid's cluster boxes: they depend on the projection
    /// alone (near, far, lens), not on where the camera stands.
    fn key(&self) -> [u32; 16] {
        self.proj.to_cols_array().map(f32::to_bits)
    }

    /// What the shader needs to find a point's cluster.
    pub(crate) fn uniform(&self) -> ClusterUniform {
        let (scale, bias) = self.slice_scale_bias();
        ClusterUniform {
            view_z: (-self.view.row(2)).to_array(),
            slices: [scale, bias, f32::from(u8::from(self.log_slices)), self.near],
            dims: [GRID[0], GRID[1], GRID[2], 0],
        }
    }

    /// A view-space point's NDC `xy`, with its depth clamped to the near plane so a
    /// point behind the camera still bounds conservatively.
    pub(crate) fn ndc(&self, p: Vec3) -> Vec2 {
        let p = Vec3::new(p.x, p.y, p.z.min(-self.near));
        let clip = self.proj * p.extend(1.0);
        clip.truncate().truncate() / clip.w
    }

    /// The view-space ray through NDC `xy`: its points on the near and far planes.
    fn ray(&self, ndc: Vec2) -> (Vec3, Vec3) {
        let near = self.inv_proj.project_point3(ndc.extend(0.0));
        (near, self.inv_proj.project_point3(ndc.extend(1.0)))
    }

    /// Every cluster's view-space AABB, indexed `x + X * (y + Y * slice)`: the
    /// bounds of its tile's four corner rays cut at the slice's two depths.
    pub(crate) fn cluster_aabbs(&self) -> Vec<(Vec3, Vec3)> {
        let [gx, gy, gz] = GRID;
        // One ray per tile corner, row-major, shared by every slice.
        let rays: Vec<(Vec3, Vec3)> = (0..=gy)
            .flat_map(|j| (0..=gx).map(move |i| (i, j)))
            .map(|(i, j)| {
                let uv = Vec2::new(i as f32 / gx as f32, j as f32 / gy as f32);
                self.ray(uv * 2.0 - Vec2::ONE)
            })
            .collect();
        // The point on ray `(a, b)` at view depth `d` (depth is -z in view space).
        let at = |(a, b): (Vec3, Vec3), d: f32| a + (b - a) * ((d + a.z) / (a.z - b.z));
        let mut out = Vec::with_capacity(super::CLUSTER_COUNT);
        for k in 0..gz {
            let depths = [self.slice_depth(k), self.slice_depth(k + 1)];
            for j in 0..gy {
                for i in 0..gx {
                    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
                    for (di, dj) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let ray = rays[((j + dj) * (gx + 1) + i + di) as usize];
                        for d in depths {
                            let p = at(ray, d);
                            (lo, hi) = (lo.min(p), hi.max(p));
                        }
                    }
                    out.push((lo, hi));
                }
            }
        }
        out
    }
}

/// How many projections' cluster boxes stay cached: a camera stack (world, a
/// viewmodel, a render texture) rarely has more distinct lenses than this.
const CACHED_LENSES: usize = 4;

/// Cluster boxes by projection, newest last, so an unchanged lens rebuilds nothing.
#[derive(Default)]
pub(crate) struct AabbCache {
    entries: Vec<([u32; 16], Vec<(Vec3, Vec3)>)>,
}

impl AabbCache {
    /// `grid`'s cluster boxes, built only when its projection is not cached.
    pub(crate) fn get(&mut self, grid: &ClusterGrid) -> &[(Vec3, Vec3)] {
        let key = grid.key();
        match self.entries.iter().position(|(k, _)| *k == key) {
            Some(at) => {
                let hit = self.entries.remove(at);
                self.entries.push(hit);
            }
            None => {
                if self.entries.len() == CACHED_LENSES {
                    self.entries.remove(0);
                }
                self.entries.push((key, grid.cluster_aabbs()));
            }
        }
        &self.entries.last().expect("just pushed").1
    }
}

#[cfg(test)]
impl ClusterGrid {
    /// The cluster a world point falls in: `cluster_index` in `common.wgsl`, line for
    /// line, so the tests can check the binner against what the shader will read.
    pub(crate) fn cluster_at(&self, view_proj: Mat4, world: Vec3) -> u32 {
        let u = self.uniform();
        let clip = view_proj * world.extend(1.0);
        let ndc = clip.truncate().truncate() / clip.w.max(1e-6);
        let tiles = Vec2::new(u.dims[0] as f32, u.dims[1] as f32);
        let tile = ((ndc * 0.5 + 0.5) * tiles)
            .floor()
            .clamp(Vec2::ZERO, tiles - 1.0);
        let depth = (Vec3::from_slice(&u.view_z[..3]).dot(world) + u.view_z[3]).max(u.slices[3]);
        let f = if u.slices[2] > 0.5 {
            depth.log2()
        } else {
            depth
        };
        let slice = (f * u.slices[0] + u.slices[1])
            .floor()
            .clamp(0.0, (u.dims[2] - 1) as f32);
        tile.x as u32 + u.dims[0] * (tile.y as u32 + u.dims[1] * slice as u32)
    }
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;
