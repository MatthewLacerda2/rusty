//! #1993's smallest repro: a dense red `Transparent` pane modelled in world
//! coordinates, its origin 21 m away behind the camera, and a dense blue
//! `Transparent` box 1.5 m in front of it. Sorted by origin, the pane was nearest
//! and drew last, over the box; sorted by bounds centre, the box draws over it.

use glam::Vec3;
use rusty::components::mesh::primitives::generate_box;
use rusty::components::{MaterialAsset, MaterialComponent, RenderMode};
use rusty::dev::screenshot::capture;
use rusty::scene::{Camera, MeshComponent, Scene};

use super::{centre_rgb, world_modelled};

fn glass(scene: &mut Scene, id: u32, name: &str, emissive: [f32; 3]) {
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(name.to_string())));
    let asset = MaterialAsset {
        base_color: [0.0; 3],
        emissive,
        alpha: 0.9,
        render_mode: RenderMode::Transparent,
        ..MaterialAsset::default()
    };
    scene.materials.insert(name.to_string(), asset);
}

#[test]
fn a_world_modelled_pane_behind_a_box_draws_first() {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;
    scene.skybox_path = String::new();
    let at = (Vec3::new(0.0, 0.0, 21.0), Vec3::ZERO);
    let pane = world_modelled(&mut scene, "Pane", at, generate_box(8.0, 8.0, 0.1));
    glass(&mut scene, pane, "red", [1.0, 0.0, 0.0]);
    let block = scene.add_entity("Box".to_string());
    scene.world.transform_mut(block).unwrap().position = Vec3::new(0.0, 0.0, 1.5);
    let (vertices, indices) = generate_box(1.0, 1.0, 1.0);
    let mesh = MeshComponent::new("Box", vertices, indices);
    scene.world.set_mesh(block, Some(mesh));
    glass(&mut scene, block, "blue", [0.0, 0.0, 1.0]);

    let cam = Camera::new(Vec3::new(0.0, 0.0, 7.0), -90.0, 0.0);
    let path = crate::temp::file("rusty_gpu_bounds_centre_transparent.png");
    if !capture(&scene, &cam, &path, 32, 32).expect("capture") {
        eprintln!("[bounds-centre sort] no GPU/software adapter — skipping");
        return;
    }
    let [r, _, b] = centre_rgb(&path);
    eprintln!("[bounds-centre sort] centre rgb r={r} b={b}");
    assert!(
        b > r + 40,
        "the nearer blue box must composite over the world-modelled pane (r={r}, b={b})"
    );
}
