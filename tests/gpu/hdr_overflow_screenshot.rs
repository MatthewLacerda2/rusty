//! A pixel too bright for half-float is stored finite: metal under a lamp reflects
//! past `Rgba16Float`'s 65504, and an `Inf` there turns bloom's bright pass NaN, a black
//! block round the highlight. Forward colour returns through `transparent::finite_hdr`,
//! so the highlight stays white. Lavapipe stores an overflow finite even unclamped, so
//! this fails only on a GPU that rounds to `Inf`, as Metal does.

use glam::Vec3;
use rusty::components::{LightComponent, MaterialAsset, MaterialComponent};
use rusty::dev::screenshot::capture;
use rusty::edit::authoring::{create_entity, default_post_processing, Primitive};
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 64;

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
        // Emissive past half-float too: the overflow holds however bright the GGX peak is.
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
        mode: rusty::components::LightMode::Mixed,
        ..rusty::edit::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_post_processing();
    vc.bloom.active = true;
    scene.world.set_post_processing(volume, Some(vc));
    // A volume rides on its camera: that camera is the view's.
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
    // The floor fills the frame: no pixel the black a NaN tonemaps to.
    for (x, y, p) in img.enumerate_pixels() {
        assert!(p.0.iter().all(|&v| v > 200), "({x}, {y}) is {:?}", p.0);
    }
}
