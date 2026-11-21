//! The timer's 256 query pairs a frame ([`super::PAIRS`]) hold the deepest stack a
//! test can build: a base camera running every pass that times (the sun's
//! cascades, a full shadow atlas, Hi-Z, SSAO, SSIL, the fog, SSR, the
//! translucents, depth of field, TAA, motion blur, bloom, the lens flare and the
//! exposure meter) and an overlay camera, with the Hi-Z stage timed in parts
//! (#2401: a pair per part, not one for the stage). Counted on any adapter: the
//! timer counts the pairs it is asked for ([`super::GpuTimer::asked`]) even when
//! it has no timestamps.

use glam::{Quat, Vec3};

use crate::{RenderView, OFFSCREEN_FORMAT};
use rusty_components::{CameraComponent, DofMode, ExposureMode, LightType};
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, ClearFlags, Scene};

/// Shadowing point and spot lights: more than the atlas has room for.
const LIGHTS: usize = 48;

fn deepest_stack() -> Scene {
    let mut scene = Scene::new();
    let cam = scene.add_entity("Camera".to_string());
    let lens = CameraComponent {
        motion_blur_active: true,
        taa_active: true,
        ..Default::default()
    };
    scene.world.set_camera(cam, Some(lens));
    let mut volume = default_post_processing();
    for active in [
        &mut volume.ssr.active,
        &mut volume.ssao.active,
        &mut volume.ssil.active,
        &mut volume.volumetric_fog.active,
        &mut volume.bloom.active,
        &mut volume.lens_flare.active,
    ] {
        *active = true;
    }
    volume.depth_of_field.mode = DofMode::Bokeh;
    volume.exposure.mode = ExposureMode::Automatic;
    scene.world.set_post_processing(cam, Some(volume));
    let overlay = scene.add_entity("Overlay".to_string());
    let lens = CameraComponent {
        render_order: 1,
        clear_flags: ClearFlags::DepthOnly,
        ..Default::default()
    };
    scene.world.set_camera(overlay, Some(lens));
    let sun =
        rusty_scene::create::create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    let floor = create_entity(&mut scene, "Floor", Some(Primitive::Plane));
    scene.world.transform_mut(floor).unwrap().scale = Vec3::splat(20.0);
    for n in 0..LIGHTS {
        let kind = [Primitive::PointLight, Primitive::SpotLight][n % 2];
        let id = create_entity(&mut scene, &format!("Light_{n}"), Some(kind));
        let x = (n % 8) as f32 - 3.5;
        let z = -((n / 8) as f32) - 2.0;
        scene.world.transform_mut(id).unwrap().position = Vec3::new(x, 1.5, z);
        let mut light = scene.world.light_mut(id).unwrap();
        assert!(matches!(
            light.light_type,
            LightType::Point | LightType::Spotlight
        ));
        light.cast_shadows = true;
    }
    let glass = create_entity(&mut scene, "Glass", Some(Primitive::Box));
    scene.world.transform_mut(glass).unwrap().position = Vec3::new(0.0, 0.5, -3.0);
    scene
}

#[test]
fn gpu_the_deepest_stack_asks_for_fewer_pairs_than_a_frame_holds() {
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(96, 72) else {
        return;
    };
    r.gpu_timer.split_override.set(Some(true));
    let scene = deepest_stack();
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 96, 72);
    let out = view.color_target_view().unwrap();
    let eye = Camera::new(Vec3::new(0.0, 2.0, 2.0), -90.0, -20.0);
    let mut most = 0;
    for _ in 0..3 {
        let before = r.gpu_timer.asked.get();
        r.render(&mut view, &scene, &eye, &out, false);
        most = most.max(r.gpu_timer.asked.get() - before);
    }
    eprintln!("[timing] the deepest stack asked for {most} pairs a frame");
    assert!(most > 20, "the stack ran its passes: {most}");
    assert!(r.frame_counters.hiz_tested > 0, "the Hi-Z stage ran");
    assert!(
        most < super::PAIRS,
        "{most} pairs: past the {} a frame holds",
        super::PAIRS
    );
}
