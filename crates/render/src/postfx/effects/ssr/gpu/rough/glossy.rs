use super::super::fixture::{draw, eye, frame, pixel, renderer, rgb, scene, tuned, Surface};
use super::super::fixture::{RED, REFLECTING, W};
use crate::postfx::effects::ssr::pyramid::{hit_levels, levels};
use crate::readback;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::Ssr;
use rusty_scene::Scene;

fn metal(roughness: f32) -> Surface {
    ([0.9; 3], 1.0, roughness)
}

fn redness(c: [i32; 3]) -> i32 {
    c[0] - c[2]
}

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

fn sharpest_edge(r: &mut Renderer, roughness: f32) -> i32 {
    let (w, h) = (1280, 720);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, w, h);
    let out = view.color_target_view().unwrap();
    r.render(
        &mut view,
        &shifted(true, roughness, 0.0),
        &eye(),
        &out,
        false,
    );
    let f = readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), w, h);
    let red = |x: u32| {
        let i = (((h - 4) * w + x) * 4) as usize;
        i32::from(f[i]) - i32::from(f[i + 2])
    };
    (1..w).map(|x| (red(x) - red(x - 1)).abs()).max().unwrap()
}

#[test]
fn gpu_a_near_mirror_stays_sharp_where_a_satin_floor_blurs() {
    let Some(mut r) = renderer() else { return };
    let (mirror, near) = (sharpest_edge(&mut r, 0.0), sharpest_edge(&mut r, 0.05));
    let soft = sharpest_edge(&mut r, 0.4);
    eprintln!("[ssr] sharpest reflected edge at 720p: 0 {mirror}, 0.05 {near}, 0.4 {soft}");
    assert!(near >= 60, "the 0.05 floor reflects a hard edge");
    assert!(near * 10 >= mirror * 9, "as hard as a mirror's");
    assert!(near >= soft * 2, "the 0.4 floor's is blurred");
}

#[test]
fn gpu_a_floor_rougher_than_the_minimum_smoothness_is_left_alone() {
    let Some(mut r) = renderer() else { return };
    let (on, off) = (
        frame(&mut r, &shifted(true, 0.75, 0.0)),
        frame(&mut r, &shifted(false, 0.75, 0.0)),
    );
    assert!(on == off, "roughness 0.75 gets no SSR at the fixture's 0.3");
}

#[test]
fn gpu_a_roughness_half_floor_takes_ssr_only_under_an_explicit_lower_minimum() {
    let Some(mut r) = renderer() else { return };
    let default = Ssr::default().min_smoothness;
    let (stock, _) = tuned(true, metal(0.5), RED, |v| v.ssr.min_smoothness = default);
    let (stock, lower, off) = (
        frame(&mut r, &stock),
        frame(&mut r, &shifted(true, 0.5, 0.0)),
        frame(&mut r, &shifted(false, 0.5, 0.0)),
    );
    assert!(stock == off, "the default leaves roughness 0.5 to its sky");
    let at = pixel(REFLECTING);
    let (lower, off) = (redness(rgb(&lower, at)), redness(rgb(&off, at)));
    eprintln!("[ssr] roughness-0.5 floor at a minimum of 0.3: {lower}, SSR off {off}");
    assert!(lower >= off + 6, "0.3 reflects the red cube on it");
}

#[test]
fn gpu_the_pyramid_follows_a_resize_from_its_first_frame() {
    let Some(mut r) = renderer() else { return };
    let s = shifted(true, 0.4, 0.0);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, 72);
    draw(&mut r, &mut view, &s, &eye());
    let fitted = |size| Some((size, 1, hit_levels(size)));
    assert_eq!(view.post_fx.ssr_pyramid(), fitted((W, 72)));
    view.resize(&r.device, 40, 30);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, &s, &eye(), &out, false);
    assert_eq!(view.post_fx.ssr_pyramid(), Some(((40, 30), 1, 3)));
}

#[test]
fn a_pyramid_reaches_one_texel() {
    assert_eq!(levels((1, 1)), 1);
    assert_eq!(levels((2, 1)), 2);
    assert_eq!(levels((1920, 1080)), 11);
    assert_eq!(levels((0, 0)), 1);
}
