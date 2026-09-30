//! Shared fixture for the particle render-mode pixel tests (#440): the fog tests'
//! black, light-free wall 20 m away, with one emitter holding hand-placed particles.
//! A sprite then shows its tint alone, so a test can read coverage off a pixel.

use glam::Vec3;
use rusty::components::particle::{Particle, ParticleEmitterComponent};
use rusty::components::ParticleRender;
use rusty::dev::capture::CaptureHost;
use rusty::scene::Scene;

use super::fog_scene::{shot, wall_scene};

pub const RED: [f32; 4] = [1.0, 0.1, 0.1, 1.0];

/// An 8 × 4 PNG in the temp dir, left half red and right half green: a two-frame
/// (2 × 1) flipbook. Returns its path.
pub fn sheet_sprite() -> String {
    let path = std::env::temp_dir().join("rusty_particle_sheet.png");
    let img = image::RgbaImage::from_fn(8, 4, |x, _| {
        if x < 4 {
            image::Rgba([255, 0, 0, 255])
        } else {
            image::Rgba([0, 255, 0, 255])
        }
    });
    img.save(&path).expect("write sheet");
    path.to_string_lossy().into_owned()
}

/// A plain white sprite, so a particle's colour is its tint alone.
pub fn white_sprite() -> String {
    let path = std::env::temp_dir().join("rusty_particle_white.png");
    image::RgbaImage::from_pixel(4, 4, image::Rgba([255; 4]))
        .save(&path)
        .expect("write sprite");
    path.to_string_lossy().into_owned()
}

/// The wall scene plus one emitter drawing `particles` with `render`.
pub fn scene_with(render: ParticleRender, particles: Vec<Particle>) -> (Scene, u32) {
    let mut scene = wall_scene(20.0, [0.0, 0.0, 0.0]);
    let id = scene.add_entity("Emitter".to_string());
    let mut emitter = ParticleEmitterComponent {
        texture: Some(white_sprite()),
        render,
        ..Default::default()
    };
    emitter.runtime.particles = particles;
    scene.world.set_particles(id, Some(emitter));
    (scene, id)
}

/// One particle at `at` (the camera sits at z = 5 looking down −Z).
pub fn one(at: Vec3, size: f32) -> Vec<Particle> {
    vec![Particle::at(at, size, RED, 10.0)]
}

/// Render `scene` and read pixel `(x, y)` of the 64 × 64 shot; `None` without an
/// adapter.
pub fn pixel(host: &mut CaptureHost, scene: &Scene, name: &str, x: u32, y: u32) -> Option<[u8; 3]> {
    Some(shot(host, scene, name)?.get_pixel(x, y).0)
}

/// Whether a pixel shows the red particle rather than the black wall.
pub fn is_red(px: [u8; 3]) -> bool {
    px[0] > 120 && px[1] < 90
}
