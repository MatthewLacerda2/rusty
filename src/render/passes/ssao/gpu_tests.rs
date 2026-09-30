//! SSAO's cost on the GPU (#436): what each tier traces, as the frame stats report
//! it, and that switching AO off removes all of it. Skips when no adapter is present.

use glam::Vec3;

use crate::core::quality::QualityPreset;
use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, default_visual_correction, Primitive};
use crate::scene::{Camera, Scene};

/// A crate under a visual-correction volume with AO `on`.
fn crate_scene(on: bool) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Crate", Some(Primitive::Box));
    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_visual_correction();
    vc.ssao.active = on;
    scene.world.set_visual_correction(volume, Some(vc));
    scene
}

/// `(ssao_samples, draw_calls)` rendering `scene` at `quality`.
fn cost(r: &mut Renderer, view: &mut RenderView, scene: &Scene, q: QualityPreset) -> (u64, u32) {
    r.set_quality(q);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 1.0, 4.0), -90.0, -10.0);
    r.render(view, scene, &cam, &out, false);
    (r.frame_counters.ssao_samples, r.frame_counters.draw_calls)
}

#[test]
fn gpu_each_tier_traces_what_it_promises() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(48, 32) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 48, 32, 2);
    let (on, off) = (crate_scene(true), crate_scene(false));

    // Medium: 8 samples over a half-resolution target; High: 16 over full.
    assert_eq!(
        cost(&mut r, &mut view, &on, QualityPreset::Medium).0,
        24 * 16 * 8
    );
    assert_eq!(
        cost(&mut r, &mut view, &on, QualityPreset::High).0,
        48 * 32 * 16
    );
    assert_eq!(
        cost(&mut r, &mut view, &on, QualityPreset::Low).0,
        0,
        "off on Low"
    );

    // Off: no occlusion taps, and the prepass's draw is gone from the count too.
    let (lit, lit_draws) = cost(&mut r, &mut view, &on, QualityPreset::High);
    let (unlit, unlit_draws) = cost(&mut r, &mut view, &off, QualityPreset::High);
    assert!(lit > 0);
    assert_eq!(unlit, 0);
    assert!(
        lit_draws > unlit_draws,
        "the prepass is counted: {lit_draws} vs {unlit_draws}"
    );
}
