use glam::Vec3;

use rusty_components::{MaterialAsset, MaterialComponent};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

fn noise_map() -> String {
    let mut state = 0x2545_f491u32;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        90 + (state >> 24) as u8 % 76
    };
    let img = image::RgbaImage::from_fn(32, 32, |_, _| {
        let v = next();
        image::Rgba([v, v, v, 255])
    });
    let path = rusty_test_support::temp::dir().join("rusty_1846_taa_noise.png");
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

fn slab(scene: &mut Scene, name: &str, at: Vec3, scale: Vec3) -> u32 {
    let id = create_entity(scene, name, Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (at, scale);
    drop(t);
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(name.to_string())));
    id
}

pub(super) fn noise(tint: [f32; 3], tiling: [f32; 2]) -> MaterialAsset {
    MaterialAsset {
        base_color: tint,
        base_color_map: Some(noise_map().into()),
        tiling,
        ..MaterialAsset::default()
    }
}

pub(super) fn noisy_wall(scene: &mut Scene, z: f32) -> u32 {
    let asset = noise([1.0; 3], [8.0, 4.0]);
    scene.materials.insert("wall".to_string(), asset);
    slab(
        scene,
        "wall",
        Vec3::new(0.0, 0.0, z),
        Vec3::new(30.0, 12.0, 0.2),
    )
}

pub(super) fn grey_crate(scene: &mut Scene, at: Vec3) -> u32 {
    let asset = MaterialAsset {
        base_color: [0.216; 3],
        ..MaterialAsset::default()
    };
    scene.materials.insert("crate".to_string(), asset);
    slab(scene, "crate", at, Vec3::ONE)
}

pub(super) fn luma(rgba: &[u8]) -> Vec<f32> {
    let l = |p: &[u8]| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32;
    rgba.chunks(4).map(l).collect()
}
