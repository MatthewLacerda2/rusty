//! The frame's shadow inputs. Which light steers the realtime shadow sun (#809, #887):
//! only a directional light the frame draws live; a Baked directional light, or a
//! live light of another type, leaves the default sun in place. And whose volume sets
//! the cascades (#1434): the screen's main camera's, never a render-texture camera's.

use glam::{Quat, Vec3};

use super::sun_direction;
use crate::components::{CameraComponent, LightMode, RenderTarget};
use crate::render::{RenderView, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, default_post_processing, Primitive};
use crate::scene::{Camera, Scene};

fn default_sun() -> Vec3 {
    Vec3::new(-0.5, -1.0, -0.3).normalize()
}

/// Spawn a light primitive turned by `rotation`; return its id.
fn rotated_light(scene: &mut Scene, primitive: Primitive, rotation: Quat) -> u32 {
    let id = create_entity(scene, "Light", Some(primitive));
    scene.world.transform_mut(id).unwrap().rotation = rotation;
    id
}

#[test]
fn the_shadow_sun_follows_only_a_live_directional_light() {
    let mut scene = Scene::new();
    let sun_turn = Quat::from_rotation_x(-0.7) * Quat::from_rotation_y(0.4);
    let sun = rotated_light(&mut scene, Primitive::DirectionalLight, sun_turn);
    scene.world.light_mut(sun).unwrap().mode = LightMode::Baked;
    let lamp_turn = Quat::from_rotation_z(1.1);
    rotated_light(&mut scene, Primitive::PointLight, lamp_turn);

    let dir = sun_direction(&scene, false);
    assert!(
        dir.abs_diff_eq(default_sun(), 1e-6),
        "a Baked sun and a live point light keep the default sun, got {dir}"
    );

    scene.world.light_mut(sun).unwrap().mode = LightMode::Realtime;
    let live = (sun_turn * Vec3::NEG_Z).normalize();
    let dir = sun_direction(&scene, false);
    assert!(
        dir.abs_diff_eq(live, 1e-6),
        "a Realtime sun steers the shadows: want {live}, got {dir}"
    );
}

/// A camera entity whose volume asks for `cascades` shadow cascades.
fn camera_with_cascades(scene: &mut Scene, cascades: u32, target: Option<RenderTarget>) {
    let id = create_entity(scene, "Camera", None);
    let mut vc = default_post_processing();
    vc.shadows.cascades = cascades;
    scene.world.set_post_processing(id, Some(vc));
    let camera = CameraComponent {
        target_texture: target,
        ..CameraComponent::default()
    };
    scene.world.set_camera(id, Some(camera));
}

#[test]
fn gpu_the_cascades_follow_the_main_cameras_volume() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 32, 32, 2);
    let mut scene = Scene::new();
    create_entity(&mut scene, "Crate", Some(Primitive::Box));
    // Created first: a monitor camera that must not set the screen's shadows.
    camera_with_cascades(&mut scene, 1, Some(RenderTarget::new("monitor", 16, 16)));
    camera_with_cascades(&mut scene, 3, None);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 1.0, 4.0), -90.0, -10.0);
    r.render(&mut view, &scene, &cam, &out, false);
    assert_eq!(r.shadow_renderer.cascades.len(), 3);
}
