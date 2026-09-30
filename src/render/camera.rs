//! Multi-camera compositing (#93): the ordered stack of [`Camera`] passes the
//! renderer draws each frame. The camera itself is sim-side (`scene::camera`).

use crate::scene::{Camera, Scene};

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
    for id in scene.world.ids_with_camera() {
        if !scene.world.is_active(id) {
            continue;
        }
        let c = scene
            .world
            .camera(id)
            .expect("id came from ids_with_camera");
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
    use crate::scene::{CameraComponent, ClearFlags};
    use glam::Vec3;

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
            fxaa_active: true,
        }
    }

    fn add_camera(scene: &mut Scene, name: &str, comp: CameraComponent) {
        let id = scene.add_entity(name.to_string());
        scene.world.set_camera(id, Some(comp));
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
