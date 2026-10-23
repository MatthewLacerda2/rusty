//! Material-map visual test (issue #184 / #202). Proves the renderer SAMPLES a
//! material's map textures (not just its scalar factors) through the per-entity
//! material bind group added in #202.
//!
//! It drives the metallic map: the shader gates the ambient diffuse term by
//! `(1 - metallic)` (see `shader.wgsl`), so under ambient light a fully-metallic
//! surface is dark and a non-metallic one is bright — a strong, view- and
//! light-geometry-independent signal. Three configs disambiguate where any failure
//! lies (the prints land in the CI log):
//!   `scalar 0` bright vs `scalar 1` dark proves the metallic SCALAR reaches the
//!   shader (uniform layout OK); `scalar 1 + black map (.b=0)` bright proves the
//!   MAP is sampled (1.0 * 0.0 = 0). The roughness map rides the identical path
//!   (same bind group, same `select()` in the shader, `.g` channel); its WGSL
//!   binding is validated GPU-free by `all_shaders_compose`.
//!
//! When no GPU/software adapter is present, `capture` returns `Ok(false)` and the
//! test passes without asserting (matching `postfx/screenshot.rs`).

use crate::gpu::mean_brightness;
use crate::temp::file as tmp;
use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;
use rusty::scene::{MeshComponent, Scene, VisualCorrectionComponent};

/// 2x2 black RGBA PNG. Metallic reads the BLUE channel (glTF convention), so
/// `.b = 0` drives metallic to zero.
fn write_black_png(path: &std::path::Path) {
    let mut img = image::RgbaImage::new(2, 2);
    for px in img.pixels_mut() {
        *px = image::Rgba([0, 0, 0, 255]);
    }
    img.save(path).expect("write metallic png");
}

fn vc() -> VisualCorrectionComponent {
    VisualCorrectionComponent {
        bloom_active: false,
        bloom_threshold: 0.6,
        ssr_active: false,
        ssr_temporal_upsampling: false,
        ..rusty::scene::authoring::default_visual_correction()
    }
}

/// An ambient-lit box with the given metallic scalar and optional metallic map.
fn scene(metallic: f32, metallic_map: Option<String>) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 1.0;

    let id = scene.add_entity("Box".to_string());
    // A large box so it fills the frame — the mean-brightness signal is dominated by
    // the surface, not the background, making the metallic gating clearly visible.
    let (vertices, indices) = rusty::components::mesh::primitives::generate_box(8.0, 8.0, 8.0);
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("mat".to_string())));
    scene.world.set_visual_correction(id, Some(vc()));
    scene.materials.insert(
        "mat".to_string(),
        MaterialAsset {
            base_color: [0.9, 0.9, 0.9],
            metallic,
            roughness: 1.0,
            metallic_map: metallic_map.map(Into::into),
            ..MaterialAsset::default()
        },
    );
    scene
}

#[test]
fn metallic_map_visibly_changes_the_rendered_image() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), 180.0, 0.0);
    let map_png = tmp("rusty_metallic_map.png");
    write_black_png(&map_png);
    let map = map_png.to_string_lossy().into_owned();

    let p_bright = tmp("rusty_metal_s0.png");
    let p_dark = tmp("rusty_metal_s1.png");
    let p_mapped = tmp("rusty_metal_mapped.png");

    let c0 = capture(&scene(0.0, None), &cam, &p_bright, 96, 96).expect("capture");
    let c1 = capture(&scene(1.0, None), &cam, &p_dark, 96, 96).expect("capture");
    let c2 = capture(&scene(1.0, Some(map)), &cam, &p_mapped, 96, 96).expect("capture");
    if !c0 || !c1 || !c2 {
        eprintln!("[material-maps] no GPU/software adapter — skipping visual assertion");
        return;
    }

    let bright = mean_brightness(&p_bright);
    let dark = mean_brightness(&p_dark);
    let mapped = mean_brightness(&p_mapped);
    eprintln!("[material-maps] metallic scalar0={bright:.3} scalar1={dark:.3} mapped={mapped:.3}");
    assert!(
        bright > dark + 2.0,
        "metallic scalar must gate ambient diffuse (scalar0={bright:.3}, scalar1={dark:.3})"
    );
    assert!(
        mapped > dark + 2.0,
        "the metallic MAP must drive metallic to 0 like the scalar (mapped={mapped:.3}, scalar1={dark:.3})"
    );
}
