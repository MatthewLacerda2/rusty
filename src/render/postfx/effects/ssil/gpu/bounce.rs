//! What SSIL adds (#1750): a dynamic light's bounce off a red wall tints the white
//! floor and no metal one; a scene lit only by indirect light gains nothing; off
//! draws as before.

use super::fixture::{energy, on, patch, renderer, scene, settled, FLOOR, LAMP};
use crate::components::Ssil;

/// How far the floor's red over its green must rise with SSIL on: the issue's "a
/// dynamic point light on a red wall visibly tints the white floor".
const TINT: f32 = 1.15;

/// How much brighter a scene lit only by indirect light may get with SSIL on, as a
/// share of its light: the documented bound (`docs/api/Graphics.md`).
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
    // The ambient (the lightmap's and the probes' term, `indirect` in the forward
    // shader) is all that lights it: no light feeds the gather.
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
    let mut v = super::fixture::view(&r);
    let cam = super::fixture::eye(0.0);
    let draw = super::fixture::draw;
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
    // No history yet: the gather reads nothing, so the frame is the off one.
    let first = settled(&mut r, &scene(on(1.0), LAMP).0, 1);
    let off = settled(&mut r, &scene(Ssil::default(), LAMP).0, 1);
    assert!(first == off, "no smear before a history");
}

#[test]
fn gpu_a_metal_floor_takes_no_bounce() {
    let Some(mut r) = renderer() else { return };
    // The bounce goes through the diffuse response, `(1 − metallic)`. Read a metre
    // out from the wall, where the patch holds floor alone (at `FLOOR` it touches
    // the wall's foot, which a metal floor's own lamp light lights).
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
