use glam::Vec3;

use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_asset::MapRef;
use rusty_components::{MaterialAsset, MaterialComponent};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

pub(super) const RES: u32 = 64;

pub(super) fn gradient(name: &str, side: u32) -> MapRef {
    let path = rusty_test_support::temp::dir().join(format!("{name}.png"));
    let image = image::RgbaImage::from_fn(side, side, |x, y| {
        image::Rgba([(x * 255 / side) as u8, (y * 255 / side) as u8, 90, 255])
    });
    image.save(&path).expect("a test image");
    MapRef::new(path.to_string_lossy().into_owned())
}

/// One box per map, each `z` metres in front of the camera.
pub(super) fn boxes(maps: &[(&MapRef, f32)]) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    rusty_scene::create::create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    for (i, &(map, z)) in maps.iter().enumerate() {
        let id = create_entity(&mut scene, &format!("Box_{i}"), Some(Primitive::Box));
        let x = (i as f32 - (maps.len() as f32 - 1.0) * 0.5) * 1.05;
        scene.world.transform_mut(id).unwrap().position = Vec3::new(x, 0.0, -z);
        let material = MaterialAsset {
            base_color_map: Some(map.clone()),
            ..MaterialAsset::default()
        };
        scene.materials.insert(format!("m{i}"), material);
        let worn = Some(MaterialComponent::new(format!("m{i}")));
        scene.world.set_material(id, worn);
    }
    scene
}

pub(super) fn draw(r: &mut Renderer, scene: &Scene, res: u32) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, res, res);
    let out = view.color_target_view().unwrap();
    let eye = Camera::new(Vec3::ZERO, -90.0, 0.0);
    r.render(&mut view, scene, &eye, &out, false);
    let texture = view.color_target().unwrap();
    crate::readback::read_texture_rgba8(&r.device, &r.queue, texture, res, res)
}

pub(super) fn dropped(r: &Renderer, map: &MapRef) -> u32 {
    r.gpu_textures[map]
        .mips
        .expect("a streamed texture")
        .dropped
}

/// Frames until no read is in flight and two frames left every level where it was; how many.
pub(super) fn settle(r: &mut Renderer, scene: &Scene, res: u32, maps: &[&MapRef]) -> usize {
    let mut last = Vec::new();
    for frame in 1..=200 {
        draw(r, scene, res);
        let now: Vec<u32> = maps.iter().map(|m| dropped(r, m)).collect();
        if r.streaming.pending() == 0 && now == last {
            return frame;
        }
        last = now;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("streaming never settled");
}

#[test]
fn gpu_a_far_texture_keeps_its_small_mips_and_a_near_one_all() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    r.stream_textures(true);
    let (near, far) = (gradient("stream_near", 64), gradient("stream_far", 64));
    let scene = boxes(&[(&near, 2.0), (&far, 150.0)]);
    settle(&mut r, &scene, RES, &[&near, &far]);
    assert_eq!(dropped(&r, &near), 0, "near: every level");
    assert!(
        dropped(&r, &far) >= 4,
        "far: {} levels left out",
        dropped(&r, &far)
    );
    let tex = &r.gpu_textures[&far].texture;
    assert_eq!(
        tex.size().width,
        64 >> dropped(&r, &far),
        "the upload is smaller"
    );
}

#[test]
fn gpu_moving_close_brings_the_top_levels_back_within_a_few_frames() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    r.stream_textures(true);
    let map = gradient("stream_closer", 64);
    let far = boxes(&[(&map, 150.0)]);
    settle(&mut r, &far, RES, &[&map]);
    assert!(dropped(&r, &map) >= 4);
    let near = boxes(&[(&map, 2.0)]);
    let frames = settle(&mut r, &near, RES, &[&map]);
    assert_eq!(dropped(&r, &map), 0, "close: every level back");
    assert!(frames <= 20, "{frames} frames to upgrade");
}

#[test]
fn gpu_with_streaming_off_every_texture_and_pixel_is_as_without_it() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let (near, far) = (gradient("stream_off_a", 64), gradient("stream_off_b", 64));
    let scene = boxes(&[(&near, 2.0), (&far, 40.0)]);
    let before = draw(&mut r, &scene, RES);
    for _ in 0..3 {
        draw(&mut r, &scene, RES);
    }
    assert_eq!(
        dropped(&r, &far),
        0,
        "a headless renderer streams only when asked"
    );
    r.stream_textures(true);
    settle(&mut r, &scene, RES, &[&near, &far]);
    assert!(dropped(&r, &far) > 0, "on: the far one shrank");
    r.quality.texture_streaming.active = false;
    settle(&mut r, &scene, RES, &[&near, &far]);
    assert_eq!(dropped(&r, &far), 0, "the preset off puts every level back");
    assert_eq!(draw(&mut r, &scene, RES), before, "and draws as before");
}
