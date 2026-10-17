//! The skybox panorama does not wrap in V (#1037): v = θ/π runs zenith (0) to nadir
//! (1), so its sampler clamps V while U still wraps at the φ = ±π seam.
//!
//! The panorama is 8x4: the top two rows red, the bottom two blue. At that size half
//! a texel is 1/8 of V (22.5° of θ), so a repeating V would blend the zenith 50/50
//! with the nadir across the whole view. Looking straight up must show pure red,
//! straight down pure blue. Without an adapter the capture returns `Ok(false)` and
//! the test skips.

use glam::Vec3;
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::{Camera, Scene};

fn pole_scene() -> Scene {
    let img = image::RgbaImage::from_fn(8, 4, |_, y| match y < 2 {
        true => image::Rgba([220, 30, 30, 255]),
        false => image::Rgba([30, 30, 220, 255]),
    });
    let path = crate::temp::dir().join("pole_panorama.png");
    img.save(&path).expect("write panorama");
    let mut scene = Scene::new();
    scene.skybox_path = path.to_string_lossy().into_owned();
    scene
}

/// The centre pixel's RGB looking along `pitch` degrees; `None` without an adapter.
fn centre(label: &str, pitch: f32) -> Option<[f64; 3]> {
    let path = crate::temp::dir().join(format!("rusty_pole_{label}.png"));
    let cam = Camera::new(Vec3::ZERO, 0.0, pitch);
    if !capture_into(&mut CaptureHost::new(), &pole_scene(), &cam, &path, 64, 64).expect("capture")
    {
        eprintln!("[panorama pole] no GPU/software adapter — skipping visual assertion");
        return None;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    let p = img.get_pixel(32, 32);
    let rgb = [p[0], p[1], p[2]].map(f64::from);
    eprintln!("[panorama pole] {label}: {rgb:?} ({})", path.display());
    Some(rgb)
}

#[test]
fn the_zenith_is_the_top_row_unblended() {
    let Some([r, _, b]) = centre("zenith", 89.9) else {
        return;
    };
    assert!(
        r > 200.0 && b < 50.0,
        "the zenith must be the top row's red, not blended with the nadir (r {r} b {b})"
    );
}

#[test]
fn the_nadir_is_the_bottom_row_unblended() {
    let Some([r, _, b]) = centre("nadir", -89.9) else {
        return;
    };
    assert!(
        b > 200.0 && r < 50.0,
        "the nadir must be the bottom row's blue, not blended with the zenith (r {r} b {b})"
    );
}
