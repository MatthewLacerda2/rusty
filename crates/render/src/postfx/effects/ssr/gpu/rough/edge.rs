use glam::Vec3;

use super::super::silhouette::{at, base, shot, BRONZE, H, W};
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_core::color::f16_to_f32;
use rusty_scene::{Camera, Scene};

fn hardest_edge(on: &[u8], off: &[u8]) -> i32 {
    let red = |f: &[u8], y: u32| -> i32 {
        (W / 2 - 12..W / 2 + 12)
            .map(|x| at(f, x, y))
            .map(|c| c[0] - c[2])
            .sum::<i32>()
            / 24
    };
    let foot = (0..H)
        .filter(|&y| red(off, y) > 60)
        .max()
        .expect("the cube is in view");
    let steps: Vec<i32> = (foot + 2..H - 1)
        .map(|y| (red(on, y + 1) - red(on, y)).abs())
        .collect();
    steps
        .windows(3)
        .map(|w| w[1] - w[0].max(w[2]))
        .max()
        .unwrap()
}

#[test]
fn gpu_a_rough_metal_floor_fades_the_cubes_reflection_without_a_hard_edge() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let down = Camera::new(Vec3::new(0.0, 1.0, 0.0), -90.0, -12.0);
    let floor = |on| base(on, (BRONZE, 1.0, 0.35));
    let (on, off) = (
        shot(&mut r, &floor(true), &down),
        shot(&mut r, &floor(false), &down),
    );
    let (lit, plain) = (hardest_edge(&on, &off), hardest_edge(&off, &off));
    eprintln!("[ssr] rough metal: hardest edge under the cube {lit}, SSR off {plain}");
    assert!(lit <= plain + 3, "the reflection fades out, with no edge");
}

pub(super) fn side_step(on: &[u8], off: &[u8]) -> (f32, i32) {
    let red = |f: &[u8], x: u32, y: u32| at(f, x, y)[0] - at(f, x, y)[2];
    let added = |y: u32, x: u32| red(on, x, y) - red(off, x, y);
    let row = (H / 2..H).max_by_key(|&y| added(y, W / 2)).unwrap();
    let peak = added(row, W / 2).max(1);
    let step = (1..=W / 2)
        .map(|x| added(row, x) - added(row, x - 1))
        .max()
        .unwrap();
    (step as f32 / peak as f32, peak)
}

pub(super) fn lifted(on: bool, roughness: f32) -> rusty_scene::Scene {
    let mut s = base(on, (BRONZE, 1.0, roughness));
    let cube = s.find_entity_by_name("Cube").unwrap();
    s.world.transform_mut(cube).unwrap().position.y += 0.6;
    s
}

fn hdr_red(r: &mut Renderer, scene: &Scene, cam: &Camera) -> Vec<f32> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, cam, &out, false);
    let texture = &view.post_fx.scene_hdr.texture;
    let bytes = readback::read_texture_bytes(&r.device, &r.queue, texture, (W, H), 8);
    (0..(W * H) as usize)
        .map(|t| f16_to_f32(u16::from_le_bytes([bytes[t * 8], bytes[t * 8 + 1]])))
        .collect()
}

fn hdr_side_step(on: &[f32], off: &[f32]) -> (f32, f32) {
    let added = |x: u32, y: u32| (on[(y * W + x) as usize] - off[(y * W + x) as usize]).abs();
    let row = (H / 2..H)
        .max_by(|&a, &b| added(W / 2, a).total_cmp(&added(W / 2, b)))
        .unwrap();
    let step = (1..=W / 2)
        .map(|x| added(x, row) - added(x - 1, row))
        .fold(0.0, f32::max);
    let peak = added(W / 2, row);
    (step / peak.max(1e-6), peak)
}

const SATIN_RED: f32 = 0.1;

#[test]
fn gpu_a_satin_metal_floor_reflects_a_lifted_cube_with_soft_sides() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let down = Camera::new(Vec3::new(0.0, 1.0, 0.0), -90.0, -12.0);
    let mut step = |roughness| {
        let on = hdr_red(&mut r, &lifted(true, roughness), &down);
        hdr_side_step(&on, &hdr_red(&mut r, &lifted(false, roughness), &down))
    };
    let ((mirror, bright), (satin, red)) = (step(0.04), step(0.35));
    let share = red / bright;
    eprintln!("[ssr] the lifted cube's reflected side: at most {mirror:.2} of its redness in one pixel on a mirror, {satin:.2} at roughness 0.35, which adds {share:.2} of the mirror's red at its middle");
    assert!(mirror >= 0.5, "a mirror reflects a sharp side");
    assert!(share >= SATIN_RED, "a satin metal reflects the cube");
    assert!(
        satin <= 0.3,
        "a satin metal's reflection has no hard outline (#2028)"
    );
}
