//! Glossy SSR (#1736): a satin floor (roughness 0.4) shows a soft reflection of the
//! cube on it, which goes where the cube goes; a near-mirror (0.05) stays sharp; a
//! floor rougher than 0.7 is left alone; and the pyramid follows a resize.

use super::fixture::{draw, eye, frame, pixel, renderer, rgb, scene, Surface};
use super::fixture::{RED, REFLECTING, W};
use crate::render::postfx::effects::ssr::pyramid::levels;
use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::Scene;

/// A white metal floor of `roughness`.
fn metal(roughness: f32) -> Surface {
    ([0.9; 3], 1.0, roughness)
}

/// How much redder than blue a pixel is.
fn redness(c: [i32; 3]) -> i32 {
    c[0] - c[2]
}

/// The red cube on a floor of `roughness`, SSR `on`, the cube `x` m to the side.
fn shifted(on: bool, roughness: f32, x: f32) -> Scene {
    let (mut s, _) = scene(on, metal(roughness), RED);
    let cube = s.world.find_by_name("Cube").unwrap();
    s.world.transform_mut(cube).unwrap().position.x = x;
    s
}

#[test]
fn gpu_a_satin_floor_reflects_the_cube_softly_and_follows_it() {
    let Some(mut r) = renderer() else { return };
    let at = pixel(REFLECTING);
    let red = |r: &mut Renderer, on, x| redness(rgb(&frame(r, &shifted(on, 0.4, x)), at));
    let (on, off) = (red(&mut r, true, 0.0), red(&mut r, false, 0.0));
    let moved = red(&mut r, true, 3.0);
    eprintln!("[ssr] satin floor: on {on}, off {off}, cube moved aside {moved}");
    assert!(on >= off + 15, "the 0.4 floor reflects the red cube");
    assert!(on >= moved + 15, "the reflection goes where the cube goes");
}

/// The largest jump in redness between neighbouring pixels along the row of
/// [`REFLECTING`]: how sharp the reflected cube's sides are.
fn sharpest_edge(r: &mut Renderer, roughness: f32) -> i32 {
    let f = frame(r, &shifted(true, roughness, 0.0));
    let y = pixel(REFLECTING).1;
    (1..W)
        .map(|x| (redness(rgb(&f, (x, y))) - redness(rgb(&f, (x - 1, y)))).abs())
        .max()
        .unwrap()
}

#[test]
fn gpu_a_near_mirror_stays_sharp_where_a_satin_floor_blurs() {
    let Some(mut r) = renderer() else { return };
    let (sharp, soft) = (sharpest_edge(&mut r, 0.05), sharpest_edge(&mut r, 0.4));
    eprintln!("[ssr] sharpest reflected edge: 0.05 {sharp}, 0.4 {soft}");
    assert!(sharp >= 40, "the 0.05 floor reflects a hard edge");
    assert!(sharp >= soft * 2, "the 0.4 floor's is blurred");
}

#[test]
fn gpu_a_floor_rougher_than_the_minimum_smoothness_is_left_alone() {
    let Some(mut r) = renderer() else { return };
    let (on, off) = (
        frame(&mut r, &shifted(true, 0.75, 0.0)),
        frame(&mut r, &shifted(false, 0.75, 0.0)),
    );
    assert!(on == off, "roughness 0.75 gets no SSR at the default 0.3");
}

#[test]
fn gpu_the_pyramid_follows_a_resize_from_its_first_frame() {
    let Some(mut r) = renderer() else { return };
    let s = shifted(true, 0.4, 0.0);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, 72);
    draw(&mut r, &mut view, &s, &eye());
    assert_eq!(view.post_fx.ssr_pyramid(), Some(((W, 72), levels((W, 72)))));
    view.resize(&r.device, 40, 30);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, &s, &eye(), &out, false);
    assert_eq!(view.post_fx.ssr_pyramid(), Some(((40, 30), 6)));
}

#[test]
fn a_pyramid_reaches_one_texel() {
    assert_eq!(levels((1, 1)), 1);
    assert_eq!(levels((2, 1)), 2);
    assert_eq!(levels((1920, 1080)), 11);
    assert_eq!(levels((0, 0)), 1);
}
