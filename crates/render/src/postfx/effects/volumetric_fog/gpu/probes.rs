//! The air's ambient comes from the light probes where the scene has them (#1703):
//! a sealed room baked dark under a bright sky has dark air, where the one
//! scene-wide ambient would light every froxel. The probe grid is filled once and
//! kept while nothing it reads changes.

use glam::Vec3;

use super::fixture::{draw, fog, glow, renderer, room, view, LAMP_AT};
use rusty_scene::{ProbeVolume, Scene};

/// The dark room under a bright ambient (the sky the sealed room never sees).
fn sealed() -> Scene {
    let mut scene = room(fog());
    scene.ambient_color = Vec3::splat(2.0);
    scene
}

/// A probe grid over the room, every probe holding `l0` as its constant band.
fn probes(l0: f32) -> ProbeVolume {
    let mut probes = ProbeVolume::new();
    probes.fill_grid(Vec3::new(-4.0, 0.0, -10.0), Vec3::new(4.0, 4.0, 0.0), 2.0);
    for p in probes.probes_mut() {
        p.sh.coeffs[0] = [l0; 3];
    }
    probes
}

#[test]
fn gpu_a_sealed_room_s_air_is_as_dark_as_its_probes() {
    let Some(mut r) = renderer() else { return };
    let mut read = |scene: &Scene| {
        let mut v = view(&r);
        let mut last = Vec::new();
        for _ in 0..4 {
            last = draw(&mut r, &mut v, scene);
        }
        (
            glow(&last, LAMP_AT),
            r.post_passes.volumetric_fog.probes.fills,
        )
    };
    let mut scene = sealed();
    let (ambient, _) = read(&scene);
    scene.probes = probes(0.0);
    let (dark, fills) = read(&scene);
    // L0 × Y00 is the probe's average radiance: 2.0 / 0.2821 reads as the ambient.
    scene.probes = probes(2.0 / 0.282_094_8 * 0.625);
    let (bright, refilled) = read(&scene);
    eprintln!("[volumetric] air: scene ambient {ambient}, dark probes {dark}, bright {bright}");
    assert!(ambient > 1.0, "the scene ambient lights the air: {ambient}");
    assert!(
        dark < ambient * 0.1,
        "dark probes, dark air: {dark} vs {ambient}"
    );
    assert!(
        bright > ambient * 0.5,
        "bright probes light it again: {bright}"
    );
    assert_eq!(fills, 1, "filled once over four frames");
    assert_eq!(refilled, 2, "and again when the probes changed");
}
