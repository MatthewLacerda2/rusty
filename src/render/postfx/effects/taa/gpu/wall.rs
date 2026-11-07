//! What #1846's GPU tests share: a wall of grey noise across the fixture's view,
//! the surface TAA's history must keep sharp under a turning camera and must not
//! smear a revealed patch of.

use glam::Vec3;

use crate::components::{MaterialAsset, MaterialComponent};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;

/// A 32×32 texture of seeded grey noise (sRGB 90 to 165) in the temp dir.
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
    let path = crate::test_temp::dir().join("rusty_1846_taa_noise.png");
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

/// A box primitive `scale` big at `at`, drawn with library material `name`.
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

/// The noise, tinted `tint` and tiled `tiling` times across a face.
pub(super) fn noise(tint: [f32; 3], tiling: [f32; 2]) -> MaterialAsset {
    MaterialAsset {
        base_color: tint,
        base_color_map: Some(noise_map().into()),
        tiling,
        ..MaterialAsset::default()
    }
}

/// A 30 m wide, 12 m tall wall of noise facing the camera, `z` along -Z.
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

/// A unit crate at `at` of the wall's mean grey, flat: its colour sits inside the
/// wall's neighbourhood box, so the colour clamp alone cannot tell them apart.
pub(super) fn grey_crate(scene: &mut Scene, at: Vec3) -> u32 {
    // sRGB 128, the noise's mean, in linear.
    let asset = MaterialAsset {
        base_color: [0.216; 3],
        ..MaterialAsset::default()
    };
    scene.materials.insert("crate".to_string(), asset);
    slab(scene, "crate", at, Vec3::ONE)
}

/// The luma of each pixel of an RGBA8 frame.
pub(super) fn luma(rgba: &[u8]) -> Vec<f32> {
    let l = |p: &[u8]| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32;
    rgba.chunks(4).map(l).collect()
}
