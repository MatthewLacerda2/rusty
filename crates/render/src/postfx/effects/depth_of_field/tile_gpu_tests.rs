use glam::Vec3;

use super::gpu_fixture::{bokeh, cube, eye, scene, FAR};
use super::DofParams;
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::DepthOfField;
use rusty_scene::camera::DEFAULT_FOV;
use rusty_scene::Scene;

const W: u32 = 960;
const H: u32 = 720;

const POST_DEPTH: f32 = 0.5;
const GAP: f32 = 30.0;

fn shot(r: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let out = v.color_target_view().unwrap();
    r.render(&mut v, scene, &eye(), &out, false);
    readback::read_texture_rgba8(&r.device, &r.queue, v.color_target().unwrap(), W, H)
}

fn half_width() -> f32 {
    (DEFAULT_FOV.to_radians() * 0.5).tan() * W as f32 / H as f32
}

fn column(p: Vec3) -> f32 {
    (p.x / (half_width() * -p.z) * 0.5 + 0.5) * W as f32
}

fn with_post(dof: DepthOfField, post: bool, edge: f32) -> Scene {
    let (mut scene, _) = scene(dof);
    if post {
        let width = 0.03;
        let right = ((edge - GAP) / W as f32 * 2.0 - 1.0) * half_width() * POST_DEPTH;
        let at = Vec3::new(right - width * 0.5, 0.0, -POST_DEPTH);
        let id = cube(&mut scene, at);
        scene.world.transform_mut(id).unwrap().scale = Vec3::new(width, 2.0, 0.02);
    }
    scene
}

fn spread(a: &[u8], b: &[u8], x: i32) -> u32 {
    let i = |x: i32, y: u32| ((y * W + x as u32) * 4) as usize;
    let diff = |x, y| {
        (0..3)
            .map(|c| a[i(x, y) + c].abs_diff(b[i(x, y) + c]))
            .max()
    };
    let rows = H / 2 - 20..H / 2 + 20;
    rows.flat_map(|y| (x - 10..=x + 10).map(move |x| diff(x, y).unwrap() as u32))
        .sum()
}

#[test]
fn gpu_a_background_beside_a_near_blur_keeps_its_own_blur() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    super::gpu_fixture::blur_at(&mut r, 1);
    let face = FAR + Vec3::new(-0.75, 0.0, 0.75);
    let edge = column(face);
    let dof = DepthOfField {
        aperture: 6.8,
        ..bokeh(3.0)
    };
    let p = DofParams::new(&dof, H, 1).unwrap();
    let (post_coc, far_coc) = (p.coc(POST_DEPTH), p.coc(-face.z));
    assert_eq!(
        post_coc, -p.shape[1],
        "the post clamps to the largest circle"
    );
    assert!((3.0..=8.0).contains(&far_coc), "the edge's CoC {far_coc}");
    let mut widths = [0u32; 2];
    for (post, width) in [false, true].into_iter().zip(&mut widths) {
        let sharp = shot(&mut r, &with_post(DepthOfField::default(), post, edge));
        let blurred = shot(&mut r, &with_post(dof, post, edge));
        *width = spread(&blurred, &sharp, edge as i32);
    }
    let [alone, beside] = widths;
    eprintln!("[dof] far CoC {far_coc:.2} px: edge change {alone} alone, {beside} by the post");
    assert!(alone >= 1000, "the edge blurs ({alone})");
    assert!(
        beside * 10 >= alone * 9,
        "the post's tiles leave the edge's blur alone ({alone} → {beside})"
    );
}
