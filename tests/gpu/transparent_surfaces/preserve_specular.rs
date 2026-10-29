//! Preserve specular (#1523, URP's *Preserve Specular Lighting*): a nearly clear
//! (alpha 0.1), mirror-smooth pane over a black floor keeps its reflection whole when
//! it is on, and fades it with the alpha when it is off.

use glam::Vec3;
use rusty::components::MaterialAsset;
use rusty::scene::Camera;

use super::{glass, mean_in, shot, slab, stage, SIZE};

/// The pane over a black floor, seen at a low angle, with `preserve` set.
fn pane(preserve: bool) -> rusty::scene::Scene {
    let mut scene = stage(-1.2);
    let floor = MaterialAsset {
        base_color: [0.0; 3],
        roughness: 1.0,
        ..MaterialAsset::default()
    };
    slab(
        &mut scene,
        "Floor",
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(40.0, 0.1, 40.0),
        floor,
    );
    let water = MaterialAsset {
        roughness: 0.05,
        preserve_specular: preserve,
        ..glass([0.0; 3], 0.1)
    };
    slab(
        &mut scene,
        "Water",
        Vec3::ZERO,
        Vec3::new(40.0, 0.01, 40.0),
        water,
    );
    scene
}

#[test]
fn preserve_specular_keeps_the_reflection_of_a_nearly_clear_pane() {
    let cam = Camera::new(Vec3::new(0.0, 1.0, 6.0), -90.0, -15.0);
    let (Some(on), Some(off)) = (
        shot(&pane(true), &cam, "spec_on"),
        shot(&pane(false), &cam, "spec_off"),
    ) else {
        return;
    };
    // The lower half of the frame is the pane.
    let window = ((0, SIZE), (SIZE / 2, SIZE));
    let (on, off) = (
        mean_in(&on, window.0, window.1),
        mean_in(&off, window.0, window.1),
    );
    eprintln!("[1523] pane brightness: preserve {on:.1}, faded {off:.1}");
    assert!(
        on > off * 2.0 + 4.0,
        "the reflection stays whole: {on:.1} vs {off:.1}"
    );
}
