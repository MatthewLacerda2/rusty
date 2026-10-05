//! The animated bound a skinned mesh is culled by (#833).
//!
//! A skinned mesh's rest-pose AABB is wrong the moment a limb moves, so before #833
//! skinned meshes were never culled. [`SkinBounds`] keeps, per joint, the mesh-local
//! box of the bind-pose vertices that joint weights — computed once when the mesh is
//! uploaded. Each frame, every joint's box is carried by that joint's palette matrix
//! (the very matrices the shader skins with) and the union is the posed bound.
//!
//! Why it always contains the posed mesh: the shader draws a vertex at
//! `Σ wᵢ·Pᵢ·v` (homogeneous). With non-negative weights summing to `S > 0`, the
//! projected point is `Σ (wᵢ/S)·Pᵢ·v` — a convex combination of points `Pᵢ·v`, each
//! inside joint `i`'s carried box, so it lies inside their union's AABB. The weights
//! need not sum to 1: the perspective divide normalises them. A vertex whose weights
//! sum to under 0.01 is skinned as identity by the shader (`blend_joints`), so it
//! stays where it is bound; those are kept in a separate rest box.
//!
//! What it cannot bound returns `None`, and the caller never culls: a negative weight
//! (not a convex combination) or a weighted joint the palette does not have (the
//! shader would read another draw's matrices). A popped-in limb is worse than a
//! wasted draw.

use glam::{Mat4, Vec3};

use super::transform_aabb;
use crate::components::mesh::Vertex;
use crate::components::MeshComponent;
use crate::render::GpuMesh;

/// Below this total weight the shader skins a vertex as identity (`blend_joints`'s
/// `0.01`). Doubled so a vertex on the edge is counted both ways, whichever side the
/// GPU's arithmetic puts it.
const IDENTITY_BELOW: f32 = 0.02;

type Aabb = (Vec3, Vec3);

/// Per-joint bind-pose boxes of a mesh's vertices, the input of its animated bound.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkinBounds {
    /// Indexed by joint slot: the mesh-local box of every vertex with a positive
    /// weight on that joint, `None` for a joint no vertex uses.
    joints: Vec<Option<Aabb>>,
    /// The box of the vertices the shader leaves unskinned (near-zero total weight).
    rest: Option<Aabb>,
    /// A vertex carries a negative weight: no convex bound exists.
    unbounded: bool,
}

impl SkinBounds {
    /// One pass over `vertices`, done once per mesh upload.
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

    /// The mesh-local box holding the mesh skinned by `palette`, or `None` when it
    /// cannot be bounded (see the module doc) — the caller must then not cull.
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

/// Extend `aabb` (or start it) to hold `p`.
fn grow(aabb: &mut Option<Aabb>, p: Vec3) {
    *aabb = Some(aabb.map_or((p, p), |(min, max)| (min.min(p), max.max(p))));
}

impl GpuMesh {
    /// The world-space box `mesh` can occupy this frame under `world`, for frustum
    /// culling (#330): the cached rest AABB for a static mesh, the posed bound for a
    /// skinned one (#833). `None` means "cannot tell" — never cull it.
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
