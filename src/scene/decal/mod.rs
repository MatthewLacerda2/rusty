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
//!
//! **Lifecycle (#639).** A decal may stick to an *owner* entity (its box is kept in
//! the owner's space and follows it), may have a lifetime with a fade-out, and has
//! an id a script can remove it by. Ageing runs on the fixed sim tick
//! ([`DecalSet::tick`]), never the wall clock; the registry is in [`registry`].

use glam::camera::rh::view::look_to_mat4;
use glam::{Mat4, Quat, Vec3};

mod pose;
mod registry;

pub use pose::DecalPose;
pub use registry::{DecalSet, EVICTION_FADE, EVICTION_HEADROOM, MAX_DECALS};

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
    /// The entity the decal sticks to (#639): its box follows that entity's world
    /// matrix, hides while it is inactive and is dropped when it is destroyed.
    pub owner: Option<u32>,
    /// Seconds of sim time the decal lives, or `None` to live until evicted.
    pub lifetime: Option<f32>,
    /// Seconds over which it fades out at the end of its `lifetime`.
    pub fade: f32,
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
            owner: None,
            lifetime: None,
            fade: 0.0,
        }
    }
}

/// One box-projector decal: an oriented volume + a texture projected through it.
/// Spawned from a surface hit (point + normal); the box is oriented so its local
/// −Z (the projection axis) points *into* the surface along the hit normal.
#[derive(Clone, Debug, PartialEq)]
pub struct Decal {
    /// The handle `Decals.Spawn` returns (#639); unique for the scene's life.
    pub id: u32,
    /// World-space centre of the projector box *at spawn*; an owned decal's box
    /// is resolved each frame from its [`DecalOwner`] instead.
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
    /// The entity it sticks to and its box in that entity's space, if any.
    pub owner: Option<DecalOwner>,
    /// Sim seconds since it was stamped.
    pub age: f32,
    /// Age at which it is gone, if it ever is.
    pub lifetime: Option<f32>,
    /// Seconds of fade-out that end at `lifetime`.
    pub fade: f32,
    /// Fading out early (evicted by the cap or removed with a fade): no longer
    /// counted against [`MAX_DECALS`].
    pub retiring: bool,
}

/// What an owned decal sticks to: the entity, and the box (the unit cube's
/// object→owner matrix) in that entity's space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecalOwner {
    pub entity: u32,
    pub local: Mat4,
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
            id: 0,
            position: point,
            rotation: rot,
            size: Vec3::new(size, size, depth),
            color: spec.color,
            texture: spec.texture,
            material: spec.material,
            owner: None,
            age: 0.0,
            lifetime: spec.lifetime.map(|l| l.max(0.0)),
            fade: spec.fade.max(0.0),
            retiring: false,
        }
    }

    /// Object→world matrix mapping the unit cube `[-0.5, 0.5]³` to this box.
    pub fn model_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.size, self.rotation, self.position)
    }

    /// How much of the decal is left, `1` whole to `0` gone: the fade-out over the
    /// last `fade` seconds of its lifetime.
    pub fn opacity(&self) -> f32 {
        match self.lifetime {
            Some(end) if self.fade > 0.0 => ((end - self.age) / self.fade).clamp(0.0, 1.0),
            Some(end) if self.age >= end => 0.0,
            _ => 1.0,
        }
    }

    /// Whether its lifetime has run out.
    pub fn expired(&self) -> bool {
        self.lifetime.is_some_and(|end| self.age >= end)
    }

    /// Start fading out now, gone in `seconds`, from wherever its opacity is (so a
    /// decal already half faded keeps fading from half, never pops back to whole).
    /// A decal already due to end sooner keeps its own schedule.
    pub fn retire(&mut self, seconds: f32) {
        self.retiring = true;
        let seconds = seconds.max(0.0);
        let end = self.age + seconds;
        if self.lifetime.is_some_and(|own| own <= end) {
            return;
        }
        self.fade = seconds / self.opacity().max(1.0e-3);
        self.lifetime = Some(end);
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

#[cfg(test)]
#[path = "decal_tests.rs"]
mod tests;
