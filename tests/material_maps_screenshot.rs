//! Material-map visual test (issue #184 / #202). Proves the renderer actually
//! SAMPLES a material's roughness map — not just its scalar. Renders the same lit,
//! metallic box twice: once with no roughness_map (the scalar alone), once with a
//! roughness_map whose green channel scales the scalar way down. Lower roughness
//! sharpens/brightens the specular highlight, so the two frames differ measurably.
//!
//! When no GPU/software adapter is present, `capture` returns `Ok(false)` and the
//! test passes without asserting (matching `postfx_screenshot.rs`).

#![cfg(feature = "dev")]

use glam::{Quat, Vec3};
use rusty::components::{LightComponent, LightType, MaterialAsset, MaterialComponent};
use rusty::dev::screenshot::capture;
use rusty::render::Camera;
use rusty::scene::{MeshComponent, Scene};

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(name)
}

/// Write a 2x2 RGBA PNG whose green channel is `g` everywhere (the roughness
/// channel in the glTF metallic-roughness convention).
fn write_roughness_png(path: &std::path::Path, g: u8) {
    let mut img = image::RgbaImage::new(2, 2);
    for px in img.pixels_mut() {
        *px = image::Rgba([255, g, 255, 255]);
    }
    img.save(path).expect("write roughness png");
}

/// A lit, metallic box scene. When `roughness_map` is `Some`, the material samples
/// it; the scalar roughness is fixed at 1.0 in both cases so any visible difference
/// is attributable to the map alone.
fn scene_with_roughness_map(roughness_map: Option<String>) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.2;

    // A directional light aimed roughly back at the camera for a clear highlight.
    let light_id = scene.add_entity("Sun".to_string());
    {
        let mut light = scene.get_entity_mut(light_id).unwrap();
        light.transform.rotation = Quat::from_rotation_x(-0.7);
        light.light = Some(LightComponent {
            light_type: LightType::Directional,
            color: Vec3::ONE,
            intensity: 3.0,
            range: 0.0,
            inner_cone: 0.0,
            outer_cone: 0.0,
        });
    }

    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty::render::mesh::generate_box(2.0, 2.0, 2.0);
    {
        let mut e = scene.get_entity_mut(id).unwrap();
        e.mesh = Some(MeshComponent {
            primitive_type: "Box".to_string(),
            asset_ref: None,
            vertices,
            indices,
            bind_palette: Vec::new(),
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            is_dirty: rusty::scene::DirtyFlag::new(true),
        });
        e.material = Some(MaterialComponent {
            material: "mat".to_string(),
        });
    }
    scene.materials.insert(
        "mat".to_string(),
        MaterialAsset {
            base_color: [0.8, 0.8, 0.8],
            metallic: 1.0,
            roughness: 1.0,
            roughness_map,
            ..MaterialAsset::default()
        },
    );
    scene
}

fn mean_brightness(path: &std::path::Path) -> Option<f64> {
    let img = image::open(path).ok()?.to_rgb8();
    let total: u64 = img
        .pixels()
        .map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64)
        .sum();
    Some(total as f64 / (img.pixels().len() as f64 * 3.0))
}

#[test]
fn roughness_map_visibly_changes_the_rendered_image() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), 180.0, 0.0);

    let map_png = tmp("rusty_roughness_map.png");
    write_roughness_png(&map_png, 26); // G ~= 0.1 -> scalar 1.0 * 0.1 = smooth

    let no_map = tmp("rusty_rough_scalar.png");
    let with_map = tmp("rusty_rough_mapped.png");

    let scalar_scene = scene_with_roughness_map(None);
    let mapped_scene = scene_with_roughness_map(Some(map_png.to_string_lossy().into_owned()));

    let cap_a = capture(&scalar_scene, &cam, &no_map, 96, 96).expect("capture must not error");
    let cap_b = capture(&mapped_scene, &cam, &with_map, 96, 96).expect("capture must not error");

    if !cap_a || !cap_b {
        eprintln!("[material-maps] no GPU/software adapter — skipping visual assertion");
        return;
    }

    let b_scalar = mean_brightness(&no_map).expect("scalar png readable");
    let b_mapped = mean_brightness(&with_map).expect("mapped png readable");
    eprintln!("[material-maps] mean brightness: scalar={b_scalar:.3}, mapped={b_mapped:.3}");
    assert!(
        (b_mapped - b_scalar).abs() > 0.5,
        "the roughness MAP must change the image vs the scalar alone \
         (scalar={b_scalar:.3}, mapped={b_mapped:.3})"
    );
}
