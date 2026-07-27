//! Tests for the static-scene cubemap capture (#243). Sibling of
//! `cubemap_capture.rs`; shares its 300-line source cap.
//!
//! Skips gracefully when no GPU/software adapter is present — the same contract as
//! the screenshot/entity-pool headless tests.

use glam::{Quat, Vec3};

use crate::components::MaterialAsset;
use crate::render::CubemapFace;
use crate::scene::{MeshComponent, Scene};

const RES: u32 = 32;

/// A unit box mesh (the wall/actor geometry).
fn box_mesh() -> MeshComponent {
    let (vertices, indices) = crate::render::gpu::mesh::generate_box(1.0, 1.0, 1.0);
    MeshComponent {
        primitive_type: "Box".to_string(),
        asset_ref: None,
        vertices,
        indices,
        bind_palette: Vec::new(),
        skin: None,
        clips: Vec::new(),
        pose_palette: Vec::new(),
        is_dirty: crate::scene::DirtyFlag::new(true),
    }
}

/// Add an emissive box at `pos`, scaled to `scale`, with library material `mat_name`
/// glowing `emissive`. `is_static` decides whether the capture should include it.
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
    scene.world.set_material(
        id,
        Some(crate::components::MaterialComponent { material: mat_name }),
    );
}

/// Build a box of six coloured static walls around the origin, each glowing a
/// distinct colour, plus one dynamic (non-static) blue actor sitting between the
/// camera and the +X wall. Big thin walls 6 units out on each axis so the 90° face
/// fully sees its wall.
fn colored_box_scene() -> Scene {
    let mut scene = Scene::new();
    let big = Vec3::new(20.0, 20.0, 20.0);
    // (name, position, emissive colour).
    let walls = [
        ("PosX", Vec3::X * 6.0, [9.0, 0.0, 0.0]),
        ("NegX", Vec3::NEG_X * 6.0, [0.0, 9.0, 0.0]),
        ("PosY", Vec3::Y * 6.0, [0.0, 0.0, 9.0]),
        ("NegY", Vec3::NEG_Y * 6.0, [9.0, 9.0, 0.0]),
        ("PosZ", Vec3::Z * 6.0, [9.0, 0.0, 9.0]),
        ("NegZ", Vec3::NEG_Z * 6.0, [0.0, 9.0, 9.0]),
    ];
    for (name, pos, emissive) in walls {
        add_box(&mut scene, name, pos, big, emissive, true);
    }
    // A dynamic actor right in front of the +X wall, glowing blue. If the static
    // filter works it must NOT appear in the +X face.
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

/// The centre pixel's (r, g, b) of a face.
fn center_rgb(face: &[u8], res: u32) -> (u8, u8, u8) {
    let mid = (res / 2) as usize;
    let idx = (mid * res as usize + mid) * 4;
    (face[idx], face[idx + 1], face[idx + 2])
}

/// Capturing from inside the coloured box yields six faces showing the wall colours
/// in the correct directions, and the dynamic actor blocking the +X wall does NOT
/// appear (the +X face is still red, not the actor's blue).
#[test]
fn static_cubemap_shows_walls_and_excludes_dynamic() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return; // No GPU/software adapter — skip, same contract as the screenshots.
    };
    let scene = colored_box_scene();
    let cap = renderer.capture_static_cubemap(&scene, Vec3::ZERO, RES);

    assert_eq!(cap.resolution, RES);
    for f in cap.faces.iter() {
        assert_eq!(f.len() as u32, RES * RES * 4);
    }

    // Each face's dominant channel must match the wall it points at.
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

fn assert_dominant(cap: &crate::render::CubemapCapture, face: CubemapFace, ch: Channel) {
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
