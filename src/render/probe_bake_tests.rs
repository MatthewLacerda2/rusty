//! Tests for the full-bounce probe bake (#241). Sibling of `probe_bake.rs`; shares
//! its 300-line source cap.
//!
//! The GPU bake skips gracefully when no adapter is present — same contract as the
//! cubemap-capture and screenshot headless tests. The pure SH projection
//! (`project_cubemap`) is also exercised on a synthetic cubemap, so the directional
//! math is covered even on an adapter-less CI host.

use glam::{Quat, Vec3};

use super::{project_cubemap, CubemapCapture, CubemapFace, Renderer};
use crate::components::MaterialAsset;
use crate::scene::{MeshComponent, Scene};

const RES: u32 = 32;

fn box_mesh() -> MeshComponent {
    let (vertices, indices) = crate::render::mesh::generate_box(1.0, 1.0, 1.0);
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

/// A bright emissive static box at `pos`, scaled `scale`, glowing `emissive`.
fn add_static_box(scene: &mut Scene, name: &str, pos: Vec3, scale: Vec3, emissive: [f32; 3]) {
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
    let mut e = scene.get_entity_mut(id).unwrap();
    e.mesh = Some(box_mesh());
    e.is_static = true;
    e.transform.position = pos;
    e.transform.scale = scale;
    e.transform.rotation = Quat::IDENTITY;
    e.material = Some(crate::components::MaterialComponent { material: mat_name });
}

/// A scene with a single big red static wall 6 units out on +X (and otherwise empty),
/// so a probe near it bounces red and one far away barely sees it.
fn red_wall_scene() -> Scene {
    let mut scene = Scene::new();
    add_static_box(
        &mut scene,
        "RedWall",
        Vec3::X * 6.0,
        Vec3::new(1.0, 20.0, 20.0),
        [9.0, 0.0, 0.0],
    );
    scene
}

/// Baking a probe next to a red wall yields an SH whose +X-facing irradiance is
/// red-dominant (directional, not a flat grey), and a probe far on the other side of
/// the room carries less red — the bake is position-dependent, the issue's acceptance.
#[test]
fn bake_red_wall_is_directional_and_position_dependent() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(RES, RES)) else {
        return; // No GPU/software adapter — skip, same contract as the screenshots.
    };
    let scene = red_wall_scene();

    let near = renderer.bake_probe(&scene, Vec3::new(4.0, 0.0, 0.0), RES);
    let far = renderer.bake_probe(&scene, Vec3::new(-4.0, 0.0, 0.0), RES);

    // Directional: facing the wall (+X) the irradiance is red-dominant, not uniform.
    let toward = near.eval(Vec3::X);
    assert!(
        toward.x > toward.y && toward.x > toward.z,
        "expected red-dominant toward wall, got {toward}"
    );
    let away = near.eval(Vec3::NEG_X);
    assert!(
        toward.x > away.x,
        "expected brighter facing the wall: {toward} vs {away}"
    );

    // Position-dependent: the near probe catches more red than the far one.
    assert!(
        near.eval(Vec3::X).x > far.eval(Vec3::X).x,
        "expected near probe redder than far: {} vs {}",
        near.eval(Vec3::X).x,
        far.eval(Vec3::X).x
    );
}

/// `bake_probes` writes SH onto every probe in place (skips with no adapter).
#[test]
fn bake_probes_fills_all_in_place() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(RES, RES)) else {
        return;
    };
    let mut scene = red_wall_scene();
    scene.probes.add_probe(Vec3::new(4.0, 0.0, 0.0));
    scene.probes.add_probe(Vec3::new(-4.0, 0.0, 0.0));
    renderer.bake_probes(&mut scene, RES);
    assert!(scene
        .probes
        .probes
        .iter()
        .all(|p| p.sh != crate::scene::sh::Sh9::zero()));
}

/// A synthetic cubemap with a single red face projects to a red, directional SH —
/// covers the projection math on hosts with no GPU adapter at all. Painting the
/// `PosX` face buffer lights the world hemisphere that face actually saw (the capture
/// renders it with a flipped camera, so the `PosX` buffer maps to the −X hemisphere),
/// so irradiance must be brightest toward −X, red, and dim the other way.
#[test]
fn project_red_face_is_red_and_directional() {
    let res = 8;
    let mut faces: [Vec<u8>; 6] = Default::default();
    for f in &mut faces {
        *f = vec![0u8; (res * res * 4) as usize];
    }
    // Paint the PosX face buffer fully red (255 in the R channel, opaque).
    for px in faces[CubemapFace::PosX as usize].chunks_mut(4) {
        px[0] = 255;
        px[3] = 255;
    }
    let capture = CubemapCapture {
        resolution: res,
        faces,
    };
    let sh = project_cubemap(&capture);
    let lit = sh.eval(Vec3::NEG_X);
    let dark = sh.eval(Vec3::X);
    assert!(lit.x > 0.0, "expected red irradiance, got {lit}");
    assert!(
        lit.y < 1.0e-4 && lit.z < 1.0e-4,
        "expected pure red, got {lit}"
    );
    assert!(lit.x > dark.x, "expected directional: {lit} vs {dark}");
}
