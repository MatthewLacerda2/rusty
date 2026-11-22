use glam::{Mat4, Vec3};

use super::transform_aabb;
use crate::GpuMesh;
use rusty_components::mesh::Vertex;
use rusty_components::MeshComponent;

/// Twice `blend_joints`' 0.01 (`common.wgsl`), so an edge vertex is boxed as both rest and skinned.
const IDENTITY_BELOW: f32 = 0.02;

type Aabb = (Vec3, Vec3);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkinBounds {
    joints: Vec<Option<Aabb>>,
    rest: Option<Aabb>,
    unbounded: bool,
}

impl SkinBounds {
    pub fn from_vertices(vertices: &[Vertex]) -> Self {
        let mut bounds = Self::default();
        for v in vertices {
            let p = Vec3::from_array(v.position);
            let w = v.joint_weights;
            if w.iter().any(|&wi| wi < 0.0) {
                bounds.unbounded = true;
            }
            if w[0] + w[1] + w[2] + w[3] < IDENTITY_BELOW {
                grow(&mut bounds.rest, p);
            }
            for (&joint, &weight) in v.joint_indices.iter().zip(&w) {
                if weight > 0.0 {
                    let slot = joint as usize;
                    if bounds.joints.len() <= slot {
                        bounds.joints.resize(slot + 1, None);
                    }
                    grow(&mut bounds.joints[slot], p);
                }
            }
        }
        bounds
    }

    pub fn posed(&self, palette: &[Mat4]) -> Option<Aabb> {
        if self.unbounded {
            return None;
        }
        let mut posed = self.rest;
        for (slot, joint) in self.joints.iter().enumerate() {
            let Some((min, max)) = *joint else {
                continue;
            };
            let (lo, hi) = transform_aabb(min, max, *palette.get(slot)?);
            posed = Some(posed.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
        }
        posed
    }
}

fn grow(aabb: &mut Option<Aabb>, p: Vec3) {
    *aabb = Some(aabb.map_or((p, p), |(min, max)| (min.min(p), max.max(p))));
}

impl GpuMesh {
    /// `None` when the pose cannot be bounded (a negative weight, a joint past the
    /// palette): never cull it.
    pub(crate) fn world_bounds(&self, mesh: &MeshComponent, world: Mat4) -> Option<Aabb> {
        let (min, max) = if mesh.is_skinned() {
            self.skin_bounds.posed(mesh.active_palette())?
        } else {
            self.local_aabb
        };
        Some(transform_aabb(min, max, world))
    }
}

#[cfg(test)]
#[path = "skinned_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "skinned_weight_tests.rs"]
mod weight_tests;
