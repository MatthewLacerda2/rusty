//! src/render/culling.rs — view-frustum culling (#330).
//!
//! A [`Frustum`] is the six half-spaces of a camera's (or light's) view-projection
//! volume, extracted once per camera per frame via the standard Gribb–Hartmann method.
//! An entity is submitted only when its world-space AABB intersects the frustum; a
//! reject skips the per-entity uniform sync, bind-group work, and draw entirely, so CPU
//! submission cost scales with the *visible* set rather than the whole scene.
//!
//! The AABB tested is the mesh's LOCAL-space box (cached once at GPU upload, see
//! `GpuMesh`) transformed by the entity's world matrix — 8 corners, O(1) — never a
//! per-frame walk of the vertices. This module is pure geometry: it reads no engine
//! state and mutates nothing, so it is confined to the render stage and cannot touch
//! determinism or the headless replay.

use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};

/// The six clip-space planes of a view-projection volume. Each plane is stored as a
/// `Vec4` `(a, b, c, d)` whose `xyz` is the inward-pointing normal; a point `p` is
/// inside the half-space when `a·p.x + b·p.y + c·p.z + d ≥ 0`. Planes are left
/// un-normalized — only the SIGN of that dot matters for an inside/outside test, so the
/// normalization is wasted work here.
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// Extract the six planes from a combined view-projection matrix (Gribb–Hartmann).
    /// Uses the `[0, 1]` clip-space depth convention that glam/wgpu produce, so the near
    /// plane is `row2` (z ≥ 0) rather than `row3 + row2`; the side/top/bottom planes are
    /// convention-independent. Works for perspective and orthographic matrices alike, so
    /// the same routine serves both the camera stack and the directional light's ortho
    /// shadow volume.
    pub fn from_view_projection(vp: Mat4) -> Self {
        let r0 = vp.row(0);
        let r1 = vp.row(1);
        let r2 = vp.row(2);
        let r3 = vp.row(3);
        Self {
            planes: [
                r3 + r0, // left:   w + x ≥ 0
                r3 - r0, // right:  w - x ≥ 0
                r3 + r1, // bottom: w + y ≥ 0
                r3 - r1, // top:    w - y ≥ 0
                r2,      // near:   z ≥ 0        (glam/wgpu [0,1] depth)
                r3 - r2, // far:    w - z ≥ 0
            ],
        }
    }

    /// True when a world-space AABB `[min, max]` is at least partially inside the
    /// frustum (conservative: it may keep a box that straddles a plane corner but never
    /// rejects one that is actually visible). Uses the positive-vertex test — the AABB
    /// corner farthest along each plane normal; if even that corner is outside a plane,
    /// the whole box is outside.
    pub fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            let n = plane.xyz();
            let p = Vec3::new(
                if n.x >= 0.0 { max.x } else { min.x },
                if n.y >= 0.0 { max.y } else { min.y },
                if n.z >= 0.0 { max.z } else { min.z },
            );
            if n.dot(p) + plane.w < 0.0 {
                return false;
            }
        }
        true
    }
}

/// The world-space AABB of a local-space box `[local_min, local_max]` under `world`:
/// transform all 8 corners and take their component-wise min/max. O(1) per entity — the
/// per-frame transform of the cached local AABB the frustum test consumes.
pub fn transformed_aabb(local_min: Vec3, local_max: Vec3, world: Mat4) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for i in 0..8 {
        let corner = Vec3::new(
            if i & 1 == 0 { local_min.x } else { local_max.x },
            if i & 2 == 0 { local_min.y } else { local_max.y },
            if i & 4 == 0 { local_min.z } else { local_max.z },
        );
        let world_corner = world.transform_point3(corner);
        min = min.min(world_corner);
        max = max.max(world_corner);
    }
    (min, max)
}

#[cfg(test)]
#[path = "culling_tests.rs"]
mod culling_tests;
