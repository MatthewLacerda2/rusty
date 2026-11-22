use glam::{Quat, Vec3};

use crate::CubemapFace;
use rusty_components::MaterialAsset;
use rusty_scene::{MeshComponent, Scene};

const RES: u32 = 32;

fn box_mesh() -> MeshComponent {
    let (vertices, indices) = rusty_components::mesh::primitives::generate_box(1.0, 1.0, 1.0);
    MeshComponent::new("Box", vertices, indices)
}

fn add_box(
    scene: &mut Scene,
    name: &str,
    pos: Vec3,
    scale: Vec3,
    emissive: [f32; 3],
    is_static: bool,
) {
    let mat_name = format!("{name}_mat");
    scene.materials.insert(
        mat_name.clone(),
        MaterialAsset {
            base_color: [0.0, 0.0, 0.0],
            emissive,
            ..MaterialAsset::default()
        },
    );
    let id = scene.add_entity(name.to_string());
    scene.world.set_mesh(id, Some(box_mesh()));
    scene.world.set_static(id, is_static);
    {
        let mut t = scene.world.transform_mut(id).unwrap();
        t.position = pos;
        t.scale = scale;
        t.rotation = Quat::IDENTITY;
    }
    scene
        .world
        .set_material(id, Some(rusty_components::MaterialComponent::new(mat_name)));
}

fn colored_box_scene() -> Scene {
    let mut scene = Scene::new();
    let walls = [
        ("PosX", Vec3::X * 6.0, [9.0, 0.0, 0.0]),
        ("NegX", Vec3::NEG_X * 6.0, [0.0, 9.0, 0.0]),
        ("PosY", Vec3::Y * 6.0, [0.0, 0.0, 9.0]),
        ("NegY", Vec3::NEG_Y * 6.0, [9.0, 9.0, 0.0]),
        ("PosZ", Vec3::Z * 6.0, [9.0, 0.0, 9.0]),
        ("NegZ", Vec3::NEG_Z * 6.0, [0.0, 9.0, 9.0]),
    ];
    for (name, pos, emissive) in walls {
        // The origin sits outside each thin wall, seeing its front face: back faces are culled.
        let thin = Vec3::splat(20.0) - pos.normalize().abs() * 19.5;
        add_box(&mut scene, name, pos, thin, emissive, true);
    }
    let small = Vec3::new(1.5, 1.5, 1.5);
    add_box(
        &mut scene,
        "Actor",
        Vec3::X * 2.0,
        small,
        [0.0, 0.0, 9.0],
        false,
    );
    scene
}

fn center_rgb(face: &[glam::Vec3], res: u32) -> (f32, f32, f32) {
    let mid = (res / 2) as usize;
    face[mid * res as usize + mid].into()
}

#[test]
fn gpu_static_cubemap_shows_walls_and_excludes_dynamic() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return; // No GPU/software adapter — skip, same contract as the screenshots.
    };
    let scene = colored_box_scene();
    let cap = renderer.capture_static_cubemap(&scene, Vec3::ZERO, RES);

    assert_eq!(cap.resolution, RES);
    for f in cap.faces.iter() {
        assert_eq!(f.len() as u32, RES * RES);
    }

    assert_dominant(&cap, CubemapFace::PosX, Channel::R); // red wall, NOT the blue actor
    assert_dominant(&cap, CubemapFace::NegX, Channel::G);
    assert_dominant(&cap, CubemapFace::PosY, Channel::B);
    assert_dominant(&cap, CubemapFace::NegY, Channel::Rg);
    assert_dominant(&cap, CubemapFace::PosZ, Channel::Rb);
    assert_dominant(&cap, CubemapFace::NegZ, Channel::Gb);
}

enum Channel {
    R,
    G,
    B,
    Rg,
    Rb,
    Gb,
}

fn assert_dominant(cap: &crate::CubemapCapture, face: CubemapFace, ch: Channel) {
    let (r, g, b) = center_rgb(cap.face(face), cap.resolution);
    match ch {
        Channel::R => assert!(r > g && r > b, "{face:?}: expected red, got {r},{g},{b}"),
        Channel::G => assert!(g > r && g > b, "{face:?}: expected green, got {r},{g},{b}"),
        Channel::B => assert!(b > r && b > g, "{face:?}: expected blue, got {r},{g},{b}"),
        Channel::Rg => assert!(r > b && g > b, "{face:?}: expected yellow, got {r},{g},{b}"),
        Channel::Rb => assert!(
            r > g && b > g,
            "{face:?}: expected magenta, got {r},{g},{b}"
        ),
        Channel::Gb => assert!(g > r && b > r, "{face:?}: expected cyan, got {r},{g},{b}"),
    }
}
