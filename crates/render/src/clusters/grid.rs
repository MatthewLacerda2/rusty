use glam::{Mat4, Vec2, Vec3};

use super::GRID;
use crate::gpu::uniforms::ClusterUniform;
use rusty_components::Projection;
use rusty_scene::camera::depth;
use rusty_scene::Camera;

pub(crate) struct ClusterGrid {
    pub view: Mat4,
    proj: Mat4,
    inv_proj: Mat4,
    pub near: f32,
    pub far: f32,
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
            far: cam.far,
            log_slices: matches!(cam.projection, Projection::Perspective),
        }
    }

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

    fn key(&self) -> [u32; 16] {
        self.proj.to_cols_array().map(f32::to_bits)
    }

    pub(crate) fn uniform(&self) -> ClusterUniform {
        let (scale, bias) = self.slice_scale_bias();
        ClusterUniform {
            view_z: (-self.view.row(2)).to_array(),
            slices: [scale, bias, f32::from(u8::from(self.log_slices)), self.near],
            dims: [GRID[0], GRID[1], GRID[2], 0],
        }
    }

    pub(crate) fn ndc(&self, p: Vec3) -> Vec2 {
        let p = Vec3::new(p.x, p.y, p.z.min(-self.near));
        let clip = self.proj * p.extend(1.0);
        clip.truncate().truncate() / clip.w
    }

    fn ray(&self, ndc: Vec2) -> (Vec3, Vec3) {
        let near = self.inv_proj.project_point3(ndc.extend(depth::NEAR));
        (near, self.inv_proj.project_point3(ndc.extend(depth::FAR)))
    }

    pub(crate) fn cluster_aabbs(&self) -> Vec<Aabb> {
        let [gx, gy, gz] = GRID;
        // One ray per tile corner, row-major, shared by every slice.
        let rays: Vec<(Vec3, Vec3)> = (0..=gy)
            .flat_map(|j| (0..=gx).map(move |i| (i, j)))
            .map(|(i, j)| {
                let uv = Vec2::new(i as f32 / gx as f32, j as f32 / gy as f32);
                self.ray(uv * 2.0 - Vec2::ONE)
            })
            .collect();
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

/// Projections kept: a camera stack (world, viewmodel, render texture) rarely has more lenses.
const CACHED_LENSES: usize = 4;

pub(crate) type Aabb = (Vec3, Vec3);

#[derive(Default)]
pub(crate) struct AabbCache {
    entries: Vec<([u32; 16], Vec<Aabb>)>,
}

impl AabbCache {
    /// `grid`'s cluster boxes, built only when its projection is not cached.
    pub(crate) fn get(&mut self, grid: &ClusterGrid) -> &[Aabb] {
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
    /// Mirrors `common.wgsl`'s `cluster_index` line for line, so tests check the binner against the
    /// shader: change both together.
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
