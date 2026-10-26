//! The skybox panorama (#995): the sky pass, the forward reflection and post-FX's
//! SSR fallback all map a direction onto the equirect image through one
//! `equirect_uv` (common.wgsl) and sample it through the skybox's own sampler.
//!
//! The panorama is two-tone: red for u < 0.5 (the -z half of the sky), blue for
//! u ≥ 0.5 (the +z half). Looking down -z must show red sky and down +z blue; a
//! polished metal sphere seen from +z reflects the +z sky, blue, with SSR on as
//! well as off. SSR runs only on the High tier, so the SSR shot sets it. Without an
//! adapter the capture returns `Ok(false)` and the test skips.

use std::path::{Path, PathBuf};

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::core::settings::quality::QualityPreset;
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::authoring::{create_entity, default_post_processing, Primitive};
use rusty::scene::{Camera, Scene};

/// Write the two-tone panorama into the test's temp dir and return its path.
fn panorama() -> PathBuf {
    let img = image::RgbaImage::from_fn(64, 32, |x, _| match x < 32 {
        true => image::Rgba([220, 30, 30, 255]),
        false => image::Rgba([30, 30, 220, 255]),
    });
    let path = crate::temp::dir().join("panorama.png");
    img.save(&path).expect("write panorama");
    path
}

fn sky_scene() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = panorama().to_string_lossy().into_owned();
    scene.ambient_intensity = 1.0;
    scene
}

/// [`sky_scene`] plus a white metal sphere at the origin (no lights, so only the
/// reflected sky lights it); with `ssr`, a volume turns High-tier SSR on.
fn metal_scene(ssr: bool) -> Scene {
    let mut scene = sky_scene();
    let id = create_entity(&mut scene, "Sphere", Some(Primitive::Sphere));
    scene.world.transform_mut(id).unwrap().scale = Vec3::splat(3.0);
    let material = MaterialComponent::new("metal".to_string());
    scene.world.set_material(id, Some(material));
    let metal = MaterialAsset {
        base_color: [1.0, 1.0, 1.0],
        metallic: 1.0,
        roughness: 0.2,
        ..MaterialAsset::default()
    };
    scene.materials.insert("metal".to_string(), metal);
    if ssr {
        let volume = create_entity(&mut scene, "Volume", None);
        let mut vc = default_post_processing();
        vc.bloom.active = false;
        vc.ssr.active = true;
        vc.ssr.quality = "High".to_string();
        scene.world.set_post_processing(volume, Some(vc));
        // A volume rides on its camera (#1434): that camera is the view's.
        scene
            .world
            .set_camera(volume, Some(rusty::components::CameraComponent::default()));
    }
    scene
}

/// Mean RGB (0..255) of the central 16x16 pixels.
fn centre_rgb(path: &Path) -> [f64; 3] {
    let img = image::open(path).expect("png").to_rgb8();
    let (cx, cy) = (img.width() / 2, img.height() / 2);
    let mut sum = [0.0; 3];
    for y in cy - 8..cy + 8 {
        for x in cx - 8..cx + 8 {
            let p = img.get_pixel(x, y);
            (0..3).for_each(|c| sum[c] += p[c] as f64);
        }
    }
    sum.map(|s| s / 256.0)
}

/// Capture `scene` from `cam` as `label` on the `quality` tier; `None` when there is
/// no adapter.
fn shoot(label: &str, scene: &Scene, cam: &Camera, quality: QualityPreset) -> Option<[f64; 3]> {
    let path = crate::temp::dir().join(format!("rusty_panorama_{label}.png"));
    let mut host = CaptureHost::new();
    if let Some(renderer) = host.renderer(96, 96) {
        renderer.set_quality(quality);
    }
    if !capture_into(&mut host, scene, cam, &path, 96, 96).expect("capture") {
        eprintln!("[panorama] no GPU/software adapter — skipping visual assertion");
        return None;
    }
    let rgb = centre_rgb(&path);
    eprintln!("[panorama] {label}: {rgb:.1?} ({})", path.display());
    Some(rgb)
}

#[test]
fn the_sky_pass_maps_the_panorama_by_direction() {
    let scene = sky_scene();
    let towards = |yaw| Camera::new(Vec3::ZERO, yaw, 0.0);
    let Some([r, _, b]) = shoot("minus_z", &scene, &towards(-90.0), QualityPreset::Medium) else {
        return;
    };
    assert!(
        r > b + 60.0,
        "-z sky must be the red half (r {r:.1} b {b:.1})"
    );
    let [r, _, b] =
        shoot("plus_z", &scene, &towards(90.0), QualityPreset::Medium).expect("adapter vanished");
    assert!(
        b > r + 60.0,
        "+z sky must be the blue half (r {r:.1} b {b:.1})"
    );
}

fn assert_reflects_blue_sky(label: &str, ssr: bool) {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let quality = if ssr {
        QualityPreset::High
    } else {
        QualityPreset::Medium
    };
    let Some([r, g, b]) = shoot(label, &metal_scene(ssr), &cam, quality) else {
        return;
    };
    assert!(
        b > r + 40.0 && b > g + 40.0,
        "{label}: the sphere must reflect the blue +z sky (rgb {r:.1} {g:.1} {b:.1})"
    );
}

#[test]
fn metal_reflects_the_panorama() {
    assert_reflects_blue_sky("metal", false);
}

#[test]
fn metal_reflects_the_panorama_with_ssr_on() {
    assert_reflects_blue_sky("metal_ssr", true);
}
