//! src/scene/camera/ — the active view [`Camera`] and its scene reconciliation.
//!
//! The camera the sim drives (scripts move it, `Camera.*` reads it, the audio
//! listener follows it) is plain glam data, so it lives sim-side; the renderer
//! only reads it and builds its per-frame camera stack from it (#494).

use glam::camera::rh::proj::directx;
use glam::{Mat4, Vec3};

mod lens;
mod main;
mod projection;
mod view;

pub use lens::{held_fov, held_lens, Lens, MAX_LENS_EXTENT};
pub use main::{active_cameras, camera_volume, main_camera, render_order, screen_cameras};
pub use projection::ScreenPoint;
pub use view::{look_rotation, set_view_orientation, set_view_position, sync_pose_from_scene};

use crate::components::Projection;
use crate::scene::{layer_in_mask, ClearFlags, Scene};

/// The engine's active view camera — a World resource (Unity's `Camera.main`):
/// position + yaw/pitch plus the lens and culling state reconciled from the scene's
/// `CameraComponent`s. Gameplay scripts, the audio listener and the renderer all
/// read this one camera.
///
/// Its lens is always valid: every copy from a component goes through
/// [`held_lens`] (#1359), so a reader uses `fov` / `near` / `far` / `projection` as
/// they are and gets a finite projection.
#[derive(Clone)]
pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,   // Degrees (rotation around Y axis)
    pub pitch: f32, // Degrees (rotation around X axis)
    pub fov: f32,   // Vertical field of view, in degrees
    pub near: f32,  // Near clip plane distance
    pub far: f32,   // Far clip plane distance
    /// Layer membership bitmask: a mesh draws only when its layer's bit is set.
    /// Reconciled from the active `CameraComponent` each frame (see #92).
    pub culling_mask: u32,
    /// How this camera initializes the framebuffer before drawing. The viewport's
    /// base camera (free-fly editor cam / world cam) clears the skybox; stacked
    /// cameras may clear depth-only to composite on top (#93).
    pub clear_flags: ClearFlags,
    /// Perspective (through `fov`) or orthographic (#430).
    pub projection: Projection,
    /// Whether a scene `CameraComponent` set the lens on the last sync, so the
    /// fallback knows to hand the free camera its own fov back (#997).
    scene_lens: bool,
}

pub use crate::components::rendering::camera::DEFAULT_FOV;
use crate::components::rendering::camera::{DEFAULT_FAR, DEFAULT_NEAR};

/// Pitch limit, in degrees, for every pitch a person or script sets: short of
/// straight up/down, where the look-at basis degenerates and yaw stops meaning
/// anything.
pub const MAX_PITCH: f32 = 89.0;

impl Camera {
    pub fn new(position: Vec3, yaw: f32, pitch: f32) -> Self {
        Self {
            position,
            yaw,
            pitch,
            fov: DEFAULT_FOV,
            near: DEFAULT_NEAR,
            far: DEFAULT_FAR,
            culling_mask: u32::MAX,
            clear_flags: ClearFlags::Skybox,
            projection: Projection::Perspective,
            scene_lens: false,
        }
    }

    pub fn forward(&self) -> Vec3 {
        let pitch_rad = self.pitch.to_radians();
        let yaw_rad = self.yaw.to_radians();

        let cos_pitch = pitch_rad.cos();
        let sin_pitch = pitch_rad.sin();
        let cos_yaw = yaw_rad.cos();
        let sin_yaw = yaw_rad.sin();

        Vec3::new(cos_yaw * cos_pitch, sin_pitch, sin_yaw * cos_pitch).normalize()
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    pub fn build_view_projection(&self, aspect: f32) -> Mat4 {
        self.projection_matrix(aspect) * self.view_matrix()
    }

    /// The view → clip matrix for a viewport of `aspect` (wgpu depth `0..1`).
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        let (near, far) = (self.near, self.far);
        match self.projection {
            Projection::Perspective => {
                directx::perspective(self.fov.to_radians(), aspect, near, far)
            }
            Projection::Orthographic { size: h } => {
                directx::orthographic(-h * aspect, h * aspect, -h, h, near, far)
            }
        }
    }

    /// Whether an entity on `layer` is rendered by this camera's culling mask.
    pub fn renders_layer(&self, layer: u8) -> bool {
        layer_in_mask(layer, self.culling_mask)
    }
}

/// Reconcile the render camera's lens (fov/near/far) and culling mask with the
/// scene's active `CameraComponent` (#92). In the editor viewport the free-fly
/// camera owns its lens and renders every layer; in play mode the [`main_camera`]
/// (the screen stack's base) is authoritative, falling back to defaults
/// when none exists. This drives the API/particle camera and edit-mode rendering;
/// the multi-camera stack itself is built by the renderer
/// (`render::build_camera_stack`).
pub fn sync_lens_from_scene(camera: &mut Camera, scene: &Scene, is_playing: bool) {
    // `World::camera` yields a `CompRef` guard, so copy the lens values out while
    // the guard is alive rather than returning a borrow.
    let lens = is_playing
        .then(|| main_camera(scene))
        .flatten()
        .and_then(|id| scene.world.camera(id))
        .map(|c| (held_lens(&c), c.culling_mask));

    let had_scene_lens = std::mem::replace(&mut camera.scene_lens, lens.is_some());
    match lens {
        Some((lens, mask)) => {
            lens.apply(camera);
            camera.culling_mask = mask;
        }
        None => {
            // The fov goes back to the default only when a scene camera stops
            // driving it, so `Camera.SetFov` on the free camera survives the per-frame
            // sync; there is no API for the free camera's clip planes.
            if had_scene_lens {
                camera.fov = DEFAULT_FOV;
            }
            camera.projection = Projection::Perspective;
            camera.near = DEFAULT_NEAR;
            camera.far = DEFAULT_FAR;
            camera.culling_mask = u32::MAX;
        }
    }
}

/// The main view: a [`Camera`] at the scene's [`main_camera`] entity, the one answer
/// every path shares (#1619) — the player, the editor's **Game** tab (#183), the
/// headless harness's screen stack and the `Camera.*` getters. Position and
/// orientation come from the entity's world transform, the lens from its component.
/// Falls back to `base` (the free view camera) when no active screen camera entity
/// exists.
pub fn game_camera_from_scene(base: &Camera, scene: &Scene) -> Camera {
    main_camera(scene)
        .and_then(|id| camera_for_entity(base, scene, id))
        .unwrap_or_else(|| base.clone())
}

/// The render [`Camera`] a `CameraComponent` entity sees through: its world
/// transform's position and orientation plus the component's lens, mask, clear
/// flags and projection. `base` supplies the rest. `None` without a camera.
pub fn camera_for_entity(base: &Camera, scene: &Scene, id: u32) -> Option<Camera> {
    let c = scene.world.camera(id)?;
    let world = scene.world.local_to_world(id);
    let (yaw, pitch) = yaw_pitch_from_matrix(world);
    let mut cam = base.clone();
    cam.position = world.col(3).truncate();
    cam.yaw = yaw;
    cam.pitch = pitch;
    apply_lens(&mut cam, &c);
    Some(cam)
}

/// Layer `c`'s lens (fov/near/far/projection), culling mask and clear flags onto
/// `cam`, keeping its pose.
pub fn apply_lens(cam: &mut Camera, c: &crate::components::CameraComponent) {
    held_lens(c).apply(cam);
    cam.culling_mask = c.culling_mask;
    cam.clear_flags = c.clear_flags;
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod sync_tests;

#[cfg(test)]
#[path = "lens_tests.rs"]
mod lens_tests;

/// Yaw/pitch (degrees) of a world matrix's forward (-Z) axis, the inverse of
/// [`Camera::forward`], so a camera entity's orientation maps onto the render camera.
fn yaw_pitch_from_matrix(world: Mat4) -> (f32, f32) {
    // glTF/engine convention: an entity looks down its local -Z.
    let forward = (-world.col(2).truncate()).normalize_or_zero();
    let yaw = forward.z.atan2(forward.x).to_degrees();
    let pitch = forward.y.clamp(-1.0, 1.0).asin().to_degrees();
    (yaw, pitch)
}
