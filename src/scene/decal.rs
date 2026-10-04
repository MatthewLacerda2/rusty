//! src/scene/decal.rs — box-projector decal records (bullet holes, scorch, splats).
//!
//! A decal is a *volume* (an oriented box), not a flat sticker. The renderer
//! (`render::decals`, #638) bins each box into the light clusters, and the forward
//! shader folds the decal into the material inputs of every surface point inside
//! the box before that surface is lit. This module is the sim-side half — the
//! record `Scene::spawn_decal` stores from a raycast hit and its FIFO cap. Plain glam
//! data, no GPU types (#494). Decals are ephemeral scene state (not an `Entity`
//! component), so they stay off the `--components` gate.
//!
//! **Order.** Decals blend in spawn order (FIFO): a newer decal lands over an older
//! one where they overlap, and the cap evicts the oldest first.

use glam::camera::rh::view::look_to_mat4;
use glam::{Mat4, Quat, Vec3};

/// Maximum decals drawn per frame. Bullet holes/scorch accumulate, but old ones
/// are evicted (FIFO) so the pass stays bounded; matches a typical FPS budget.
pub const MAX_DECALS: usize = 256;

/// How a decal is stamped, beyond where: everything `Decals.Spawn` takes after the
/// hit point and normal. [`Default`] is the positional form's defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct DecalSpec {
    /// Width and height of the stamp, in world units.
    pub size: f32,
    /// How far the projector box reaches through the surface.
    pub depth: f32,
    /// Spin around the projection axis, in degrees.
    pub rotation_deg: f32,
    /// RGBA tint, multiplied into the decal's albedo (alpha scales its coverage).
    pub color: [f32; 4],
    /// Albedo texture path when no material names one; `None` stamps a solid square.
    pub texture: Option<String>,
    /// The decal material (#638): a library material whose maps and `decal` block
    /// say what the decal changes. `None` changes only the albedo, by `texture`.
    pub material: Option<String>,
}

impl Default for DecalSpec {
    fn default() -> Self {
        Self {
            size: 0.5,
            depth: 0.5,
            rotation_deg: 0.0,
            color: [1.0; 4],
            texture: None,
            material: None,
        }
    }
}

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
    /// Albedo texture path when no material names one, or `None` for a solid square.
    pub texture: Option<String>,
    /// The library material this decal stamps (#638), by name; see [`DecalSpec`].
    pub material: Option<String>,
}

impl Decal {
    /// Build a decal at a surface hit. `point` is the world hit position, `normal`
    /// the (outward) surface normal; `spec` sizes, spins and dresses the stamp. The
    /// box straddles the surface and is oriented so its local −Z aims into the
    /// surface (along −normal).
    pub fn from_hit(point: Vec3, normal: Vec3, spec: DecalSpec) -> Self {
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
        rot *= Quat::from_axis_angle(Vec3::Z, spec.rotation_deg.to_radians());
        // Centre the box on the hit so the projector straddles the surface: half
        // its depth in front to catch bumps, half behind to catch dents. (Before
        // #638 the box sat wholly in front, so the surface lay on its faded cap.)
        let depth = spec.depth.max(1.0e-4);
        let size = spec.size.max(1.0e-4);
        Self {
            position: point,
            rotation: rot,
            size: Vec3::new(size, size, depth),
            color: spec.color,
            texture: spec.texture,
            material: spec.material,
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
    /// already produced by `Physics.Raycast`), dressed by `spec`. The registry is a
    /// bounded FIFO so spam can't grow it without limit.
    pub fn spawn_decal(&mut self, point: Vec3, normal: Vec3, spec: DecalSpec) {
        let decal = Decal::from_hit(point, normal, spec);
        if self.decals.len() >= MAX_DECALS {
            self.decals.remove(0);
        }
        self.decals.push(decal);
    }

    /// Drop every live decal (e.g. on level reset).
    pub fn clear_decals(&mut self) {
        self.decals.clear();
    }
}
