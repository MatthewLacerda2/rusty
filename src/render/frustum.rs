//! src/render/frustum.rs — view-frustum extraction and AABB culling (#330).
//!
//! Frustum culling skips the CPU submission work (uniform sync, bind-group binds, the
//! draw call) for entities the camera cannot see, so per-frame cost tracks the *visible*
//! set instead of the whole scene. This is render-stage only — the sim never reads
//! visibility — so determinism and headless replay are untouched by construction.
//!
//! The six planes are pulled from a view-projection matrix by the standard
//! Gribb–Hartmann method. glam builds wgpu/D3D-style clip volumes (depth `0..w`), so the
//! near plane is `row2` (z ≥ 0), not `row3 + row2` as in the OpenGL `-w..w` derivation.
//! The camera pass extracts against the camera's view-proj; each shadow collect extracts
//! against the *light's* view-proj, so an off-screen caster still inside the light volume
//! is kept — culling shadow casters by the camera is the classic pop-a-shadow bug.

use glam::{Mat4, Vec3, Vec4};

/// Six frustum planes in world space, each `(a, b, c, d)` with a point `p` inside the
/// half-space when `a·pₓ + b·p_y + c·p_z + d ≥ 0` (planes point inward).
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// Extract the six planes from a view-projection matrix (Gribb–Hartmann). Works for
    /// perspective *and* orthographic matrices, so the same path serves the camera passes
    /// and the directional light's ortho shadow volume.
    pub fn from_view_proj(view_proj: Mat4) -> Self {
        let r0 = view_proj.row(0);
        let r1 = view_proj.row(1);
        let r2 = view_proj.row(2);
        let r3 = view_proj.row(3);
        Self {
            planes: [
                r3 + r0, // left
                r3 - r0, // right
                r3 + r1, // bottom
                r3 - r1, // top
                r2,      // near (wgpu/D3D depth is 0..w, so the near plane is just row2)
                r3 - r2, // far
            ],
        }
    }

    /// Conservative test: `true` if the world-space AABB is at least partially inside the
    /// frustum. Uses the p-vertex (the box corner farthest along each plane normal): if
    /// that corner is behind a plane, the whole box is outside and can be culled. The test
    /// may keep a box that is technically outside a corner edge (false-positive), but never
    /// rejects one that is partially visible — exactly the safe direction for culling.
    pub fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            let n = plane.truncate();
            let p_vertex = Vec3::new(
                if n.x >= 0.0 { max.x } else { min.x },
                if n.y >= 0.0 { max.y } else { min.y },
                if n.z >= 0.0 { max.z } else { min.z },
            );
            if n.dot(p_vertex) + plane.w < 0.0 {
                return false;
            }
        }
        true
    }
}

/// The world-space AABB of a local-space AABB under `world`: transform all eight corners
/// and take their min/max. O(1) — the whole point of caching the local AABB (#330) is that
/// this never touches the mesh vertices.
pub fn transform_aabb(local_min: Vec3, local_max: Vec3, world: Mat4) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for i in 0..8 {
        let corner = Vec3::new(
            if i & 1 == 0 { local_min.x } else { local_max.x },
            if i & 2 == 0 { local_min.y } else { local_max.y },
            if i & 4 == 0 { local_min.z } else { local_max.z },
        );
        let w = world.transform_point3(corner);
        min = min.min(w);
        max = max.max(w);
    }
    (min, max)
}

#[cfg(test)]
#[path = "frustum_tests.rs"]
mod frustum_tests;
