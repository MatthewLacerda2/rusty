//! #1994: a floor modelled in world coordinates, its origin 20 m away among another
//! room's (red) probes, its geometry under a green probe. It samples the probes at
//! its bounds' centre, so it reads green; at its origin it read the other room's red.

use glam::Vec3;
use rusty::components::mesh::primitives::generate_box;
use rusty::dev::screenshot::capture;
use rusty::scene::{Camera, Scene, Sh9};

use super::{centre_rgb, world_modelled};

/// The SH of a uniform sky of radiance `rgb`, from the six axis directions (their
/// first band cancels, their DC term is the sky's).
fn uniform(rgb: Vec3) -> Sh9 {
    let mut sh = Sh9::zero();
    for dir in [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ] {
        sh.add_sample(dir, rgb, 4.0 * std::f32::consts::PI / 6.0);
    }
    sh
}

#[test]
fn a_world_modelled_mesh_samples_the_probes_at_its_geometry() {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;
    scene.skybox_path = String::new();
    let other_room = Vec3::new(0.0, 0.0, -20.0);
    for (at, rgb) in [(other_room, Vec3::X), (Vec3::ZERO, Vec3::Y)] {
        let probe = scene.probes.add_probe(at + Vec3::Y);
        scene.probes.probes_mut()[probe].sh = uniform(rgb * 0.5);
    }
    let at = (other_room, Vec3::ZERO);
    world_modelled(&mut scene, "Floor", at, generate_box(4.0, 0.1, 4.0));

    let cam = Camera::new(Vec3::new(0.0, 2.0, 0.0), 0.0, -89.0);
    let path = crate::temp::file("rusty_gpu_bounds_centre_probes.png");
    if !capture(&scene, &cam, &path, 32, 32).expect("capture") {
        eprintln!("[bounds-centre probes] no GPU/software adapter — skipping");
        return;
    }
    let [r, g, _] = centre_rgb(&path);
    eprintln!("[bounds-centre probes] centre rgb r={r} g={g}");
    assert!(
        g > r + 20,
        "lit by the probe over its geometry, not its origin's (r={r}, g={g})"
    );
}
