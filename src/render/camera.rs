use glam::{Mat4, Vec3};

use crate::scene::{layer_in_mask, ClearFlags, Scene};

// Camera representation
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
            clear_flags: ClearFlags::Skybox,
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
/// camera component (by `render_order`) is authoritative, falling back to defaults
/// when none exists. This drives the API/particle camera and edit-mode rendering;
/// the multi-camera stack itself is built by [`build_camera_stack`].
pub fn sync_lens_from_scene(camera: &mut Camera, scene: &Scene, is_playing: bool) {
    // `Scene::iter` yields `Ref` guards, so copy the lens values out while the guard
    // is alive rather than returning a borrow.
    let mut lens = None;
    if is_playing {
        let mut best_order = i32::MAX;
        for entity in scene.iter() {
            if let Some(c) = &entity.camera {
                if entity.active && c.active && c.render_order <= best_order {
                    best_order = c.render_order;
                    lens = Some((c.fov, c.near, c.far, c.culling_mask));
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

/// Build a [`Camera`] positioned at the scene's active `CameraComponent` entity — the
/// player's-eye view the editor's **Game** tab shows while authoring (#183). Position
/// and orientation come from the camera entity's world transform (so the Game tab
/// tracks where the gameplay camera actually sits, not the free-fly cam), and the lens
/// from its component. Falls back to `base` (the free-fly camera) when no active camera
/// entity exists, so the tab still renders something sensible.
pub fn game_camera_from_scene(base: &Camera, scene: &Scene) -> Camera {
    let mut best_order = i32::MAX;
    let mut chosen: Option<Camera> = None;
    for id in scene.entity_ids() {
        let Some(entity) = scene.get_entity(id) else {
            continue;
        };
        let Some(c) = &entity.camera else { continue };
        if !entity.active || !c.active || c.render_order > best_order {
            continue;
        }
        best_order = c.render_order;
        let world = scene.compute_world_matrix(id);
        let (yaw, pitch) = yaw_pitch_from_matrix(world);
        let mut cam = base.clone();
        cam.position = world.col(3).truncate();
        cam.yaw = yaw;
        cam.pitch = pitch;
        cam.fov = c.fov;
        cam.near = c.near;
        cam.far = c.far;
        cam.culling_mask = c.culling_mask;
        cam.clear_flags = c.clear_flags;
        chosen = Some(cam);
    }
    chosen.unwrap_or_else(|| base.clone())
}

/// Yaw/pitch (degrees) of a world matrix's forward (-Z) axis, the inverse of
/// [`Camera::forward`], so a camera entity's orientation maps onto the render camera.
fn yaw_pitch_from_matrix(world: Mat4) -> (f32, f32) {
    // glTF/engine convention: an entity looks down its local -Z.
    let forward = (-world.col(2).truncate()).normalize_or_zero();
    let yaw = forward.z.atan2(forward.x).to_degrees();
    let pitch = forward.y.clamp(-1.0, 1.0).asin().to_degrees();
    (yaw, pitch)
}

/// Build the ordered camera stack the renderer composites for a frame (#93).
///
/// In play mode the scene's active [`CameraComponent`](crate::scene::CameraComponent)
/// entities drive rendering: each becomes a [`Camera`] sharing the viewport's
/// position/orientation (the `base` camera, driven by the play-mode follow logic)
/// but carrying its own lens, culling mask, and clear flags. The list is sorted by
/// `render_order` ascending, so later cameras composite over earlier ones — the
/// Unity world-cam + viewmodel-cam + UI-cam stack.
///
/// In edit mode (or when no active camera component exists) the free-fly `base`
/// camera is the single pass, preserving the editor viewport behavior.
pub fn build_camera_stack(base: &Camera, scene: &Scene, is_playing: bool) -> Vec<Camera> {
    if !is_playing {
        return vec![base.clone()];
    }

    // Collect (render_order, camera) for every active camera component, layering its
    // lens/mask/clear-flags onto the shared viewport position + orientation.
    let mut stack: Vec<(i32, Camera)> = Vec::new();
    for entity in scene.iter() {
        if !entity.active {
            continue;
        }
        if let Some(c) = &entity.camera {
            if !c.active {
                continue;
            }
            let mut cam = base.clone();
            cam.fov = c.fov;
            cam.near = c.near;
            cam.far = c.far;
            cam.culling_mask = c.culling_mask;
            cam.clear_flags = c.clear_flags;
            stack.push((c.render_order, cam));
        }
    }

    if stack.is_empty() {
        return vec![base.clone()];
    }

    // Stable sort keeps authoring order among cameras sharing a render_order.
    stack.sort_by_key(|(order, _)| *order);
    stack.into_iter().map(|(_, cam)| cam).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{CameraComponent, ClearFlags, Scene};

    fn cam_component(order: i32, mask: u32, flags: ClearFlags) -> CameraComponent {
        CameraComponent {
            active: true,
            fov: 60.0,
            near: 0.2,
            far: 500.0,
            culling_mask: mask,
            render_order: order,
            clear_flags: flags,
            motion_blur_active: false,
            motion_blur_samples: 0,
        }
    }

    fn add_camera(scene: &mut Scene, name: &str, comp: CameraComponent) {
        let id = scene.add_entity(name.to_string());
        scene.get_entity_mut(id).unwrap().camera = Some(comp);
    }

    #[test]
    fn edit_mode_is_a_single_base_pass() {
        let base = Camera::new(Vec3::ZERO, 0.0, 0.0);
        let mut scene = Scene::new();
        add_camera(
            &mut scene,
            "Cam",
            cam_component(0, u32::MAX, ClearFlags::Skybox),
        );
        // Edit mode ignores the scene cameras entirely.
        let stack = build_camera_stack(&base, &scene, false);
        assert_eq!(stack.len(), 1);
    }

    #[test]
    fn play_mode_sorts_by_render_order_and_layers_lens() {
        let base = Camera::new(Vec3::new(1.0, 2.0, 3.0), 10.0, 20.0);
        let mut scene = Scene::new();
        // Authored out of order; the viewmodel (order 1) must come after the world.
        add_camera(
            &mut scene,
            "View",
            cam_component(1, 0b10, ClearFlags::DepthOnly),
        );
        add_camera(
            &mut scene,
            "World",
            cam_component(0, 0b01, ClearFlags::Skybox),
        );

        let stack = build_camera_stack(&base, &scene, true);
        assert_eq!(stack.len(), 2);
        // Sorted ascending: world first, viewmodel composites on top.
        assert_eq!(stack[0].culling_mask, 0b01);
        assert_eq!(stack[0].clear_flags, ClearFlags::Skybox);
        assert_eq!(stack[1].culling_mask, 0b10);
        assert_eq!(stack[1].clear_flags, ClearFlags::DepthOnly);
        // Each pass shares the base viewport position/orientation but its own lens.
        assert_eq!(stack[1].position, base.position);
        assert_eq!(stack[1].fov, 60.0);
    }

    #[test]
    fn inactive_cameras_are_skipped_with_base_fallback() {
        let base = Camera::new(Vec3::ZERO, 0.0, 0.0);
        let mut scene = Scene::new();
        let mut off = cam_component(0, u32::MAX, ClearFlags::Skybox);
        off.active = false;
        add_camera(&mut scene, "Off", off);
        // No active scene camera -> fall back to the single base pass.
        let stack = build_camera_stack(&base, &scene, true);
        assert_eq!(stack.len(), 1);
    }
}
