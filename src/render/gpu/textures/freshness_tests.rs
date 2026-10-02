//! The texture cache follows the file (#689): a PNG re-baked at a drawn path shows
//! its new texels on the next frame, and a map drawn before it existed is picked up
//! once it appears — baked by the engine, or written by anything else.

use glam::Vec3;

use super::freshness::MISS_RETRY_FRAMES;
use crate::components::{MaterialAsset, MaterialComponent, Tonemap};
use crate::procgen::{bake::bake_to_png, Image, Slot};
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, DirtyFlag, MeshComponent, Scene};

/// A box filling the view lit only by a white emissive modulated by `map`, with
/// every post-FX knob neutral, so the centre pixel is the map's colour.
fn emissive_scene(map: &str) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = 0.0;
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = crate::components::mesh::primitives::generate_box(8.0, 8.0, 8.0);
    let mesh = MeshComponent {
        primitive_type: "Box".to_string(),
        asset_ref: None,
        vertices,
        indices,
        bind_palette: Vec::new(),
        skin: None,
        clips: Vec::new(),
        pose_palette: Vec::new(),
        skeleton: Default::default(),
        is_dirty: DirtyFlag::new(true),
    };
    scene.world.set_mesh(id, Some(mesh));
    let material = "glow".to_string();
    scene
        .world
        .set_material(id, Some(MaterialComponent { material }));
    let asset = MaterialAsset {
        base_color: [0.0; 3],
        emissive: [1.0; 3],
        emissive_map: Some(map.to_string()),
        ..MaterialAsset::default()
    };
    scene.materials.insert("glow".to_string(), asset);
    let mut vc = crate::scene::authoring::defaults::default_visual_correction();
    (vc.active, vc.bloom_active, vc.ssr_active) = (true, false, false);
    vc.tonemap = Tonemap::None;
    scene.world.set_visual_correction(id, Some(vc));
    scene
}

/// Render one frame and read back the centre pixel's RGB.
fn centre(renderer: &mut Renderer, scene: &Scene) -> [u8; 3] {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 16, 16, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 12.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    let texture = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, 16, 16);
    let i = (8 * 16 + 8) * 4;
    [px[i], px[i + 1], px[i + 2]]
}

/// A fresh per-test PNG path under the temp dir (the file itself is not created).
fn temp_png(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("rusty_689_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.png"));
    std::fs::remove_file(&path).ok();
    path.to_string_lossy().into_owned()
}

/// Bake a flat `rgb` emissive map to `path` through the engine's one PNG writer.
fn bake(path: &str, rgb: [f32; 3]) {
    let img = Image::filled(4, [rgb[0], rgb[1], rgb[2], 1.0]);
    bake_to_png(&img, Slot::Emissive, path).unwrap();
}

fn is_red(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 40 && p[2] < 40
}

fn is_green(p: [u8; 3]) -> bool {
    p[1] > 200 && p[0] < 40 && p[2] < 40
}

#[test]
fn gpu_a_texture_rebaked_at_a_drawn_path_shows_its_new_texels() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let path = temp_png("rebake");
    bake(&path, [1.0, 0.0, 0.0]);
    let scene = emissive_scene(&path);
    let first = centre(&mut renderer, &scene);
    assert!(is_red(first), "the first bake draws red, got {first:?}");
    bake(&path, [0.0, 1.0, 0.0]);
    let second = centre(&mut renderer, &scene);
    assert!(
        is_green(second),
        "the re-bake must replace the upload, got {second:?}"
    );
}

#[test]
fn gpu_a_map_drawn_before_it_was_baked_is_picked_up_once_baked() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let path = temp_png("late_bake");
    let scene = emissive_scene(&path);
    let missing = centre(&mut renderer, &scene);
    assert!(!is_green(missing), "no file yet, so no green: {missing:?}");
    bake(&path, [0.0, 1.0, 0.0]);
    let baked = centre(&mut renderer, &scene);
    assert!(
        is_green(baked),
        "the bake must reach the next frame, got {baked:?}"
    );
}

#[test]
fn gpu_a_missing_map_written_outside_the_engine_appears_within_the_retry_window() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let path = temp_png("external");
    let scene = emissive_scene(&path);
    let missing = centre(&mut renderer, &scene);
    assert!(!is_green(missing), "no file yet, so no green: {missing:?}");
    // Written without the engine's bake, so only the miss retry can notice it.
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
