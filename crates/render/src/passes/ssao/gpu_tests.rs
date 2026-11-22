use glam::Vec3;

use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::PostProcessingComponent;
use rusty_core::settings::quality::{EffectQuality, QualityPresets};
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, ClearFlags, Scene};

fn crate_scene(on: bool) -> Scene {
    crate_scene_with(|vc| vc.ssao.active = on)
}

fn crate_scene_with(edit: impl FnOnce(&mut PostProcessingComponent)) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Crate", Some(Primitive::Box));
    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_post_processing();
    edit(&mut vc);
    scene.world.set_post_processing(volume, Some(vc));
    scene
        .world
        .set_camera(volume, Some(rusty_components::CameraComponent::default()));
    scene
}

fn cost(r: &mut Renderer, view: &mut RenderView, scene: &Scene, preset: &str) -> (u64, u32) {
    r.set_quality(QualityPresets::default().get(preset).unwrap().values);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 1.0, 4.0), -90.0, -10.0);
    r.render(view, scene, &cam, &out, false);
    (r.frame_counters.ssao_samples, r.frame_counters.draw_calls)
}

#[test]
fn gpu_each_quality_traces_what_it_promises() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(48, 32) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 48, 32);
    let (on, off) = (crate_scene(true), crate_scene(false));

    let traced = |r: &mut Renderer, view: &mut RenderView, preset| cost(r, view, &on, preset).0;
    assert_eq!(traced(&mut r, &mut view, "Medium"), 24 * 16 * 8);
    assert_eq!(traced(&mut r, &mut view, "High"), 48 * 32 * 16);
    assert_eq!(traced(&mut r, &mut view, "Low"), 24 * 16 * 6);
    let custom = crate_scene_with(|vc| {
        vc.ssao.quality = EffectQuality::Custom;
        (vc.ssao.samples, vc.ssao.resolution_divisor) = (32, 1);
    });
    assert_eq!(cost(&mut r, &mut view, &custom, "Low").0, 48 * 32 * 32);

    let (lit, lit_draws) = cost(&mut r, &mut view, &on, "High");
    let (unlit, unlit_draws) = cost(&mut r, &mut view, &off, "High");
    assert!(lit > 0);
    assert_eq!(unlit, 0);
    assert!(
        lit_draws > unlit_draws,
        "the prepass is counted: {lit_draws} vs {unlit_draws}"
    );
}

fn stacked(clear: ClearFlags) -> Scene {
    let mut scene = crate_scene(true);
    let camera = scene.add_entity("Stacked".to_string());
    let lens = rusty_components::CameraComponent {
        render_order: 1,
        clear_flags: clear,
        ..Default::default()
    };
    scene.world.set_camera(camera, Some(lens));
    scene
}

#[test]
fn gpu_a_depth_only_overlay_runs_no_ssao_of_its_own() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(48, 32) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 48, 32);
    let high = "High";
    let one = cost(&mut r, &mut view, &crate_scene(true), high).0;
    let overlay = cost(&mut r, &mut view, &stacked(ClearFlags::DepthOnly), high).0;
    assert_eq!(overlay, one, "the overlay traced its own AO");
    let opaque = cost(&mut r, &mut view, &stacked(ClearFlags::Skybox), high).0;
    assert_eq!(opaque, 2 * one, "the second camera of the stack ran");
}
