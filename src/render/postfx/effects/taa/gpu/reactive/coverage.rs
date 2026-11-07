//! A fixed reactivity follows a sprite's coverage (#1845): a `Full` sprite's
//! transparent corners leave the surface behind them as an `Off` one's do, and only
//! where it shows does it make the pixel reactive. Also the muzzle flash the
//! viewmodel's test draws over its gun (`viewmodel`).

use glam::Vec3;

use super::super::fixture::{diff, eye, frame, pixel, project, renderer, taa_scene, view};
use super::super::wall::noisy_wall;
use crate::components::particle::{Particle, ParticleEmitterComponent};
use crate::components::TaaReactive;

/// A 16×16 white sprite in the temp dir, opaque in its middle 4×4 and clear
/// around it, as a muzzle flash's texture is.
fn cored_sprite() -> String {
    let path = crate::test_temp::dir().join("rusty_1845_taa_cored.png");
    let core = 6..10;
    let img = image::RgbaImage::from_fn(16, 16, |x, y| {
        let opaque = core.contains(&x) && core.contains(&y);
        image::Rgba([255, 255, 255, if opaque { 255 } else { 0 }])
    });
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

/// One still sprite `size` across at `at`, reactivity `reactive`, on a new entity.
pub(super) fn flash(
    scene: &mut crate::scene::Scene,
    at: Vec3,
    size: f32,
    reactive: TaaReactive,
) -> u32 {
    let id = scene.add_entity("Flash".to_string());
    let mut emitter = ParticleEmitterComponent {
        texture: Some(cored_sprite().into()),
        ..Default::default()
    };
    emitter.render.reactive = reactive;
    let tint = [1.0, 0.8, 0.5, 0.6];
    emitter.runtime.particles = vec![Particle::at(at, size, tint, 99.0)];
    scene.world.set_particles(id, Some(emitter));
    id
}

const AT: Vec3 = Vec3::new(0.0, 0.0, -4.0);

/// Eight frames of a still camera on a noise wall behind a cored sprite of
/// `reactive`, so the jitter's sub-pixel shift shows the frame weight; the last.
fn sprite_over_wall(reactive: TaaReactive) -> Option<Vec<u8>> {
    let mut r = renderer()?;
    let (mut scene, _) = taa_scene(true, false);
    noisy_wall(&mut scene, -6.0);
    flash(&mut scene, AT, 2.0, reactive);
    let mut v = view(&r);
    let mut last = Vec::new();
    for _ in 0..8 {
        last = frame(&mut r, &mut v, &scene, &eye(0.0));
    }
    Some(last)
}

#[test]
fn gpu_a_full_sprites_clear_corners_leave_the_wall_alone() {
    let (Some(full), Some(off)) = (
        sprite_over_wall(TaaReactive::Full),
        sprite_over_wall(TaaReactive::Off),
    ) else {
        return;
    };
    let (cx, cy) = pixel(project(&eye(0.0), AT));
    let near = |d: u32| {
        let span = move |c: u32| c.saturating_sub(d)..=c + d;
        span(cx).flat_map(move |x| span(cy).map(move |y| (x, y)))
    };
    // The sprite's quad reaches about 10 pixels out; its core about 3.
    let ring = near(9).filter(|&(x, y)| x.abs_diff(cx).max(y.abs_diff(cy)) >= 7);
    let corners = ring.map(|p| diff(&full, &off, p)).max().unwrap();
    let core = near(2).map(|p| diff(&full, &off, p)).max().unwrap();
    eprintln!("[taa 1845] Full vs Off: corners {corners}, core {core}");
    assert_eq!(
        corners, 0,
        "a Full sprite's clear corners made the wall reactive"
    );
    assert!(core > 4, "a Full sprite's core changed nothing: {core}");
}
