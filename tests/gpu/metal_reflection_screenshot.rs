//! Environment reflections on metal (#718). A fully metallic surface has no diffuse
//! term, so without the environment reflection it renders black. That term used to run
//! only behind a camera's SSR toggle; now it always applies, Unity's way, and with no
//! skybox panorama it reflects the procedural sky (#256) the sky pass draws.
//!
//! The scene has **no lights** and a blue ambient tint, so the only thing that can
//! light a white metal sphere is the reflected sky: its centre must be well above
//! black and blue-dominant like that sky. With SSR on, the same must still hold.
//! Without an adapter `capture` returns `Ok(false)` and the test skips.

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{create_entity, default_visual_correction, Primitive};
use rusty::scene::{Camera, Scene};

/// A polished white metal sphere (metallic 1.0, roughness 0.2), no lights, a blue sky;
/// with `ssr`, a VisualCorrection volume turns SSR on.
fn scene(ssr: bool) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_color = Vec3::new(0.2, 0.45, 1.0);
    scene.ambient_intensity = 1.0;
    let id = create_entity(&mut scene, "Sphere", Some(Primitive::Sphere));
    scene.world.transform_mut(id).unwrap().scale = Vec3::splat(3.0);
    let material = MaterialComponent {
        material: "metal".to_string(),
    };
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
        let mut vc = default_visual_correction();
        vc.bloom_active = false;
        vc.ssr_active = true;
        scene.world.set_visual_correction(volume, Some(vc));
    }
    scene
}

/// Mean RGB (0..255) of the central 16x16 pixels, which the sphere fills.
fn centre_rgb(path: &std::path::Path) -> [f64; 3] {
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

fn assert_reflects_sky(label: &str, ssr: bool) {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let path = crate::temp::dir().join(format!("rusty_metal_{label}.png"));
    if !capture(&scene(ssr), &cam, &path, 96, 96).expect("capture") {
        eprintln!("[metal] no GPU/software adapter — skipping visual assertion");
        return;
    }
    let [r, g, b] = centre_rgb(&path);
    eprintln!("[metal] {label}: r={r:.1} g={g:.1} b={b:.1}");
    let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    assert!(
        luminance > 60.0,
        "{label}: metal sphere renders black (rgb {r:.1} {g:.1} {b:.1})"
    );
    assert!(
        b > r + 10.0,
        "{label}: metal must reflect the blue sky (rgb {r:.1} {g:.1} {b:.1})"
    );
}

#[test]
fn metal_reflects_the_procedural_sky_without_visual_correction() {
    assert_reflects_sky("no_vc", false);
}

#[test]
fn metal_reflects_the_sky_with_ssr_on() {
    assert_reflects_sky("ssr", true);
}
