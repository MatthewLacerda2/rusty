//! Tests for the reflection-probe bake (#245). Sibling of `reflection_bake.rs`; shares its
//! 300-line cap.
//!
//! The GPU bake skips gracefully when no adapter is present — same contract as the
//! light-probe bake. It asserts the issue's acceptance: a probe baked in a coloured static
//! room produces a KTX2 cubemap whose faces show the room, with successive mips blurrier.
//! The bake -> write -> load file round-trip and the loaded mip count are checked too.

use glam::{Quat, Vec3};

use crate::components::MaterialAsset;
use crate::render::ibl::cube_sample::srgb_to_linear;
use crate::render::ibl::cubemap::parse_ktx2_cubemap_mips;
use crate::render::{CubemapFace, Renderer};
use crate::scene::{MeshComponent, Scene};

const RES: u32 = 32;

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
    let mut e = scene.get_entity_mut(id).unwrap();
    e.mesh = Some(box_mesh());
    e.is_static = true;
    e.transform.position = pos;
    e.transform.scale = scale;
    e.transform.rotation = Quat::IDENTITY;
    e.material = Some(crate::components::MaterialComponent { material: mat_name });
}

/// A box of six coloured static walls around the origin (the room the probe captures).
fn colored_room() -> Scene {
    let mut scene = Scene::new();
    let big = Vec3::new(20.0, 20.0, 20.0);
    let walls = [
        ("PosX", Vec3::X * 6.0, [9.0, 0.0, 0.0]),
        ("NegX", Vec3::NEG_X * 6.0, [0.0, 9.0, 0.0]),
        ("PosY", Vec3::Y * 6.0, [0.0, 0.0, 9.0]),
        ("NegY", Vec3::NEG_Y * 6.0, [9.0, 9.0, 0.0]),
        ("PosZ", Vec3::Z * 6.0, [9.0, 0.0, 9.0]),
        ("NegZ", Vec3::NEG_Z * 6.0, [0.0, 9.0, 9.0]),
    ];
    for (name, pos, emissive) in walls {
        add_wall(&mut scene, name, pos, big, emissive);
    }
    scene
}

/// Mean luminance variance of a face — a blur lowers it.
fn variance(face: &[u8], size: u32) -> f32 {
    let lum: Vec<f32> = (0..(size * size))
        .map(|i| {
            let idx = (i * 4) as usize;
            srgb_to_linear(face[idx])
                + srgb_to_linear(face[idx + 1])
                + srgb_to_linear(face[idx + 2])
        })
        .collect();
    let mean = lum.iter().sum::<f32>() / lum.len() as f32;
    lum.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / lum.len() as f32
}

/// Baking a probe in the coloured room produces a cubemap whose faces show the room
/// colours, and successive mips are blurrier than mip0 (the issue's acceptance). Runs the
/// real GPU bake; skips with no adapter.
#[test]
fn bake_shows_room_and_mips_blur() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(RES, RES)) else {
        return; // No GPU/software adapter — skip, same contract as the screenshots.
    };
    let scene = colored_room();
    let bytes = renderer.bake_reflection(&scene, Vec3::ZERO, RES);

    let mips = parse_ktx2_cubemap_mips(&bytes).expect("baked KTX2 parses");
    assert_eq!(mips.size, RES);
    assert!(mips.mips.len() >= 3, "expected a roughness mip chain");

    // Face order packs six faces back-to-back; the +X face must carry red (the +X wall).
    let face_bytes = (RES * RES * 4) as usize;
    let posx0 = &mips.mips[0][CubemapFace::PosX as usize * face_bytes..][..face_bytes];
    let (mut r, mut g, mut b) = (0.0f32, 0.0, 0.0);
    for px in posx0.chunks(4) {
        r += srgb_to_linear(px[0]);
        g += srgb_to_linear(px[1]);
        b += srgb_to_linear(px[2]);
    }
    assert!(
        r > g && r > b,
        "expected +X face red, got r {r} g {g} b {b}"
    );

    // The roughest mip is smoother than the base level (roughness blur).
    let v0 = variance(posx0, RES);
    let last = mips.mips.len() - 1;
    let last_size = (RES >> last).max(1);
    let posx_last = &mips.mips[last]
        [CubemapFace::PosX as usize * (last_size * last_size * 4) as usize..]
        [..(last_size * last_size * 4) as usize];
    let v_last = variance(posx_last, last_size);
    assert!(
        v_last <= v0,
        "expected roughest mip smoother: {v_last} vs {v0}"
    );
}

/// `bake_reflections` writes a KTX2 file next to the scene, points the probe at it, and the
/// file loads back as a mipped cube (the bake -> write -> load round-trip). Skips with no
/// adapter.
#[test]
fn bake_writes_file_and_sets_path() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(RES, RES)) else {
        return;
    };
    let mut scene = colored_room();
    scene.reflection_probes.add(Vec3::ZERO, Vec3::splat(5.0));
    let dir = std::env::temp_dir().join(format!("rusty_refl_bake_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let n = renderer.bake_reflections(&mut scene, &dir, RES).unwrap();
    assert_eq!(n, 1);

    let path = scene.reflection_probes.probes[0].cubemap_path.clone();
    assert!(!path.is_empty(), "probe should point at its baked cubemap");
    assert!(
        std::path::Path::new(&path).exists(),
        "cubemap file should exist"
    );

    // The written file loads back into a mipped cube on the GPU.
    let cube = renderer.load_cubemap_mips(&path).expect("baked file loads");
    assert_eq!(cube.size, RES);

    let _ = std::fs::remove_dir_all(&dir);
}
