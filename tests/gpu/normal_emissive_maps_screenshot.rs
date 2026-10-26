//! Normal-map + emissive-map visual test (issue #207): proves the renderer SAMPLES a
//! material's `normal_map`/`emissive_map` through the per-entity material bind group
//! (the two map no-ops #178 guarded), building on the #202 path. Light-independent
//! signals on an up-facing ambient-lit plane:
//!   - Emissive: factor [1,1,1] modulated by a WHITE map (`*1`, glow kept) vs a BLACK
//!     map (`*0`, glow cancelled) — white-mapped is brighter.
//!   - Normal: flat map [128,128,255] (tangent +Z, N stays +Y) vs sideways map
//!     [255,128,128] (tangent +X, N rotated into the plane, N.y drops) — the flat one
//!     reads more of the ambient sky pole, so it is brighter. Proves TBN perturbation.
//!
//! No GPU/software adapter: `capture` returns `Ok(false)` and the test passes without
//! asserting (matching `material_maps_screenshot.rs`).

use crate::gpu::mean_brightness;
use crate::temp::file as tmp;
use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;
use rusty::scene::{MeshComponent, PostProcessingComponent, Scene};

/// Write a 2x2 solid-RGB PNG (alpha 255).
fn write_png(path: &std::path::Path, rgb: [u8; 3]) {
    let mut img = image::RgbaImage::new(2, 2);
    for px in img.pixels_mut() {
        *px = image::Rgba([rgb[0], rgb[1], rgb[2], 255]);
    }
    img.save(path).expect("write png");
}

fn vc() -> PostProcessingComponent {
    PostProcessingComponent {
        bloom: rusty::components::Bloom {
            active: false,
            threshold: 0.6,
            ..Default::default()
        },
        ssr: rusty::components::Ssr {
            active: false,
            temporal_upsampling: false,
            ..Default::default()
        },
        ..rusty::scene::authoring::default_post_processing()
    }
}

/// A large ambient-lit plane carrying the given material. The plane faces +Y so its
/// ambient term sits at the sky pole, maximizing the normal-map signal.
pub(super) fn scene(material: MaterialAsset) -> Scene {
    let mut s = Scene::new();
    s.ambient_intensity = 1.0;
    let id = s.add_entity("Plane".to_string());
    let (vertices, indices) = rusty::components::mesh::primitives::generate_plane(16.0, 16.0);
    let mesh = MeshComponent::new("Plane", vertices, indices);
    let mat = MaterialComponent::new("mat".to_string());
    s.world.set_mesh(id, Some(mesh));
    s.world.set_material(id, Some(mat));
    s.world.set_post_processing(id, Some(vc()));
    // A volume rides on its camera (#1434): that camera is the view's.
    s.world
        .set_camera(id, Some(rusty::components::CameraComponent::default()));
    s.materials.insert("mat".to_string(), material);
    s
}

/// Camera looking straight down at the plane from above.
fn top_cam() -> Camera {
    Camera::new(Vec3::new(0.0, 12.0, 0.0), 0.0, -90.0)
}

#[test]
fn emissive_map_visibly_modulates_emission() {
    let white = tmp("rusty_emissive_white.png");
    let black = tmp("rusty_emissive_black.png");
    write_png(&white, [255, 255, 255]);
    write_png(&black, [0, 0, 0]);

    let mat = |map: &std::path::Path| MaterialAsset {
        base_color: [0.0, 0.0, 0.0],
        emissive: [1.0, 1.0, 1.0],
        emissive_map: Some(map.to_string_lossy().into_owned().into()),
        ..MaterialAsset::default()
    };

    let p_white = tmp("rusty_emis_white_out.png");
    let p_black = tmp("rusty_emis_black_out.png");
    let cw = capture(&scene(mat(&white)), &top_cam(), &p_white, 96, 96).expect("capture");
    let cb = capture(&scene(mat(&black)), &top_cam(), &p_black, 96, 96).expect("capture");
    if !cw || !cb {
        eprintln!("[normal-emissive] no GPU adapter — skipping emissive assertion");
        return;
    }
    let (bw, bb) = (mean_brightness(&p_white), mean_brightness(&p_black));
    eprintln!("[normal-emissive] emissive white={bw:.3} black={bb:.3}");
    assert!(
        bw > bb + 2.0,
        "white emissive map must keep the glow a black one cancels (white={bw:.3}, black={bb:.3})"
    );
}

#[test]
fn normal_map_visibly_perturbs_shading() {
    let flat = tmp("rusty_normal_flat.png");
    let side = tmp("rusty_normal_side.png");
    write_png(&flat, [128, 128, 255]); // tangent +Z: no perturbation
    write_png(&side, [255, 128, 128]); // tangent +X: rotate N into the plane

    let mat = |map: &std::path::Path| MaterialAsset {
        base_color: [0.9, 0.9, 0.9],
        normal_map: Some(map.to_string_lossy().into_owned().into()),
        ..MaterialAsset::default()
    };

    let p_flat = tmp("rusty_normal_flat_out.png");
    let p_side = tmp("rusty_normal_side_out.png");
    let cf = capture(&scene(mat(&flat)), &top_cam(), &p_flat, 96, 96).expect("capture");
    let cs = capture(&scene(mat(&side)), &top_cam(), &p_side, 96, 96).expect("capture");
    if !cf || !cs {
        eprintln!("[normal-emissive] no GPU adapter — skipping normal assertion");
        return;
    }
    let (bf, bs) = (mean_brightness(&p_flat), mean_brightness(&p_side));
    eprintln!("[normal-emissive] normal flat={bf:.3} side={bs:.3}");
    assert!(
        bf > bs + 2.0,
        "a sideways normal map must drop the up-facing ambient term (flat={bf:.3}, side={bs:.3})"
    );
}
