//! The skybox reads its panorama at full detail across the equirect seam (#1322).
//! The panorama shares its cached upload, mip chain included, with materials, and
//! `equirect_uv`'s atan2 wraps from u = 1 to u = 0 looking down -X: sampled with
//! derivatives, the quads straddling that wrap pick the 1×1 mip, a line of the
//! whole image's average down the sky.

use glam::Vec3;

use super::freshness_tests::temp_png;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, Scene};

const SIDE: u32 = 16;

#[test]
fn gpu_the_sky_across_the_panorama_seam_keeps_its_colour() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(SIDE, SIDE) else {
        return;
    };
    // Red for 67° either side of the seam (-X), blue everywhere else: the average
    // a coarse mip would show is mostly blue.
    let path = temp_png("panorama_seam");
    image::RgbaImage::from_fn(256, 128, |x, _| match x < 48 || x >= 208 {
        true => image::Rgba([255, 0, 0, 255]),
        false => image::Rgba([0, 0, 255, 255]),
    })
    .save(&path)
    .unwrap();
    let mut scene = Scene::new();
    scene.skybox_path = path;
    // Around yaw 180° (down -X), so the seam crosses the view at several columns,
    // some inside a 2×2 shading quad (where derivatives see the wrap).
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

/// One frame of `scene`'s sky looking along `yaw`, as RGBA8.
fn sky(renderer: &mut Renderer, scene: &Scene, yaw: f32) -> Vec<u8> {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, SIDE, SIDE, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::ZERO, yaw, 0.0);
    for _ in 0..2 {
        // The first frame loads the panorama and rebuilds the global group.
        renderer.render(&mut view, scene, &cam, &out, false);
    }
    let texture = view.color_target().unwrap();
    readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, SIDE, SIDE)
}
