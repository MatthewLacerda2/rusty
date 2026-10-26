//! The grading LUT on lavapipe (#1440): neutral knobs reproduce the analytic
//! tonemap the composite applied before the LUT, each knob moves the image the way
//! its name says, and a still grade is baked once.

use glam::Vec3;

use super::{chain, W};
use crate::components::{PostProcessingComponent, Tonemap, Wheel};
use crate::core::color::linear_to_srgb;
use crate::render::{RenderView, OFFSCREEN_FORMAT};
use crate::scene::authoring::default_post_processing;
use crate::scene::authoring::rendering::post_processing as ops;
use crate::scene::{Camera, Scene};

/// The composite's grade before the LUT (`postfx.wgsl` at #1438): exposure, then
/// the tonemap, at neutral contrast, saturation and gamma.
fn analytic(x: f32, ev: f32, tonemap: Tonemap) -> f32 {
    let h = x * ev.exp2();
    let aces = (h * (2.51 * h + 0.03)) / (h * (2.43 * h + 0.59) + 0.14);
    match tonemap {
        Tonemap::None => h,
        Tonemap::Reinhard => h / (h + 1.0),
        Tonemap::Aces => aces,
    }
}

/// A grey ramp over the HDR range a frame holds: 0, then 0.001 to 40, log-spaced.
fn ramp() -> Vec<f32> {
    (0..W)
        .map(|i| match i {
            0 => 0.0,
            _ => 0.001 * 40_000f32.powf((i - 1) as f32 / (W - 2) as f32),
        })
        .collect()
}

fn volume(tonemap: Tonemap) -> PostProcessingComponent {
    let mut vc = default_post_processing();
    vc.tonemapping.mode = tonemap;
    vc
}

#[test]
fn gpu_neutral_knobs_through_the_lut_match_the_analytic_tonemap() {
    let Some(mut c) = chain() else { return };
    let ramp = ramp();
    let row: Vec<[f32; 3]> = ramp.iter().map(|&x| [x; 3]).collect();
    for tonemap in [Tonemap::Aces, Tonemap::Reinhard, Tonemap::None] {
        for ev in [0.0, 1.5] {
            let grade = volume(tonemap);
            let out = c.run(&row, ev, Some(&grade));
            let worst = ramp
                .iter()
                .zip(&out)
                .map(|(&x, px)| {
                    (255.0 * linear_to_srgb(analytic(x, ev, tonemap)) - f32::from(px[0])).abs()
                })
                .fold(0.0, f32::max);
            eprintln!("[grade] {tonemap:?} at {ev} EV: worst {worst:.2} levels");
            // Black and display white sit on texels: only the curve between rounds.
            assert!(worst <= 2.0, "{tonemap:?} at {ev} EV is {worst} levels off");
        }
    }
    // A view without a volume shows display white and black exactly.
    let mut row = vec![[1.0f32, 0.0, 0.0]; W as usize];
    row[1] = [4.0, 0.0, 0.0];
    let out = c.run(&row, 0.0, None);
    assert_eq!((out[0], out[1]), ([255, 0, 0, 255], [255, 0, 0, 255]));
}

#[test]
fn gpu_white_balance_lift_and_gain_move_the_image_as_named() {
    let Some(mut c) = chain() else { return };
    let grey = |out: &[[u8; 4]]| Vec3::new(out[1][0].into(), out[1][1].into(), out[1][2].into());
    // Pixel 0 black, pixel 1 a grey card, the rest a bright 0.6.
    let mut row = vec![[0.6f32; 3]; W as usize];
    (row[0], row[1]) = ([0.0; 3], [0.18; 3]);
    let mut vc = volume(Tonemap::None);
    let neutral = c.run(&row, 0.0, Some(&vc));

    ops::grading::white_balance::set(&mut vc, 50.0, 0.0);
    let warm = grey(&c.run(&row, 0.0, Some(&vc)));
    assert!(warm.x > warm.z + 5.0, "+50 warms the grey card: {warm}");

    let mut vc = volume(Tonemap::None);
    ops::grading::lift_gamma_gain::set_wheel(&mut vc, Wheel::Lift, [1.0; 3], 0.1);
    let lifted = c.run(&row, 0.0, Some(&vc));
    assert!(
        lifted[0][1] > neutral[0][1] + 40,
        "lift raises the blacks: {lifted:?}"
    );

    let mut vc = volume(Tonemap::None);
    ops::grading::lift_gamma_gain::set_wheel(&mut vc, Wheel::Gain, [1.0; 3], -0.5);
    let gained = c.run(&row, 0.0, Some(&vc));
    let (bright, dark) = (f32::from(neutral[9][1]), f32::from(gained[9][1]));
    // Half the gain halves the linear whites: 0.6 → 0.3.
    assert!(
        (dark - 255.0 * linear_to_srgb(0.3)).abs() <= 2.0,
        "gain scales the whites: {bright} → {dark}"
    );
    assert_eq!((neutral[0][1], gained[0][1]), (0, 0), "black stays black");

    let mut vc = volume(Tonemap::None);
    row[1] = [0.5, 0.05, 0.05];
    ops::grading::color_adjustments::set_hue_shift(&mut vc, 120.0);
    let shifted = grey(&c.run(&row, 0.0, Some(&vc)));
    assert!(
        shifted.y > shifted.x,
        "a third of a turn takes red to green: {shifted}"
    );
}

#[test]
fn gpu_a_still_grade_is_baked_once_and_a_changed_knob_bakes_again() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 16, 16, 2);
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = scene.add_entity("Camera".to_string());
    scene.world.set_camera(id, Some(Default::default()));
    scene
        .world
        .set_post_processing(id, Some(default_post_processing()));
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    let out = view.color_target_view().unwrap();
    let mut frame = |r: &mut crate::render::Renderer, scene: &Scene| {
        r.render(&mut view, scene, &cam, &out, false);
        r.frame_counters.grade_lut_bakes
    };
    assert_eq!(frame(&mut r, &scene), 1, "a new view bakes its table");
    assert_eq!(frame(&mut r, &scene), 0, "a still grade does not");
    ops::grading::color_adjustments::set_hue_shift(
        &mut scene.world.post_processing_mut(id).unwrap(),
        30.0,
    );
    assert_eq!(frame(&mut r, &scene), 1, "a changed knob bakes it again");
    assert_eq!(frame(&mut r, &scene), 0);
}
