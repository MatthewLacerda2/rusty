//! Directional specular from a directional lightmap (#1695, step 1): a `Baked` lamp's
//! highlight on a lightmapped surface, every bit of whose light is baked.
//!
//! * `highlight`: on a glossy plane the baked lobe's highlight sits where the same
//!   lamp's `Realtime` one does, its peak within a stated factor; at roughness 1 it
//!   changes nothing visible; under `HalfLambert` it still does.
//! * `sky`: a sky makes a texel directional with no lamp, and gives no lobe; a lamp
//!   under a sky keeps its highlight.
//! * `room`: a `Mixed` spot's bounce gives a glossy ceiling no lobe; a lamp in a
//!   bright-walled room aims its lobe at itself.
//! * `capture`: a bake capture never draws it.
//! * `behind`: a two-sided surface's back face reads its front's baked light.
//!
//! Skips (passes without asserting) when no GPU / software adapter is present.

mod behind;
mod capture;
mod highlight;
mod room;
mod sky;

use glam::{Quat, Vec3};
use rusty::components::{LightComponent, LightMode, LightType, MaterialAsset, MaterialComponent};
use rusty::dev::bake::lightmap::bake_scene_lightmaps;
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{primitive_mesh_component, Primitive};
use rusty::scene::lighting::lightmap::BakeSettings;
use rusty::scene::{Camera, Scene};

pub(super) const SIZE: u32 = 96;

/// A 6 x 6 static plane of `roughness` under a point lamp of `mode` a metre up,
/// under a black sky.
pub(super) fn scene(mode: LightMode, roughness: f32) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let material = MaterialAsset {
        base_color: [0.5, 0.5, 0.5],
        roughness,
        ..MaterialAsset::default()
    };
    plane(
        &mut scene,
        "plane",
        Vec3::ZERO,
        Quat::IDENTITY,
        0.4,
        material,
    );
    lamp(&mut scene, Vec3::Y, Quat::IDENTITY, point(mode));
    scene
}

/// A static plane (15 m a side at `scale` 1, facing +Y before `rotation`) of `material`.
pub(super) fn plane(
    scene: &mut Scene,
    name: &str,
    at: Vec3,
    rotation: Quat,
    scale: f32,
    material: MaterialAsset,
) {
    let plane = scene.add_entity(name.to_string());
    scene
        .world
        .set_mesh(plane, primitive_mesh_component(Primitive::Plane));
    scene.world.set_static(plane, true);
    place(scene, plane, at, rotation);
    scene.world.transform_mut(plane).unwrap().scale = Vec3::splat(scale);
    scene.materials.insert(name.to_string(), material);
    let component = MaterialComponent::new(name.to_string());
    scene.world.set_material(plane, Some(component));
}

fn place(scene: &mut Scene, id: u32, at: Vec3, rotation: Quat) {
    let mut transform = scene.world.transform_mut(id).unwrap();
    (transform.position, transform.rotation) = (at, rotation);
}

/// A point lamp of `mode`, intensity 1 and range 8.
pub(super) fn point(mode: LightMode) -> LightComponent {
    LightComponent {
        light_type: LightType::Point,
        intensity: 1.0,
        range: 8.0,
        mode,
        ..rusty::scene::authoring::default_light()
    }
}

/// `light` placed at `at`, turned by `rotation` (a spot shines along -Z).
pub(super) fn lamp(scene: &mut Scene, at: Vec3, rotation: Quat, light: LightComponent) {
    let lamp = scene.add_entity("Lamp".to_string());
    scene.world.set_light(lamp, Some(light));
    place(scene, lamp, at, rotation);
}

/// Bake `scene`'s lightmaps into a temp folder named `name`; returns its path.
pub(super) fn bake(scene: &mut Scene, name: &str, directional: bool) -> String {
    let dir = crate::temp::dir().join(format!("rusty_gpu_baked_spec_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let settings = BakeSettings {
        texels_per_unit: 10.0,
        samples: 16,
        bounces: 1,
        seed: 5,
        max_lightmap_size: 64,
        filter_radius: 1,
        directional,
    };
    assert!(bake_scene_lightmaps(scene, Some(&path), &settings).is_ok_and(|n| n > 0));
    path
}

/// `scene` from `camera`, as the luminance of every pixel (0..=255). `None` without
/// an adapter.
pub(super) fn shot(scene: &Scene, camera: &Camera, name: &str) -> Option<Vec<f32>> {
    let path = crate::temp::dir().join(format!("rusty_gpu_baked_spec_{name}.png"));
    capture(scene, camera, &path, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    let img = image::open(&path).unwrap().to_rgb8();
    let luma =
        |p: &image::Rgb<u8>| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32;
    Some(img.pixels().map(luma).collect())
}

/// Where `a` exceeds `b` most, as (x, y, by how much).
pub(super) fn peak(a: &[f32], b: &[f32]) -> (u32, u32, f32) {
    let diffs = a.iter().zip(b).map(|(a, b)| a - b).enumerate();
    let (i, d) = diffs.fold(
        (0, f32::MIN),
        |best, (i, d)| if d > best.1 { (i, d) } else { best },
    );
    (i as u32 % SIZE, i as u32 / SIZE, d)
}

/// The most `a` and `b` differ by at any pixel.
pub(super) fn most(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max)
}
