use glam::{Mat4, Vec3};

use crate::scene::{layer_in_mask, Scene};

// Camera representation
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
}

/// Default vertical field of view, in degrees.
pub const DEFAULT_FOV: f32 = 45.0;
/// Default near/far clip planes — the free-fly editor camera's clip range, and the
/// fallback when no scene camera is authoritative.
pub const DEFAULT_NEAR: f32 = 0.1;
pub const DEFAULT_FAR: f32 = 200.0;

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
        let forward = self.forward();
        let view = Mat4::look_at_rh(self.position, self.position + forward, Vec3::Y);
        // Guard against a degenerate range (e.g. far <= near from hand-edited values),
        // which would otherwise produce a NaN projection.
        let far = self.far.max(self.near + 0.001);
        let proj = Mat4::perspective_rh(self.fov.to_radians(), aspect, self.near, far);
        proj * view
    }

    /// Whether an entity on `layer` is rendered by this camera's culling mask.
    pub fn renders_layer(&self, layer: u8) -> bool {
        layer_in_mask(layer, self.culling_mask)
    }
}

/// Reconcile the render camera's lens (fov/near/far) and culling mask with the
/// scene's active `CameraComponent` (#92). In the editor viewport the free-fly
/// camera owns its lens and renders every layer; in play mode the first active
/// camera component is authoritative, falling back to defaults when none exists.
pub fn sync_lens_from_scene(camera: &mut Camera, scene: &Scene, is_playing: bool) {
    // `Scene::iter` yields `Ref` guards, so copy the lens values out while the guard
    // is alive rather than returning a borrow.
    let mut lens = None;
    if is_playing {
        for entity in scene.iter() {
            if let Some(c) = &entity.camera {
                if entity.active && c.active {
                    lens = Some((c.fov, c.near, c.far, c.culling_mask));
                    break;
                }
            }
        }
    }

    match lens {
        Some((fov, near, far, mask)) => {
            camera.fov = fov;
            camera.near = near;
            camera.far = far;
            camera.culling_mask = mask;
        }
        None => {
            camera.near = DEFAULT_NEAR;
            camera.far = DEFAULT_FAR;
            camera.culling_mask = u32::MAX;
        }
    }
}
