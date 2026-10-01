//! Shared fixture for the scene-fog pixel tests (#437): a flat, light-independent
//! wall and a neutral post-FX volume, so a pixel's colour is exactly the fogged
//! surface colour, sRGB-encoded. Zero ambient + no lights + black albedo leaves a
//! surface's lit colour equal to its `emissive`; tonemap `None` and neutral grading
//! pass it through, so a test can predict the pixel from the fog formula.

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent, Tonemap};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::{Camera, MeshComponent, Scene, VisualCorrectionComponent};

/// The camera: at `z = 5`, looking down −Z.
pub fn camera() -> Camera {
    Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0)
}

/// A scene with a big emissive `color` wall whose centre is `distance` in front of
/// the camera, plus a neutral volume. Fog is left off.
pub fn wall_scene(distance: f32, color: [f32; 3]) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;
    let id = scene.add_entity("Wall".to_string());
    let (vertices, indices) = rusty::components::mesh::primitives::generate_box(400.0, 400.0, 0.1);
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 0.0, 5.0 - distance);
    scene.world.set_mesh(
        id,
        Some(MeshComponent {
            primitive_type: "Box".to_string(),
            asset_ref: None,
            vertices,
            indices,
            bind_palette: Vec::new(),
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            is_dirty: rusty::scene::DirtyFlag::new(true),
        }),
    );
    let material = "wall".to_string();
    scene.world.set_material(
        id,
        Some(MaterialComponent {
            material: material.clone(),
        }),
    );
    scene.materials.insert(
        material,
        MaterialAsset {
            base_color: [0.0, 0.0, 0.0],
            emissive: color,
            ..MaterialAsset::default()
        },
    );
    scene
        .world
        .set_visual_correction(id, Some(neutral_volume()));
    scene
}

fn neutral_volume() -> VisualCorrectionComponent {
    VisualCorrectionComponent {
        active: true,
        bloom_active: false,
        bloom_intensity: 0.0,
        bloom_threshold: 1.0,
        exposure: 0.0,
        contrast: 1.0,
        saturation: 1.0,
        ssr_active: false,
        ssr_quality: "Low".to_string(),
        ssr_temporal_upsampling: false,
        tonemap: Tonemap::None,
        gamma: 1.0,
        shadows: Default::default(),
        // Neutral means no AO either: the fog tests read exact surface colours.
        ssao: rusty::components::SsaoSettings {
            active: false,
            ..Default::default()
        },
        custom_effects: Vec::new(),
    }
}

/// Render `scene`, or `None` when no adapter exists.
pub fn shot(host: &mut CaptureHost, scene: &Scene, name: &str) -> Option<image::RgbImage> {
    let path = crate::temp::dir().join(format!("rusty_fog_{name}.png"));
    if !capture_into(host, scene, &camera(), &path, 64, 64).expect("capture") {
        eprintln!("[fog] no GPU/software adapter — skipping visual assertion");
        return None;
    }
    Some(image::open(&path).expect("png").to_rgb8())
}

/// Render `scene` and return its centre pixel, or `None` when no adapter exists.
pub fn centre(host: &mut CaptureHost, scene: &Scene, name: &str) -> Option<[u8; 3]> {
    let img = shot(host, scene, name)?;
    Some(img.get_pixel(img.width() / 2, img.height() / 2).0)
}

/// Linear `[0, 1]` → 8-bit sRGB, what the sRGB target stores.
pub fn srgb8(linear: f32) -> i32 {
    let c = linear.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as i32
}

/// Assert two pixels match within `tol` per channel.
pub fn assert_close(got: [u8; 3], want: [i32; 3], tol: i32, what: &str) {
    for ch in 0..3 {
        let d = (got[ch] as i32 - want[ch]).abs();
        assert!(
            d <= tol,
            "{what}: channel {ch} off by {d} (got {got:?}, want {want:?})"
        );
    }
}
