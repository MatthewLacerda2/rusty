//! Bloom visual test (#889). A small, very bright emissive box on a black
//! background: bloom spreads a glow onto the dark pixels around it, so the frame
//! with bloom on is brighter than the same frame with bloom off. Before this, no
//! visual test noticed the bloom pass being skipped entirely (#835's mutation run:
//! `PostFx::run_bloom → ()` survived).
//!
//! Skips without a GPU/software adapter, unless one is required.

use crate::gpu::mean_brightness;
use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::defaults::default_post_processing;
use rusty::scene::{Camera, MeshComponent, Scene};

/// A unit box scaled to `size` at `pos`, lit only by its own `emissive` term.
/// Sized by its transform: box meshes are cached by primitive name, not by size.
fn emissive_box(scene: &mut Scene, name: &str, pos: Vec3, size: Vec3, emissive: f32) {
    let id = scene.add_entity(name.to_string());
    let (vertices, indices) = rusty::components::mesh::primitives::generate_box(1.0, 1.0, 1.0);
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (pos, size);
    drop(t);
    let mesh = MeshComponent::new("Box", vertices, indices);
    scene.world.set_mesh(id, Some(mesh));
    let material = MaterialComponent::new(name.to_string());
    scene.world.set_material(id, Some(material));
    let asset = MaterialAsset {
        base_color: [0.0; 3],
        emissive: [emissive; 3],
        roughness: 1.0, // no sky reflection: the pixel is the emissive alone
        ..MaterialAsset::default()
    };
    scene.materials.insert(name.to_string(), asset);
}

/// A small, very bright box in front of a black wall that hides the sky, with
/// bloom on or off.
pub(crate) fn scene(bloom: bool) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;
    let wall = Vec3::new(40.0, 40.0, 0.2);
    emissive_box(&mut scene, "Wall", Vec3::new(0.0, 0.0, -3.0), wall, 0.0);
    emissive_box(&mut scene, "Glow", Vec3::ZERO, Vec3::ONE, 8.0);
    let post = scene.add_entity("Post".to_string());
    let mut vc = default_post_processing();
    (vc.bloom.active, vc.ssr.active) = (bloom, false);
    scene.world.set_post_processing(post, Some(vc));
    // A volume rides on its camera (#1434): that camera is the view's.
    scene
        .world
        .set_camera(post, Some(rusty::components::CameraComponent::default()));
    scene
}

#[test]
fn bloom_spreads_a_bright_surface_onto_its_surroundings() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let (off, on) = (
        crate::temp::dir().join("rusty_bloom_off.png"),
        crate::temp::dir().join("rusty_bloom_on.png"),
    );
    let c0 = capture(&scene(false), &cam, &off, 64, 64).expect("capture");
    let c1 = capture(&scene(true), &cam, &on, 64, 64).expect("capture");
    if !c0 || !c1 {
        assert!(
            !rusty::render::gpu_required(),
            "no adapter, but one is required"
        );
        return;
    }
    let (off, on) = (mean_brightness(&off), mean_brightness(&on));
    eprintln!("[bloom] off={off:.3} on={on:.3}");
    assert!(
        on > off + 5.0,
        "bloom must brighten the frame (off={off:.3}, on={on:.3})"
    );
}

mod lens_dirt;
