//! The frame's shadow inputs. Which light steers the realtime shadow sun (#809, #887):
//! only a directional light the frame draws live; a Baked directional light, or a
//! live light of another type, leaves no sun, and then no cascade is fitted (#1713)
//! — except in a bake capture, which draws the Baked sun live. And whose volume sets
//! the cascades (#1434): the screen's main camera's, never a render-texture camera's.

use glam::{Quat, Vec3};

use super::{frame_cascades, live_sun};
use crate::{RenderView, OFFSCREEN_FORMAT};
use rusty_components::{CameraComponent, LightMode, RenderTarget, ShadowSettings};
use rusty_scene::authoring::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

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

    assert_eq!(
        live_sun(&scene, false),
        None,
        "a Baked sun and a live point light make no shadow sun"
    );

    scene.world.light_mut(sun).unwrap().mode = LightMode::Realtime;
    let live = (sun_turn * Vec3::NEG_Z).normalize();
    let dir = live_sun(&scene, false).expect("a live sun");
    assert!(
        dir.abs_diff_eq(live, 1e-6),
        "a Realtime sun steers the shadows: want {live}, got {dir}"
    );
}

/// The issue's case (#1713): a scene whose only sun is Baked fits no cascade in a
/// frame, so none is swept or sampled; a bake capture, which draws that sun live,
/// keeps them, and so does a Realtime sun.
#[test]
fn a_baked_only_sun_fits_no_cascade_outside_a_capture() {
    let mut scene = Scene::new();
    let sun = rotated_light(&mut scene, Primitive::DirectionalLight, Quat::IDENTITY);
    scene.world.light_mut(sun).unwrap().mode = LightMode::Baked;
    let cam = Camera::new(Vec3::new(0.0, 1.0, 4.0), -90.0, -10.0);
    let settings = ShadowSettings::default();
    let fit = |scene: &Scene, capture| frame_cascades(scene, capture, &cam, 1.5, &settings).len();
    let want = settings.cascades as usize;
    assert_eq!(fit(&scene, false), 0, "a Baked sun costs no cascade");
    assert_eq!(fit(&scene, true), want, "a bake capture shadows it");
    scene.world.set_active(sun, false);
    assert_eq!(fit(&scene, true), 0, "an inactive sun is no sun");
    scene.world.set_active(sun, true);
    scene.world.light_mut(sun).unwrap().mode = LightMode::Realtime;
    assert_eq!(
        fit(&scene, false),
        want,
        "a Realtime sun keeps its cascades"
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
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 32, 32);
    let mut scene = Scene::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    create_entity(&mut scene, "Crate", Some(Primitive::Box));
    // Created first: a monitor camera that must not set the screen's shadows.
    camera_with_cascades(&mut scene, 1, Some(RenderTarget::new("monitor", 16, 16)));
    camera_with_cascades(&mut scene, 3, None);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 1.0, 4.0), -90.0, -10.0);
    r.render(&mut view, &scene, &cam, &out, false);
    assert_eq!(r.shadow_renderer.cascades.len(), 3);
}

/// The atlas keys each light's tiles by the id `local_light_ids` gives it (#1711):
/// one id per light `local_lights` stages, in its order, live or in a capture.
#[test]
fn every_staged_light_has_its_entity_id_in_order() {
    use crate::clusters::local_lights;
    let mut scene = Scene::new();
    let lamps = [
        Primitive::PointLight,
        Primitive::DirectionalLight,
        Primitive::SpotLight,
        Primitive::PointLight,
        Primitive::SpotLight,
    ];
    let ids: Vec<u32> = (lamps.iter().enumerate())
        .map(|(i, &p)| {
            let id = rotated_light(&mut scene, p, Quat::IDENTITY);
            scene.world.transform_mut(id).unwrap().position = Vec3::X * i as f32;
            id
        })
        .collect();
    scene.world.light_mut(ids[3]).unwrap().mode = LightMode::Baked;
    scene.world.set_active(ids[4], false);
    for capture in [false, true] {
        let staged = local_lights(&scene, capture);
        let keyed = super::local_light_ids(&scene, capture);
        assert_eq!(staged.len(), keyed.len(), "capture {capture}");
        for ((light, _), id) in staged.iter().zip(&keyed) {
            let at = scene.world.world_position(*id);
            assert_eq!(Vec3::from(light.position), at, "capture {capture}");
        }
    }
}
