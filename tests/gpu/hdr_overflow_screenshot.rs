//! A pixel too bright for half-float is stored finite (#1699). Inverse-square light
//! has no ceiling: polished metal a hand's width under a lamp reflects past
//! `Rgba16Float`'s 65504, and an `Inf` in the HDR target turns bloom's bright pass
//! NaN, which its blur spreads into a black block around the highlight. Every forward
//! colour entry point returns through `forward::transparent::finite_hdr`, so the highlight
//! stays white and bloom stays finite. The floor is also emissive far past 65504,
//! so the overflow does not hang on how bright a GGX peak gets.
//!
//! Lavapipe stores an overflowing `Rgba16Float` texel finite (with the clamp taken
//! out, this frame still came out white there), so on Linux CI this test cannot
//! fail; a GPU that rounds to `Inf`, as Metal does, is where it holds the clamp.
//!
//! Skips (passes without asserting) when no GPU / software adapter is present.

use glam::Vec3;
use rusty::components::{LightComponent, MaterialAsset, MaterialComponent};
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{create_entity, default_post_processing, Primitive};
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 64;

/// A polished metal floor, emissive past half-float, under a black sky, an
/// intensity-3.5 lamp 15 cm above it right under the camera, and bloom on.
fn scene() -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let floor = create_entity(&mut scene, "Floor", Some(Primitive::Plane));
    scene.world.transform_mut(floor).unwrap().scale = Vec3::splat(4.0);
    scene
        .world
        .set_material(floor, Some(MaterialComponent::new("metal".to_string())));
    let metal = MaterialAsset {
        base_color: [1.0, 1.0, 1.0],
        metallic: 1.0,
        roughness: 0.05,
        emissive: [1e6, 1e6, 1e6],
        ..MaterialAsset::default()
    };
    scene.materials.insert("metal".to_string(), metal);
    let lamp = scene.add_entity("Lamp".into());
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(0.0, 0.15, 0.0);
    let light = LightComponent {
        intensity: 3.5,
        range: 4.0,
        cast_shadows: false,
        ..rusty::scene::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_post_processing();
    vc.bloom.active = true;
    scene.world.set_post_processing(volume, Some(vc));
    // A volume rides on its camera (#1434): that camera is the view's.
    scene
        .world
        .set_camera(volume, Some(rusty::components::CameraComponent::default()));
    scene
}

#[test]
fn a_highlight_past_half_float_stays_white_under_bloom() {
    let camera = Camera::new(Vec3::new(0.0, 1.5, 0.0), 0.0, -89.0);
    let path = crate::temp::file("rusty_gpu_1699_overflow.png");
    if !capture(&scene(), &camera, &path, SIZE, SIZE).expect("capture") {
        eprintln!("[hdr overflow] no GPU/software adapter — skipping");
        return;
    }
    let img = image::open(&path).unwrap().to_rgb8();
    // The floor fills the frame: every pixel white, none the black a NaN reads as
    // once tonemapped.
    for (x, y, p) in img.enumerate_pixels() {
        assert!(p.0.iter().all(|&v| v > 200), "({x}, {y}) is {:?}", p.0);
    }
}
