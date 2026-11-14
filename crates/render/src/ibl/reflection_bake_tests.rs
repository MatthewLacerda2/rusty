//! Tests for the reflection-probe bake (#245). Sibling of `reflection_bake.rs`; shares its
//! 300-line cap.
//!
//! The GPU bake skips gracefully when no adapter is present — same contract as the
//! light-probe bake. It asserts the issue's acceptance: a probe baked in a coloured static
//! room produces a KTX2 cubemap whose faces show the room, with successive mips blurrier.
//! The bake -> write -> load file round-trip and the loaded mip count are checked too.

use glam::{Quat, Vec3};

use crate::ibl::cubemap::parse_ktx2_cubemap_mips;
use crate::ibl::reflection_prefilter::source::decode_face;
use crate::ibl::reflection_prefilter::TEXEL_BYTES;
use crate::CubemapFace;
use rusty_components::MaterialAsset;
use rusty_scene::{MeshComponent, ReflectionProbe, Scene};

/// The face the tests bake at: small, so lavapipe captures it quickly.
const RES: u32 = 128;

fn box_mesh() -> MeshComponent {
    let (vertices, indices) = rusty_components::mesh::primitives::generate_box(1.0, 1.0, 1.0);
    MeshComponent::new("Box", vertices, indices)
}

/// An emissive static wall at `pos`, scaled `scale`, glowing `emissive`.
fn add_wall(scene: &mut Scene, name: &str, pos: Vec3, scale: Vec3, emissive: [f32; 3]) {
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
    scene.world.set_static(id, true);
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

/// A box of six coloured static walls around the origin (the room the probe captures).
pub(super) fn colored_room() -> Scene {
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
        // 20 x 20 and half a unit thick, so the origin is outside every wall and
        // sees its front face: back faces are culled (#1615).
        let thin = Vec3::splat(20.0) - pos.normalize().abs() * 19.5;
        add_wall(&mut scene, name, pos, thin, emissive);
    }
    scene
}

/// Mean luminance variance of a face — a blur lowers it.
fn variance(face: &[Vec3]) -> f32 {
    let lum: Vec<f32> = face.iter().map(|c| c.element_sum()).collect();
    let mean = lum.iter().sum::<f32>() / lum.len() as f32;
    lum.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / lum.len() as f32
}

/// Face `face` of level `level` of a parsed cube, `size` texels wide.
fn face_of(level: &[u8], face: CubemapFace, size: u32) -> Vec<Vec3> {
    let len = (size * size) as usize * TEXEL_BYTES;
    decode_face(&level[face as usize * len..][..len])
}

/// Baking a probe in the coloured room produces a cubemap whose faces show the room
/// colours, and successive mips are blurrier than mip0 (the issue's acceptance). Runs the
/// real GPU bake; skips with no adapter.
#[test]
fn gpu_bake_shows_room_and_mips_blur() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return; // No GPU/software adapter — skip, same contract as the screenshots.
    };
    let scene = colored_room();
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::splat(5.0));
    let bytes = renderer.bake_reflection(&scene, &probe, RES).bytes;

    let mips = (parse_ktx2_cubemap_mips(&bytes).expect("baked KTX2 parses")).decoded();
    assert_eq!(mips.size, RES);
    assert!(mips.mips.len() >= 3, "expected a roughness mip chain");

    // The +X face carries the +X wall: red, at the wall's own radiance of 9, not
    // clipped at display white (#1696).
    let posx0 = face_of(&mips.mips[0], CubemapFace::PosX, RES);
    let mean = posx0.iter().sum::<Vec3>() / posx0.len() as f32;
    assert!(
        (mean.x - 9.0).abs() < 0.1 && mean.y < 0.1 && mean.z < 0.1,
        "expected the +X face red at 9, got {mean}"
    );

    // The roughest mip is smoother than the base level (roughness blur).
    let last = mips.mips.len() - 1;
    let posx_last = face_of(&mips.mips[last], CubemapFace::PosX, (RES >> last).max(1));
    let (v0, v_last) = (variance(&posx0), variance(&posx_last));
    assert!(
        v_last <= v0,
        "expected roughest mip smoother: {v_last} vs {v0}"
    );
}

/// `bake_reflections` writes a KTX2 file into the folder it is given, points the probe at it, and the
/// file loads back as a mipped cube (the bake -> write -> load round-trip). Skips with no
/// adapter.
#[test]
fn gpu_bake_writes_file_and_sets_path() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = colored_room();
    scene.lighting_settings.reflections.resolution = RES;
    scene.reflection_probes.add(Vec3::ZERO, Vec3::splat(5.0));
    let dir =
        rusty_test_support::temp::dir().join(format!("rusty_refl_bake_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let n = renderer.bake_reflections(&mut scene, &dir).unwrap();
    assert_eq!(n, 1);

    let path = scene.reflection_probes.probes[0].cubemap_path.clone();
    assert!(!path.is_empty(), "probe should point at its baked cubemap");
    assert!(
        std::path::Path::new(&path).exists(),
        "cubemap file should exist"
    );

    // The written file loads back, through the path the frame uses, into a mipped
    // cube the probe array admits.
    renderer.update_reflection_probes(&scene, scene.reflection_probes.probes[0].position);
    assert_eq!(renderer.reflection_probes.face(), RES, "baked file loads");

    let _ = std::fs::remove_dir_all(&dir);
}
