use glam::Vec3;

use super::analytic::analytic;
use super::{chain, W};
use crate::{RenderView, OFFSCREEN_FORMAT};
use rusty_components::{PostProcessingComponent, Tonemap, Wheel};
use rusty_core::color::linear_to_srgb;
use rusty_scene::create::default_post_processing;
use rusty_scene::{Camera, Scene};

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
    for tonemap in Tonemap::ALL {
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
            assert!(worst <= 2.0, "{tonemap:?} at {ev} EV is {worst} levels off");
        }
    }
    let mut row = vec![[1.0f32, 0.0, 0.0]; W as usize];
    row[1] = [4.0, 0.0, 0.0];
    let out = c.run(&row, 0.0, None);
    assert_eq!((out[0], out[1]), ([255, 0, 0, 255], [255, 0, 0, 255]));
}

#[test]
fn gpu_white_balance_lift_and_gain_move_the_image_as_named() {
    let Some(mut c) = chain() else { return };
    let grey = |out: &[[u8; 4]]| Vec3::new(out[1][0].into(), out[1][1].into(), out[1][2].into());
    let mut row = vec![[0.6f32; 3]; W as usize];
    (row[0], row[1]) = ([0.0; 3], [0.18; 3]);
    let mut vc = volume(Tonemap::None);
    let neutral = c.run(&row, 0.0, Some(&vc));

    (vc.white_balance.temperature, vc.white_balance.tint) = (50.0, 0.0);
    let warm = grey(&c.run(&row, 0.0, Some(&vc)));
    assert!(warm.x > warm.z + 5.0, "+50 warms the grey card: {warm}");

    let mut vc = volume(Tonemap::None);
    vc.lift_gamma_gain.wheel_mut(Wheel::Lift).offset = 0.1;
    let lifted = c.run(&row, 0.0, Some(&vc));
    assert!(
        lifted[0][1] > neutral[0][1] + 40,
        "lift raises the blacks: {lifted:?}"
    );

    let mut vc = volume(Tonemap::None);
    vc.lift_gamma_gain.wheel_mut(Wheel::Gain).offset = -0.5;
    let gained = c.run(&row, 0.0, Some(&vc));
    let (bright, dark) = (f32::from(neutral[9][1]), f32::from(gained[9][1]));
    assert!(
        (dark - 255.0 * linear_to_srgb(0.3)).abs() <= 2.0,
        "gain scales the whites: {bright} → {dark}"
    );
    assert_eq!((neutral[0][1], gained[0][1]), (0, 0), "black stays black");

    let mut vc = volume(Tonemap::None);
    row[1] = [0.5, 0.05, 0.05];
    vc.color_adjustments.hue_shift = 120.0;
    let shifted = grey(&c.run(&row, 0.0, Some(&vc)));
    assert!(
        shifted.y > shifted.x,
        "a third of a turn takes red to green: {shifted}"
    );
}

#[test]
fn gpu_a_still_grade_is_baked_once_and_a_changed_knob_bakes_again() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 16, 16);
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = scene.add_entity("Camera".to_string());
    scene.world.set_camera(id, Some(Default::default()));
    scene
        .world
        .set_post_processing(id, Some(default_post_processing()));
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    let out = view.color_target_view().unwrap();
    let mut frame = |r: &mut crate::Renderer, scene: &Scene| {
        r.render(&mut view, scene, &cam, &out, false);
        r.frame_counters.grade_lut_bakes
    };
    assert_eq!(frame(&mut r, &scene), 1, "a new view bakes its table");
    assert_eq!(frame(&mut r, &scene), 0, "a still grade does not");
    scene
        .world
        .post_processing_mut(id)
        .unwrap()
        .color_adjustments
        .hue_shift = 30.0;
    assert_eq!(frame(&mut r, &scene), 1, "a changed knob bakes it again");
    assert_eq!(frame(&mut r, &scene), 0);
}
