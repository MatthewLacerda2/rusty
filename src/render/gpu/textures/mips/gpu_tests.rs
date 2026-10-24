//! A distant texture samples a mip, not the full-size image (#1322): a fine black
//! and white checker seen from far enough that each pixel spans many cells renders
//! as an even grey, where the base level alone aliases to black and white. Cells
//! of 8 texels keep bilinear filtering of the base level from blending them.

use glam::Vec3;

use super::super::freshness_tests::{emissive_scene, temp_png};
use super::TextureKind;
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
    image::RgbaImage::from_fn(1024, 1024, |x, y| match (x / 8 + y / 8) % 2 {
        0 => image::Rgba([0, 0, 0, 255]),
        _ => image::Rgba([255, 255, 255, 255]),
    })
    .save(&path)
    .unwrap();
    let scene = emissive_scene(&path);
    let px = frame(&mut renderer, &scene);
    // The middle 4×4 pixels all land on the box's face, and each should be half
    // the light of white: sRGB 188 (the colour map's mips average linearly).
    for y in 6..10 {
        for x in 6..10 {
            let i = ((y * SIDE + x) * 4) as usize;
            for c in &px[i..i + 3] {
                assert!(
                    (170..=205).contains(c),
                    "pixel ({x},{y}) is {:?}: the base level aliased, no mip was sampled",
                    &px[i..i + 3]
                );
            }
        }
    }
}

/// Black and white texels in a 2×2 checker, alpha 0 under black: the one-texel
/// mip `kind` draws from it.
fn checker_mip(renderer: &Renderer, kind: TextureKind) -> [u8; 4] {
    let checker = image::RgbaImage::from_fn(2, 2, |x, y| match (x + y) % 2 {
        0 => image::Rgba([0, 0, 0, 0]),
        _ => image::Rgba([255, 255, 255, 255]),
    });
    let (device, queue) = (&renderer.device, &renderer.queue);
    let texture = super::super::upload((device, queue), "checker", &checker, 2);
    renderer
        .mip_generator
        .generate(device, queue, &texture, kind);
    let texel = readback::read_mip_rgba8(device, queue, (&texture, 1), (1, 1));
    texel.try_into().unwrap()
}

#[test]
fn gpu_a_colour_map_averages_in_linear_light_and_a_data_map_its_bytes() {
    let Some(renderer) = crate::render::test_gpu::headless_or_skip(SIDE, SIDE) else {
        return;
    };
    // Half the light of white is sRGB 188, not the byte midpoint 128; alpha is
    // linear in both.
    let colour = checker_mip(&renderer, TextureKind::Colour);
    let data = checker_mip(&renderer, TextureKind::Data);
    let near = |a: [u8; 4], b: [u8; 4]| a.iter().zip(b).all(|(&x, y)| x.abs_diff(y) <= 1);
    assert!(near(colour, [188, 188, 188, 128]), "colour mip {colour:?}");
    assert!(near(data, [128, 128, 128, 128]), "data mip {data:?}");
}
