use glam::Vec3;

use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_asset::convert::texture_ref::is_written;
use rusty_components::{MaterialAsset, MaterialComponent, Tonemap};
use rusty_core::fs::freshness::MISS_RETRY_FRAMES;
use rusty_procgen::{bake::bake_to_png, Image, Slot};
use rusty_scene::{Camera, MeshComponent, Scene};
use rusty_test_support::rebakes::{follows_rebakes, Bake};

pub(super) fn emissive_scene(map: &str) -> Scene {
    let mut scene = Scene::new();
    crate::test_gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty_components::mesh::primitives::generate_box(8.0, 8.0, 8.0);
    let mesh = MeshComponent::new("Box", vertices, indices);
    scene.world.set_mesh(id, Some(mesh));
    let material = "glow".to_string();
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(material)));
    let asset = MaterialAsset {
        base_color: [0.0; 3],
        emissive: [1.0; 3],
        emissive_map: Some(map.into()),
        ..MaterialAsset::default()
    };
    scene.materials.insert("glow".to_string(), asset);
    let mut vc = rusty_scene::create::defaults::default_post_processing();
    (vc.active, vc.bloom.active, vc.ssr.active) = (true, false, false);
    vc.tonemapping.mode = Tonemap::None;
    scene.world.set_post_processing(id, Some(vc));
    scene
        .world
        .set_camera(id, Some(rusty_components::CameraComponent::default()));
    scene
}

pub(super) fn centre(renderer: &mut Renderer, scene: &Scene) -> [u8; 3] {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 16, 16);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 12.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    let texture = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, 16, 16);
    let i = (8 * 16 + 8) * 4;
    [px[i], px[i + 1], px[i + 2]]
}

pub(super) fn temp_png(name: &str) -> String {
    let dir = rusty_test_support::temp::dir().join("rusty_689");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.png"));
    std::fs::remove_file(&path).ok();
    path.to_string_lossy().into_owned()
}

fn bake(path: &str, rgb: [f32; 3]) {
    let img = Image::filled(4, [rgb[0], rgb[1], rgb[2], 1.0]);
    bake_to_png(&img, Slot::Emissive, path).unwrap();
}

const RED: [f32; 3] = [1.0, 0.0, 0.0];
const GREEN: [f32; 3] = [0.0, 1.0, 0.0];

fn is_red(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 40 && p[2] < 40
}

fn is_green(p: [u8; 3]) -> bool {
    p[1] > 200 && p[0] < 40 && p[2] < 40
}

#[test]
fn gpu_a_texture_follows_its_bakes() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let path = temp_png("rebake");
    let scene = emissive_scene(&path);
    follows_rebakes(
        &path,
        |p, which| bake(p, if which == Bake::A { RED } else { GREEN }),
        |_| centre(&mut renderer, &scene),
        |&px, which| {
            if which == Bake::A {
                is_red(px)
            } else {
                is_green(px)
            }
        },
    );
}

#[test]
fn gpu_a_missing_map_written_outside_the_engine_appears_within_the_retry_window() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let path = temp_png("external");
    let scene = emissive_scene(&path);
    let missing = centre(&mut renderer, &scene);
    assert!(!is_green(missing), "no file yet, so no green: {missing:?}");
    image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 255, 0, 255]))
        .save(&path)
        .unwrap();
    for _ in 0..MISS_RETRY_FRAMES {
        centre(&mut renderer, &scene);
    }
    let found = centre(&mut renderer, &scene);
    assert!(
        is_green(found),
        "the retry must load the new file, got {found:?}"
    );
}

#[test]
fn a_texture_reference_goes_stale_with_its_source() {
    let stale = rusty_core::fs::written::Stale::new(vec!["assets/quad.fbx".into()]);
    assert!(is_written(&stale, "assets/quad.fbx#tex/Skin"));
    assert!(!is_written(&stale, "assets/other.fbx#tex/Skin"));
    assert!(!is_written(&stale, "assets/skin.png"));
}
