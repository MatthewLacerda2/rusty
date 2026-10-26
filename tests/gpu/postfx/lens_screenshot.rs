//! The volume's lens effects (#1439), pixel-tested on lavapipe: the vignette darkens
//! the corners and not the centre, chromatic aberration fringes a white edge near
//! the border and not at the centre, film grain is the same at the same tick and
//! new at the next, and each one at intensity 0 renders byte-identical to off.

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent, PostProcessingComponent};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::Scene;

use super::super::fog::scene::{camera, wall_scene};

/// Render `scene` at `size`², or `None` when no adapter exists.
fn shot(host: &mut CaptureHost, scene: &Scene, name: &str, size: u32) -> Option<image::RgbImage> {
    let path = crate::temp::dir().join(format!("rusty_lens_{name}.png"));
    if !capture_into(host, scene, &camera(), &path, size, size).expect("capture") {
        eprintln!("[lens] no GPU/software adapter — skipping visual assertion");
        return None;
    }
    Some(image::open(&path).expect("png").to_rgb8())
}

/// A mid-grey wall with `edit` applied to its volume.
fn grey(edit: impl FnOnce(&mut PostProcessingComponent)) -> Scene {
    let mut scene = wall_scene(2.0, [0.5, 0.5, 0.5]);
    let id = scene.world.ids_with_post_processing()[0];
    edit(&mut scene.world.post_processing_mut(id).unwrap());
    scene
}

#[test]
fn each_effect_at_zero_intensity_renders_byte_identical_to_off() {
    let mut host = CaptureHost::new();
    let Some(off) = shot(&mut host, &grey(|_| {}), "off", 64) else {
        return;
    };
    let zero = grey(|vc| {
        vc.vignette.active = true;
        vc.vignette.intensity = 0.0;
        vc.chromatic_aberration.active = true;
        vc.chromatic_aberration.intensity = 0.0;
        vc.film_grain.active = true;
        vc.film_grain.intensity = 0.0;
    });
    assert_eq!(shot(&mut host, &zero, "zero", 64).unwrap(), off);
}

#[test]
fn vignette_darkens_the_corners_and_leaves_the_centre() {
    let mut host = CaptureHost::new();
    let Some(off) = shot(&mut host, &grey(|_| {}), "vig_off", 64) else {
        return;
    };
    let on = grey(|vc| {
        vc.vignette.active = true;
        vc.vignette.intensity = 1.0;
    });
    let on = shot(&mut host, &on, "vig_on", 64).unwrap();
    let px = |img: &image::RgbImage, x, y| img.get_pixel(x, y).0[1] as i32;
    assert!(
        (px(&on, 32, 32) - px(&off, 32, 32)).abs() <= 1,
        "centre kept"
    );
    for (x, y) in [(0, 0), (63, 0), (0, 63), (63, 63)] {
        assert!(
            px(&on, x, y) + 40 < px(&off, x, y),
            "corner ({x}, {y}) darkened"
        );
    }
}

/// A black backdrop with a white wall in front whose left edge is at world `x`.
fn edge_scene(x: f32, aberration: f32) -> Scene {
    let mut scene = wall_scene(3.0, [0.0, 0.0, 0.0]);
    let back = scene.world.ids_with_mesh()[0];
    let mesh = scene.world.mesh(back).unwrap().clone();
    let white = scene.add_entity("White".to_string());
    // The box is 400 wide: its left edge is 200 left of its centre.
    scene.world.transform_mut(white).unwrap().position = Vec3::new(200.0 + x, 0.0, 3.0);
    scene.world.set_mesh(white, Some(mesh));
    let material = Some(MaterialComponent::new("white".to_string()));
    scene.world.set_material(white, material);
    let asset = MaterialAsset {
        base_color: [0.0, 0.0, 0.0],
        emissive: [1.0, 1.0, 1.0],
        ..MaterialAsset::default()
    };
    scene.materials.insert("white".to_string(), asset);
    let id = scene.world.ids_with_post_processing()[0];
    let mut vc = scene.world.post_processing_mut(id).unwrap();
    vc.chromatic_aberration.active = aberration > 0.0;
    vc.chromatic_aberration.intensity = aberration;
    drop(vc);
    scene
}

/// Whether the middle row holds a red fringe pixel and a yellow fringe pixel.
fn fringes(img: &image::RgbImage) -> (bool, bool) {
    let y = img.height() / 2;
    let row: Vec<[u8; 3]> = (0..img.width()).map(|x| img.get_pixel(x, y).0).collect();
    let red = row.iter().any(|&[r, g, b]| r > 150 && g < 60 && b < 60);
    let yellow = row.iter().any(|&[r, g, b]| r > 150 && g > 150 && b < 60);
    (red, yellow)
}

#[test]
fn chromatic_aberration_fringes_an_edge_near_the_border_not_the_centre() {
    let mut host = CaptureHost::new();
    // 2 units from the camera, the 45° view is ±0.83 wide: 0.7 is near the right
    // border (uv ≈ 0.92), 0 the centre.
    let Some(plain) = shot(&mut host, &edge_scene(0.7, 0.0), "ca_plain", 256) else {
        return;
    };
    assert_eq!(fringes(&plain), (false, false), "no fringes when off");
    let border = shot(&mut host, &edge_scene(0.7, 1.0), "ca_border", 256).unwrap();
    assert_eq!(fringes(&border), (true, true), "red and yellow fringes");
    let centre = shot(&mut host, &edge_scene(0.0, 1.0), "ca_centre", 256).unwrap();
    assert_eq!(fringes(&centre), (false, false), "a clean centre");
}

#[test]
fn film_grain_is_the_same_at_the_same_tick_and_new_at_the_next() {
    let mut host = CaptureHost::new();
    let at = |host: &mut CaptureHost, time: f32, tag: &str| {
        let mut scene = grey(|vc| {
            vc.film_grain.active = true;
            vc.film_grain.intensity = 0.5;
        });
        scene.shader_time = time;
        shot(host, &scene, tag, 64)
    };
    let Some(a) = at(&mut host, 1.0, "grain_a") else {
        return;
    };
    let b = at(&mut host, 2.0, "grain_b").unwrap();
    let differing = a.pixels().zip(b.pixels()).filter(|(p, q)| p != q).count();
    let total = (a.width() * a.height()) as usize;
    assert!(differing > total / 2, "only {differing}/{total} changed");
    assert_eq!(a, at(&mut host, 1.0, "grain_a2").unwrap(), "same tick");
}
