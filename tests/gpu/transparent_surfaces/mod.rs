//! Transparent surfaces à la Unity (#1523): one file per feature, over the shared
//! pool fixture here. Every test renders the same primitive scene twice, the knob
//! off and on, and compares pixels, so no golden is committed. Each skips without
//! an adapter.

mod preserve_specular;
mod receive_shadows;

use glam::{Quat, Vec3};
use rusty::components::{MaterialAsset, MaterialComponent, RenderMode};
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{create_entity, default_post_processing, Primitive};
use rusty::scene::{Camera, Scene};

/// The capture size.
pub const SIZE: u32 = 64;

/// A scene with a sun at `sun_pitch` radians below the horizon and a view volume:
/// bloom and SSR off and a fixed exposure, so a pixel is what the forward pass drew.
pub fn stage(sun_pitch: f32) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(sun_pitch);
    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_post_processing();
    vc.exposure.fixed_ev = -1.0;
    vc.bloom.active = false;
    vc.ssr.active = false;
    scene.world.set_post_processing(volume, Some(vc));
    let camera = rusty::components::CameraComponent::default();
    scene.world.set_camera(volume, Some(camera));
    scene
}

/// A box named `name` at `position` scaled by `scale`, drawn with `material`.
pub fn slab(
    scene: &mut Scene,
    name: &str,
    position: Vec3,
    scale: Vec3,
    material: MaterialAsset,
) -> u32 {
    let id = create_entity(scene, name, Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    t.position = position;
    t.scale = scale;
    drop(t);
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(name.to_string())));
    scene.materials.insert(name.to_string(), material);
    id
}

/// A `Transparent` material of `base_color` at `alpha`, the rest default.
pub fn glass(base_color: [f32; 3], alpha: f32) -> MaterialAsset {
    MaterialAsset {
        base_color,
        alpha,
        render_mode: RenderMode::Transparent,
        ..MaterialAsset::default()
    }
}

/// `scene` through `cam`, or `None` without an adapter.
pub fn shot(scene: &Scene, cam: &Camera, name: &str) -> Option<image::RgbImage> {
    let path = crate::temp::dir().join(format!("rusty_1523_{name}_{}.png", std::process::id()));
    if !capture(scene, cam, &path, SIZE, SIZE).expect("capture") {
        eprintln!("[1523] no GPU/software adapter — skipping {name}");
        return None;
    }
    let img = image::open(&path).expect("png readable").to_rgb8();
    let _ = std::fs::remove_file(path);
    Some(img)
}

/// The mean of the RGB channels over the window `x0..x1, y0..y1`.
pub fn mean_in(img: &image::RgbImage, (x0, x1): (u32, u32), (y0, y1): (u32, u32)) -> f64 {
    let mut sum = 0u64;
    for y in y0..y1 {
        for x in x0..x1 {
            sum += img
                .get_pixel(x, y)
                .0
                .iter()
                .map(|&c| u64::from(c))
                .sum::<u64>();
        }
    }
    sum as f64 / f64::from((x1 - x0) * (y1 - y0) * 3)
}

/// How many pixels are clearly darker in `a` than in `b` (by more than 60 summed).
pub fn darker(a: &image::RgbImage, b: &image::RgbImage) -> usize {
    let sum = |p: &image::Rgb<u8>| p.0.iter().map(|&c| i32::from(c)).sum::<i32>();
    a.pixels()
        .zip(b.pixels())
        .filter(|(a, b)| sum(b) - sum(a) > 60)
        .count()
}
