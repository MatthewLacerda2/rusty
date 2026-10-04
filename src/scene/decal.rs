//! src/scene/decal.rs — box-projector decal records (bullet holes, scorch, splats).
//!
//! A decal is a *volume* (an oriented box), not a flat sticker: the render pass
//! (`render::passes::decals`) reconstructs each covered surface point from depth and
//! projects the texture along the box axes. This module is the sim-side half — the
//! record `Scene::spawn_decal` stores from a raycast hit and its FIFO cap. Plain glam
//! data, no GPU types (#494). Decals are ephemeral scene state (not an `Entity`
//! component), so they stay off the `--components` gate.

use glam::camera::rh::view::look_to_mat4;
use glam::{Mat4, Quat, Vec3};

/// Maximum decals drawn per frame. Bullet holes/scorch accumulate, but old ones
/// are evicted (FIFO) so the pass stays bounded; matches a typical FPS budget.
pub const MAX_DECALS: usize = 256;

/// One box-projector decal: an oriented volume + a texture projected through it.
/// Spawned from a surface hit (point + normal); the box is oriented so its local
/// −Z (the projection axis) points *into* the surface along the hit normal.
#[derive(Clone, Debug)]
pub struct Decal {
    /// World-space centre of the projector box.
    pub position: Vec3,
    /// Box orientation. Local −Z is the projection direction (into the surface).
    pub rotation: Quat,
    /// Full box extents (width, height, depth) in world units. Width/height size
    /// the stamp on the surface; depth is how far the projector reaches.
    pub size: Vec3,
    /// RGBA tint multiplied into the sampled texel (alpha scales the blend).
    pub color: [f32; 4],
    /// Decal texture path, or `None` for the default checker.
    pub texture: Option<String>,
}

impl Decal {
    /// Build a decal at a surface hit. `point` is the world hit position, `normal`
    /// the (outward) surface normal. `size` is width/height of the stamp; `depth`
    /// how far the box projects through the surface. The box straddles the surface
    /// and is oriented so its local −Z aims into the surface (along −normal).
    #[allow(clippy::too_many_arguments)]
    pub fn from_hit(
        point: Vec3,
        normal: Vec3,
        size: f32,
        depth: f32,
        rotation_deg: f32,
        color: [f32; 4],
        texture: Option<String>,
    ) -> Self {
        let n = normal.normalize_or_zero();
        let n = if n.length_squared() < 1.0e-6 {
            Vec3::Y
        } else {
            n
        };
        // Orient so local −Z (the projection axis) runs along −normal, into the
        // surface. `quat_look_along` builds a basis whose −Z points along `dir`.
        let mut rot = quat_look_along(-n);
        // Spin the stamp around the projection axis for variety (bullet holes).
        rot *= Quat::from_axis_angle(Vec3::Z, rotation_deg.to_radians());
        // Centre the box on the surface so the projector straddles it; the depth
        // gives margin on both sides to catch the wrapped geometry.
        let centre = point + n * (depth * 0.5);
        Self {
            position: centre,
            rotation: rot,
            size: Vec3::new(size, size, depth),
            color,
            texture,
        }
    }

    /// Object→world matrix mapping the unit cube `[-0.5, 0.5]³` to this box.
    pub fn model_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.size, self.rotation, self.position)
    }
}

/// Rotation whose local −Z axis points along `dir` (a "look in direction" basis),
/// with a stable up-vector fallback when `dir` is near-vertical.
fn quat_look_along(dir: Vec3) -> Quat {
    let f = dir.normalize_or_zero();
    let up = if f.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    // look_to_rh builds a view rotation; its inverse orients an object's −Z to dir.
    Quat::from_mat4(&look_to_mat4(Vec3::ZERO, f, up)).inverse()
}

impl crate::scene::Scene {
    /// Spawn a box-projector decal at a surface hit (the point + outward normal
    /// already produced by `Physics.Raycast`). `size` is the
    /// stamp's width/height in world units; `depth` how far the box projects
    /// through the surface; `rotation_deg` spins the stamp around its axis;
    /// `color` tints the texel (alpha scales the blend); `texture` is the decal
    /// sprite (or the default checker). The registry is a bounded FIFO so spam
    /// can't grow it without limit.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_decal(
        &mut self,
        point: Vec3,
        normal: Vec3,
        size: f32,
        depth: f32,
        rotation_deg: f32,
        color: [f32; 4],
        texture: Option<String>,
    ) {
        let decal = crate::scene::decal::Decal::from_hit(
            point,
            normal,
            size.max(1.0e-4),
            depth.max(1.0e-4),
            rotation_deg,
            color,
            texture,
        );
        if self.decals.len() >= crate::scene::decal::MAX_DECALS {
            self.decals.remove(0);
        }
        self.decals.push(decal);
    }

    /// Drop every live decal (e.g. on level reset).
    pub fn clear_decals(&mut self) {
        self.decals.clear();
    }
}
