use glam::Vec3;

use super::super::freshness_tests::{emissive_scene, temp_png};
use super::TextureKind;
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_scene::{Camera, Scene};

const SIDE: u32 = 16;

fn frame(renderer: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, SIDE, SIDE);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 12.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    let texture = view.color_target().unwrap();
    readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, SIDE, SIDE)
}

#[test]
fn gpu_a_distant_checker_renders_as_its_average() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(SIDE, SIDE) else {
        return;
    };
    let scene = emissive_scene(&checker("mip_checker"));
    let px = frame(&mut renderer, &scene);
    for (at, c) in middle(&px) {
        assert!(
            (170..=205).contains(&c),
            "pixel {at:?} is {c}: the base level aliased, no mip was sampled"
        );
    }
}

#[test]
fn gpu_import_settings_choose_the_mips_and_filter_and_follow_a_change() {
    use rusty_asset::sidecar::{set_texture, FilterMode, TextureSettings};
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(SIDE, SIDE) else {
        return;
    };
    let path = checker("mip_checker_point");
    let point = TextureSettings {
        filter_mode: FilterMode::Point,
        generate_mipmaps: false,
        ..TextureSettings::default()
    };
    set_texture(std::path::Path::new(&path), point).unwrap();
    let scene = emissive_scene(&path);
    for (at, c) in middle(&frame(&mut renderer, &scene)) {
        assert!(
            !(100..=230).contains(&c),
            "pixel {at:?} is {c}: filtered or a mip was sampled"
        );
    }
    set_texture(std::path::Path::new(&path), TextureSettings::default()).unwrap();
    for (at, c) in middle(&frame(&mut renderer, &scene)) {
        assert!(
            (170..=205).contains(&c),
            "pixel {at:?} is {c}: not re-uploaded"
        );
    }
}

fn checker(name: &str) -> String {
    let path = temp_png(name);
    image::RgbaImage::from_fn(1024, 1024, |x, y| match (x / 8 + y / 8) % 2 {
        0 => image::Rgba([0, 0, 0, 255]),
        _ => image::Rgba([255, 255, 255, 255]),
    })
    .save(&path)
    .unwrap();
    path
}

fn middle(px: &[u8]) -> impl Iterator<Item = ((u32, u32), u8)> + '_ {
    let at = (6..10).flat_map(|y| (6..10).map(move |x| (x, y)));
    at.flat_map(move |(x, y)| {
        let i = ((y * SIDE + x) * 4) as usize;
        px[i..i + 3].iter().map(move |&c| ((x, y), c))
    })
}

fn level_one(renderer: &Renderer, kind: TextureKind, image: &image::RgbaImage) -> Vec<u8> {
    let (device, queue) = (&renderer.device, &renderer.queue);
    let (w, h) = image.dimensions();
    let levels = super::full_chain_len(w, h);
    let writes = (queue, &renderer.staging);
    let texture = super::super::upload((device, writes), "mip", image, (levels, true));
    (renderer.mip_generator).generate(device, queue, &texture, kind);
    let below = ((w / 2).max(1), (h / 2).max(1));
    readback::read_mip_rgba8(device, queue, (&texture, 1, 0), below)
}

fn row(values: &[u8]) -> image::RgbaImage {
    image::RgbaImage::from_fn(values.len() as u32, 1, |x, _| {
        let v = values[x as usize];
        image::Rgba([v, v, v, 255])
    })
}

#[test]
fn gpu_a_colour_map_averages_in_linear_light_and_a_data_map_its_bytes() {
    let Some(renderer) = crate::test_gpu::headless_or_skip(SIDE, SIDE) else {
        return;
    };
    let checker = image::RgbaImage::from_fn(2, 2, |x, y| match (x + y) % 2 {
        0 => image::Rgba([0, 0, 0, 0]),
        _ => image::Rgba([255, 255, 255, 255]),
    });
    let colour = level_one(&renderer, TextureKind::Colour, &checker);
    let data = level_one(&renderer, TextureKind::Data, &checker);
    let near = |a: &[u8], b: [u8; 4]| a.iter().zip(b).all(|(&x, y)| x.abs_diff(y) <= 1);
    assert!(near(&colour, [188, 188, 188, 128]), "colour mip {colour:?}");
    assert!(near(&data, [128, 128, 128, 128]), "data mip {data:?}");
}

#[test]
fn gpu_an_odd_side_averages_every_texel_it_covers() {
    let Some(renderer) = crate::test_gpu::headless_or_skip(SIDE, SIDE) else {
        return;
    };
    let three = level_one(&renderer, TextureKind::Data, &row(&[255, 0, 255]));
    assert!(three[0].abs_diff(170) <= 1, "3 → 1 gave {}", three[0]);
    let five = level_one(&renderer, TextureKind::Data, &row(&[255, 0, 0, 0, 255]));
    for texel in [five[0], five[4]] {
        assert!(texel.abs_diff(102) <= 1, "5 → 2 gave {five:?}");
    }
}
