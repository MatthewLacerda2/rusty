//! A distant texture samples a mip, not the full-size image (#1322): a one-texel
//! black and white checker seen from far enough that each pixel spans many texels
//! renders as an even grey, where the base level alone aliases to black and white.

use glam::Vec3;

use super::freshness_tests::{emissive_scene, temp_png};
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, Scene};

const SIDE: u32 = 16;

/// One frame's RGB rows of `scene` seen from 12 units away.
fn frame(renderer: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, SIDE, SIDE, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 12.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    let texture = view.color_target().unwrap();
    readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, SIDE, SIDE)
}

#[test]
fn gpu_a_distant_checker_renders_as_its_average() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(SIDE, SIDE) else {
        return;
    };
    let path = temp_png("mip_checker");
    image::RgbaImage::from_fn(256, 256, |x, y| match (x + y) % 2 {
        0 => image::Rgba([0, 0, 0, 255]),
        _ => image::Rgba([255, 255, 255, 255]),
    })
    .save(&path)
    .unwrap();
    let scene = emissive_scene(&path);
    let px = frame(&mut renderer, &scene);
    // The middle 4×4 pixels all land on the box's face.
    for y in 6..10 {
        for x in 6..10 {
            let i = ((y * SIDE + x) * 4) as usize;
            for c in &px[i..i + 3] {
                assert!(
                    (60..=230).contains(c),
                    "pixel ({x},{y}) is {:?}: the base level aliased, no mip was sampled",
                    &px[i..i + 3]
                );
            }
        }
    }
}
