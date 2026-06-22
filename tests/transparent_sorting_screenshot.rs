//! Transparent back-to-front compositing test (#242). Proves the alpha-blended
//! transparent pass sorts its draws by view-space depth, so two overlapping
//! translucent quads composite the SAME way regardless of the order they were
//! inserted into the scene — the issue's core acceptance.
//!
//! Colours are light-independent by construction: zero ambient + no lights, so a
//! surface's lit colour is exactly its `emissive` factor. Each quad is `Transparent`
//! with a strong emissive tint and alpha < 1, so the rendered pixel is a pure blend.
//! A near GREEN quad in front of a far RED quad must read green-dominant; and flipping
//! the insertion order must not change the pixel (the sort, not draw order, decides).
//!
//! No GPU/software adapter -> `capture` returns `Ok(false)` and the test passes
//! without asserting (matching `material_maps_screenshot.rs`).

#![cfg(feature = "dev")]

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent, RenderMode, Tonemap};
use rusty::dev::screenshot::capture;
use rusty::render::Camera;
use rusty::scene::{MeshComponent, Scene, VisualCorrectionComponent};

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(name)
}

fn vc() -> VisualCorrectionComponent {
    VisualCorrectionComponent {
        active: true,
        bloom_active: false,
        bloom_intensity: 1.0,
        bloom_threshold: 0.6,
        exposure: 0.0,
        contrast: 1.0,
        saturation: 1.0,
        ssr_active: false,
        ssr_quality: "High".to_string(),
        ssr_temporal_upsampling: false,
        tonemap: Tonemap::Aces,
        gamma: 2.2,
    }
}

/// Add a screen-filling translucent quad (a thin box) at world `z`, emissive-tinted
/// `color` with `alpha`. Returns nothing; the entity is appended to the scene.
fn add_quad(scene: &mut Scene, name: &str, z: f32, color: [f32; 3], alpha: f32) {
    let id = scene.add_entity(name.to_string());
    let (vertices, indices) = rusty::render::mesh::generate_box(8.0, 8.0, 0.1);
    let mut e = scene.get_entity_mut(id).unwrap();
    e.transform.position = Vec3::new(0.0, 0.0, z);
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
        material: name.to_string(),
    });
    e.visual_correction = Some(vc());
    drop(e);
    scene.materials.insert(
        name.to_string(),
        MaterialAsset {
            base_color: [0.0, 0.0, 0.0],
            emissive: color,
            alpha,
            render_mode: RenderMode::Transparent,
            ..MaterialAsset::default()
        },
    );
}

const RED: [f32; 3] = [1.0, 0.0, 0.0];
const GREEN: [f32; 3] = [0.0, 1.0, 0.0];

/// A scene with a far red quad and a near green quad; `near_first` flips the order the
/// two are inserted (geometry/depth identical, only insertion order differs).
fn scene(near_first: bool) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;
    if near_first {
        add_quad(&mut scene, "near", 2.0, GREEN, 0.6);
        add_quad(&mut scene, "far", 0.0, RED, 0.6);
    } else {
        add_quad(&mut scene, "far", 0.0, RED, 0.6);
        add_quad(&mut scene, "near", 2.0, GREEN, 0.6);
    }
    scene
}

fn center_rgb(path: &std::path::Path) -> Option<[u8; 3]> {
    let img = image::open(path).ok()?.to_rgb8();
    let p = img.get_pixel(img.width() / 2, img.height() / 2);
    Some([p[0], p[1], p[2]])
}

#[test]
fn overlapping_translucent_quads_composite_back_to_front_regardless_of_order() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let p_far_first = tmp("rusty_tsp_far_first.png");
    let p_near_first = tmp("rusty_tsp_near_first.png");

    let c0 = capture(&scene(false), &cam, &p_far_first, 64, 64).expect("capture");
    let c1 = capture(&scene(true), &cam, &p_near_first, 64, 64).expect("capture");
    if !c0 || !c1 {
        eprintln!("[transparent-sort] no GPU/software adapter — skipping visual assertion");
        return;
    }

    let a = center_rgb(&p_far_first).expect("png");
    let b = center_rgb(&p_near_first).expect("png");
    eprintln!("[transparent-sort] far-first={a:?} near-first={b:?}");

    // Order independence: the depth sort, not insertion order, decides compositing, so
    // both insertion orders must yield the same pixel (±1 LSB for rounding).
    for ch in 0..3 {
        let d = (a[ch] as i32 - b[ch] as i32).abs();
        assert!(
            d <= 1,
            "channel {ch} differs by {d}: far-first={a:?} near-first={b:?}"
        );
    }
    // Back-to-front correctness: the near GREEN quad blends over the far RED one, so
    // green dominates red in the composite (a broken/opaque path would not blend).
    assert!(
        a[1] > a[0] + 10,
        "near green must composite over far red (got {a:?})"
    );
}
