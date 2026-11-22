use glam::Vec3;

use super::freshness_tests::temp_png;
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_scene::{Camera, Scene};

const SIDE: u32 = 16;

#[test]
fn gpu_the_sky_across_the_panorama_seam_keeps_its_colour() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(SIDE, SIDE) else {
        return;
    };
    let path = temp_png("panorama_seam");
    image::RgbaImage::from_fn(256, 128, |x, _| match (48..208).contains(&x) {
        true => image::Rgba([0, 0, 255, 255]),
        false => image::Rgba([255, 0, 0, 255]),
    })
    .save(&path)
    .unwrap();
    let mut scene = Scene::new();
    scene.skybox_path = path;
    for yaw in [180.0, 181.5, 183.0, 184.5] {
        let px = sky(&mut renderer, &scene, yaw);
        for y in 4..12 {
            for x in 0..SIDE {
                let i = ((y * SIDE + x) * 4) as usize;
                let (r, b) = (px[i], px[i + 2]);
                let rgb = &px[i..i + 3];
                assert!(r > b, "yaw {yaw}: pixel ({x},{y}) is {rgb:?}, a coarse mip");
            }
        }
    }
}

fn sky(renderer: &mut Renderer, scene: &Scene, yaw: f32) -> Vec<u8> {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, SIDE, SIDE);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::ZERO, yaw, 0.0);
    for _ in 0..2 {
        renderer.render(&mut view, scene, &cam, &out, false);
    }
    let texture = view.color_target().unwrap();
    readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, SIDE, SIDE)
}
