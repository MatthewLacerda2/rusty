use glam::Vec3;

use super::super::fixture::frame;
use crate::render_texture::is_render_texture;
use crate::Renderer;
use rusty_asset::MapRef;
use rusty_components::{MaterialAsset, MaterialComponent};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

pub(super) fn png(name: &str, rgb: [u8; 3]) -> MapRef {
    let path = rusty_test_support::temp::dir().join(format!("{name}.png"));
    let [r, g, b] = rgb;
    let image = image::RgbaImage::from_pixel(16, 16, image::Rgba([r, g, b, 255]));
    image.save(&path).expect("a test image");
    MapRef::new(path.to_string_lossy().into_owned())
}

pub(super) fn textured(maps: &[&MapRef]) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    rusty_scene::create::create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    for (i, &map) in maps.iter().enumerate() {
        let id = create_entity(&mut scene, &format!("Box_{i}"), Some(Primitive::Box));
        scene.world.transform_mut(id).unwrap().position = Vec3::new(i as f32 - 1.0, 0.0, -4.0);
        let name = format!("m{i}");
        let material = MaterialAsset {
            base_color_map: Some(map.clone()),
            ..MaterialAsset::default()
        };
        scene.materials.insert(name.clone(), material);
        scene
            .world
            .set_material(id, Some(MaterialComponent::new(name)));
    }
    scene
}

pub(super) fn maps(r: &Renderer) -> Vec<String> {
    let keys = r
        .gpu_textures
        .keys()
        .filter(|m| !is_render_texture(m.as_str()));
    let mut names: Vec<String> = keys
        .map(|m| m.as_str().rsplit('/').next().unwrap_or_default().to_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn gpu_a_load_releases_the_textures_only_the_scene_before_used() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (a1, a2, b) = (
        png("a1", [200, 0, 0]),
        png("a2", [0, 200, 0]),
        png("b", [0, 0, 200]),
    );
    let (_, a) = frame(&mut r, &textured(&[&a1, &a2]));
    assert_eq!(maps(&r), ["a1.png", "a2.png"]);
    let (_, after) = frame(&mut r, &textured(&[&b]));
    assert_eq!(maps(&r), ["b.png"], "A's maps are gone");
    assert_eq!(
        after.texture_bytes * 2,
        a.texture_bytes,
        "B's one map alone"
    );
    assert_eq!(r.material_group_count(), 1, "no group holds A's maps");
}

#[test]
fn gpu_a_map_both_scenes_use_is_uploaded_once_and_kept() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (shared, a, b) = (
        png("s", [90, 90, 90]),
        png("a", [200, 0, 0]),
        png("b", [0, 0, 200]),
    );
    frame(&mut r, &textured(&[&shared, &a]));
    let next = textured(&[&shared, &b]);
    let (_, loaded) = frame(&mut r, &next);
    assert_eq!(loaded.uploads.count, 1, "b, not the shared map");
    assert_eq!(maps(&r), ["b.png", "s.png"]);
    let (_, again) = frame(&mut r, &next);
    assert_eq!(again.uploads.count, 0);
}

#[test]
fn gpu_play_and_stop_upload_no_texture_and_hold_the_edit_scenes_maps() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (e, l) = (png("e", [200, 0, 0]), png("l", [0, 0, 200]));
    let mut edit = textured(&[&e]);
    frame(&mut r, &edit);
    edit.set_playing(true);
    let (_, play) = frame(&mut r, &edit);
    assert_eq!(play.uploads.count, 0, "Play");
    let mut level = textured(&[&l]);
    level.set_playing(true);
    frame(&mut r, &level);
    assert_eq!(maps(&r), ["e.png", "l.png"], "the edit scene is held");
    edit.set_playing(false);
    let (_, stop) = frame(&mut r, &edit);
    assert_eq!(stop.uploads.count, 0, "Stop draws the held scene's maps");
    assert_eq!(maps(&r), ["e.png"], "the level's are released");
}

#[test]
fn gpu_a_textured_scene_loaded_again_after_another_draws_the_same_pixels() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (a1, a2, b) = (
        png("a1", [200, 30, 0]),
        png("a2", [0, 200, 60]),
        png("b", [0, 0, 200]),
    );
    let (before, _) = frame(&mut r, &textured(&[&a1, &a2]));
    frame(&mut r, &textured(&[&b]));
    assert_eq!(maps(&r), ["b.png"]);
    let (after, c) = frame(&mut r, &textured(&[&a1, &a2]));
    assert_eq!(c.uploads.count, 2, "read again from disk");
    assert!(before == after, "a reload draws what the first load drew");
    assert!(
        before.iter().any(|&p| p != before[0]),
        "something is on screen"
    );
}
