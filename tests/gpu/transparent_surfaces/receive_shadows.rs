//! Receive shadows (#1523, URP's *Receive Shadows*): a roof's shadow darkens a
//! transparent plane under it, unless the plane's material turns the toggle off.

use glam::Vec3;
use rusty::scene::Camera;

use super::{darker, glass, shot, slab, stage};

/// A pale transparent plane under a sun high overhead, a roof above it when `roof`
/// (out of the camera's view), the plane receiving shadows when `receive`.
fn plane(roof: bool, receive: bool) -> rusty::scene::Scene {
    let mut scene = stage(-1.45);
    let water = rusty::components::MaterialAsset {
        receive_shadows: receive,
        ..glass([0.9; 3], 0.8)
    };
    slab(
        &mut scene,
        "Water",
        Vec3::ZERO,
        Vec3::new(20.0, 0.01, 20.0),
        water,
    );
    if roof {
        let concrete = rusty::components::MaterialAsset::default();
        slab(
            &mut scene,
            "Roof",
            Vec3::new(0.0, 6.0, 0.0),
            Vec3::new(3.0, 0.2, 3.0),
            concrete,
        );
    }
    scene
}

#[test]
fn a_plane_that_receives_no_shadows_stays_lit_under_a_caster() {
    let cam = Camera::new(Vec3::new(0.0, 4.0, 4.0), -90.0, -55.0);
    let Some(open) = shot(&plane(false, true), &cam, "rs_open") else {
        return;
    };
    let shadowed = shot(&plane(true, true), &cam, "rs_on").unwrap();
    let unshadowed = shot(&plane(true, false), &cam, "rs_off").unwrap();
    let (on, off) = (darker(&shadowed, &open), darker(&unshadowed, &open));
    eprintln!("[1523] darkened by the roof: receiving {on}, not receiving {off}");
    assert!(on > 100, "the roof shades a receiving plane: {on}");
    assert!(off < 5, "a non-receiving plane stays lit: {off}");
}
