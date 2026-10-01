//! Data maps sample their texels raw, not sRGB-decoded (#647). Each check renders
//! a material driven by a mid-grey (128) map against the same material with the
//! value as a scalar instead, and asserts the two frames match: decoded as sRGB,
//! 128 reads ≈ 0.22, not ≈ 0.5, and the frames part visibly.
//!   - Metallic: ambient diffuse is gated by `(1 - metallic)`, a strong signal.
//!   - Roughness: a light grazing the plane makes the highlight's spread visible.
//!   - Normal: the flat (128,128,255) map leaves N unperturbed — the frame matches
//!     the unmapped one. Decoded, its XY bend N by ≈ 50°.
//!
//! No GPU/software adapter: `capture` returns `Ok(false)` and the test passes
//! without asserting (matching `material_maps_screenshot.rs`).

use glam::Vec3;
use rusty::components::{LightComponent, LightType, MaterialAsset};
use rusty::dev::screenshot::capture;
use rusty::scene::{Camera, Scene};

/// The mean absolute per-channel difference (0–255) two frames may differ by and
/// still read as the same shading: the 128 → 0.502 quantisation, nothing more.
const SAME: f64 = 1.5;

/// Write a 2x2 solid-RGB PNG (alpha 255) and return its path.
fn png(name: &str, rgb: [u8; 3]) -> String {
    let path = crate::temp::dir().join(name);
    let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
    img.save(&path).expect("write png");
    path.to_string_lossy().into_owned()
}

/// An ambient- and lamp-lit up-facing plane carrying `material`; the lamp's
/// highlight is what roughness spreads.
fn scene(material: MaterialAsset) -> Scene {
    let mut s = super::normal_emissive_maps_screenshot::scene(material);
    let lamp = s.add_entity("Lamp".to_string());
    s.world.transform_mut(lamp).expect("transform").position = Vec3::new(1.0, 3.0, 0.0);
    let light = LightComponent {
        light_type: LightType::Point,
        color: Vec3::ONE,
        intensity: 20.0,
        range: 30.0,
        inner_cone: 0.0,
        outer_cone: 0.0,
    };
    s.world.set_light(lamp, Some(light));
    s
}

/// Render `scene` to `name`; `None` when no adapter exists.
fn frame(scene: &Scene, name: &str) -> Option<image::RgbImage> {
    let cam = Camera::new(Vec3::new(0.0, 12.0, 0.0), 0.0, -90.0);
    let path = crate::temp::dir().join(name);
    capture(scene, &cam, &path, 96, 96)
        .expect("capture")
        .then_some(())?;
    Some(image::open(&path).expect("open").to_rgb8())
}

/// Mean absolute per-channel difference between two frames.
fn diff(a: &image::RgbImage, b: &image::RgbImage) -> f64 {
    let sum: u64 = a
        .as_raw()
        .iter()
        .zip(b.as_raw())
        .map(|(x, y)| x.abs_diff(*y) as u64)
        .sum();
    sum as f64 / a.as_raw().len() as f64
}

/// Assert `mapped` shades like `scalar` (and, as a control, unlike `decoded`).
fn assert_same(what: &str, mapped: MaterialAsset, scalar: MaterialAsset, decoded: MaterialAsset) {
    let frames =
        [mapped, scalar, decoded].map(|m| frame(&scene(m), &format!("rusty_647_{what}.png")));
    let [Some(mapped), Some(scalar), Some(decoded)] = frames else {
        eprintln!("[linear-data-maps] no GPU adapter — skipping {what}");
        return;
    };
    let (same, control) = (diff(&mapped, &scalar), diff(&decoded, &scalar));
    eprintln!(
        "[linear-data-maps] {what}: map vs scalar {same:.3}, sRGB-decoded vs scalar {control:.3}"
    );
    assert!(
        control > SAME,
        "{what}: the control must be visible ({control:.3})"
    );
    assert!(
        same < SAME,
        "{what}: a 128 map must shade like 0.5 ({same:.3})"
    );
}

#[test]
fn a_mid_grey_metallic_map_reads_one_half() {
    let map = png("rusty_647_metal.png", [128, 128, 128]);
    let mat = |metallic, metallic_map| MaterialAsset {
        base_color: [0.9, 0.9, 0.9],
        metallic,
        metallic_map,
        ..MaterialAsset::default()
    };
    assert_same(
        "metallic",
        mat(1.0, Some(map)),
        mat(0.5, None),
        mat(0.216, None),
    );
}

#[test]
fn a_mid_grey_roughness_map_reads_one_half() {
    let map = png("rusty_647_rough.png", [128, 128, 128]);
    // Metallic, so the lamp's highlight — what roughness spreads — is the picture.
    let mat = |roughness, roughness_map| MaterialAsset {
        base_color: [0.9, 0.9, 0.9],
        metallic: 1.0,
        roughness,
        roughness_map,
        ..MaterialAsset::default()
    };
    assert_same(
        "roughness",
        mat(1.0, Some(map)),
        mat(0.5, None),
        mat(0.216, None),
    );
}

#[test]
fn a_flat_normal_map_leaves_the_normal_unperturbed() {
    let flat = png("rusty_647_flat.png", [128, 128, 255]);
    let bent = png("rusty_647_bent.png", [55, 55, 255]); // 128 → 0.216, re-encoded raw
    let mat = |normal_map| MaterialAsset {
        base_color: [0.9, 0.9, 0.9],
        normal_map,
        ..MaterialAsset::default()
    };
    assert_same("normal", mat(Some(flat)), mat(None), mat(Some(bent)));
}
