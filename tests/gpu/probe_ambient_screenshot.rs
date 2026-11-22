//! Probe ambient and flat ambient agree: `Sh9::eval` reads back irradiance E and a
//! Lambert surface reflects albedo / π × E, as the flat ambient and a lightmap
//! already give; a probe-lit floor was once π× brighter. The floor faces up because
//! the flat ambient equals the sky's L only at N = +Y. Skips without an adapter.

use glam::Vec3;
use rusty::dev::screenshot::capture;
use rusty::edit::authoring::{primitive_mesh_component, Primitive};
use rusty::scene::{Camera, Scene, Sh9};

const SIZE: u32 = 32;
/// Sky radiance, dim enough that π× it does not clip, so a π× error shows.
const SKY: f32 = 0.25;

fn floor(is_static: bool) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_color = Vec3::ONE;
    scene.ambient_intensity = SKY;
    let id = scene.add_entity("Floor".to_string());
    scene
        .world
        .set_mesh(id, primitive_mesh_component(Primitive::Plane));
    scene.world.set_static(id, is_static);
    scene
}

/// The SH a probe bakes from a uniform sky of radiance `l` (Fibonacci sphere).
fn uniform_sky_sh(l: f32) -> Sh9 {
    const N: usize = 4096;
    let mut sh = Sh9::zero();
    let golden = std::f32::consts::PI * (3.0 - 5.0f32.sqrt());
    for i in 0..N {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / N as f32;
        let r = (1.0 - y * y).sqrt();
        let phi = golden * i as f32;
        let dir = Vec3::new(r * phi.cos(), y, r * phi.sin());
        sh.add_sample(dir, Vec3::splat(l), 4.0 * std::f32::consts::PI / N as f32);
    }
    sh
}

fn centre_level(scene: &Scene, name: &str) -> Option<f64> {
    let camera = Camera::new(Vec3::new(0.0, 2.0, 0.0), 0.0, -89.0);
    let path = crate::temp::dir().join(format!("rusty_gpu_probe_ambient_{name}.png"));
    capture(scene, &camera, &path, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    let img = image::open(&path).unwrap().to_rgb8();
    let (lo, hi) = (SIZE / 2 - 4, SIZE / 2 + 4);
    let block: Vec<u8> = (lo..hi)
        .flat_map(|y| (lo..hi).map(move |x| (x, y)))
        .map(|(x, y)| img.get_pixel(x, y).0[0])
        .collect();
    Some(block.iter().map(|&v| f64::from(v)).sum::<f64>() / block.len() as f64)
}

#[test]
fn a_probe_lit_floor_matches_a_flat_ambient_one_of_the_same_sky() {
    let Some(flat) = centre_level(&floor(true), "flat") else {
        eprintln!("[probe ambient] no GPU/software adapter — skipping");
        return;
    };
    let mut probed = floor(false);
    let probe = probed.probes.add_probe(Vec3::new(0.0, 1.0, 0.0));
    probed.probes.probes_mut()[probe].sh = uniform_sky_sh(SKY);
    let lit = centre_level(&probed, "probe").unwrap();
    eprintln!("[probe ambient] flat={flat:.2} probe={lit:.2}");
    assert!(flat > 20.0, "the flat ambient lights the floor: {flat:.2}");
    assert!(
        (lit - flat).abs() <= 3.0,
        "same sky, same brightness: probe {lit:.2} vs flat {flat:.2}"
    );
}
