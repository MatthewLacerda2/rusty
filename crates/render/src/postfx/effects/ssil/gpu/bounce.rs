use crate::test_gpu::ssil::{energy, on, patch, renderer, scene, settled, FLOOR, LAMP};
use rusty_components::Ssil;

const TINT: f32 = 1.15;

const INDIRECT_GAIN: f32 = 0.005;

#[test]
fn gpu_a_point_light_on_a_red_wall_tints_the_white_floor() {
    let Some(mut r) = renderer() else { return };
    let red_over_green = |r: &mut _, ssil| {
        let frame = settled(r, &scene(ssil, LAMP).0, 4);
        let [red, green, _] = patch(&frame, FLOOR);
        red / green
    };
    let off = red_over_green(&mut r, Ssil::default());
    let lit = red_over_green(&mut r, on(1.0));
    eprintln!("[ssil] the floor's red/green: {off} off, {lit} on");
    assert!(lit >= off * TINT, "the bounce tints the floor red");
    assert!(r.frame_counters.ssil_samples > 0, "counted");
}

#[test]
fn gpu_a_scene_lit_only_by_indirect_light_gains_nothing() {
    let Some(mut r) = renderer() else { return };
    let lit = |r: &mut _, ssil| energy(&settled(r, &scene(ssil, 0.0).0, 4));
    let (off, with) = (lit(&mut r, Ssil::default()), lit(&mut r, on(4.0)));
    eprintln!("[ssil] indirect-only energy: {off} off, {with} on");
    assert!(with <= off * (1.0 + INDIRECT_GAIN), "{with} vs {off}");
}

#[test]
fn gpu_off_holds_nothing_and_draws_as_never_on() {
    let Some(mut r) = renderer() else { return };
    let (lit, _) = scene(on(1.0), LAMP);
    let (dark, _) = scene(Ssil::default(), LAMP);
    let mut v = crate::test_gpu::ssil::view(&r);
    let cam = crate::test_gpu::ssil::eye(0.0);
    let draw = crate::test_gpu::ssil::draw;
    let before = draw(&mut r, &mut v, &dark, &cam);
    assert_eq!(r.frame_counters.ssil_samples, 0);
    for _ in 0..3 {
        draw(&mut r, &mut v, &lit, &cam);
    }
    assert!(v.post_fx.holds_ssil());
    let after = draw(&mut r, &mut v, &dark, &cam);
    assert!(!v.post_fx.holds_ssil(), "off frees its targets");
    assert!(before == after, "off draws byte for byte as before it ran");
}

#[test]
fn gpu_the_first_frame_with_ssil_adds_nothing() {
    let Some(mut r) = renderer() else { return };
    let first = settled(&mut r, &scene(on(1.0), LAMP).0, 1);
    let off = settled(&mut r, &scene(Ssil::default(), LAMP).0, 1);
    assert!(first == off, "no smear before a history");
}

#[test]
fn gpu_a_metal_floor_takes_no_bounce() {
    let Some(mut r) = renderer() else { return };
    // A metre out from the wall, where the patch holds floor alone.
    let at = glam::Vec3::new(1.5, 0.0, -5.0);
    let red = |r: &mut _, ssil, metallic| {
        let (mut scene, _) = scene(ssil, LAMP);
        scene.materials.get_mut("floor").unwrap().metallic = metallic;
        patch(&settled(r, &scene, 4), at)[0]
    };
    let rough = (red(&mut r, Ssil::default(), 0.0), red(&mut r, on(4.0), 0.0));
    let metal = (red(&mut r, Ssil::default(), 1.0), red(&mut r, on(4.0), 1.0));
    eprintln!("[ssil] the floor's red off/on: rough {rough:?}, metal {metal:?}");
    assert!(
        rough.1 >= rough.0 * TINT,
        "a rough floor there takes the bounce"
    );
    assert!(
        metal.1 <= metal.0 * (1.0 + INDIRECT_GAIN),
        "a metal takes none"
    );
}

#[test]
fn gpu_the_tint_holds_under_taas_jitter() {
    let Some(mut r) = renderer() else { return };
    // TAA jitters each frame: SSIL reprojects through the jittered cameras.
    let red_over_green = |r: &mut _, ssil| {
        let (mut scene, _) = scene(ssil, LAMP);
        for id in scene.world.ids_with_camera() {
            scene.world.camera_mut(id).unwrap().taa_active = true;
        }
        let [red, green, _] = patch(&settled(r, &scene, 8), FLOOR);
        red / green
    };
    let off = red_over_green(&mut r, Ssil::default());
    let lit = red_over_green(&mut r, on(1.0));
    eprintln!("[ssil] the floor's red/green under TAA: {off} off, {lit} on");
    assert!(lit >= off * TINT, "the bounce tints the floor red");
}
