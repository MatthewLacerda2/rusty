use glam::Vec3;

use super::super::fixture::frame;
use super::holders_gpu_tests::image;
use super::maps_gpu_tests::{png, textured};
use rusty_asset::MapRef;
use rusty_components::particle::{Particle, ParticleEmitterComponent};
use rusty_scene::Scene;
use rusty_test_support::panorama::fixture as panorama;

#[test]
fn gpu_a_scene_opened_on_its_bound_sky_keeps_it_while_held() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let sky = panorama("sky.png", [60, 100, 220], [160, 110, 60], 40);
    let other = panorama("other.png", [220, 60, 60], [60, 160, 60], 40);
    let mut z = textured(&[]);
    z.skybox_path = sky.clone();
    frame(&mut r, &z);
    let mut edit = textured(&[]);
    edit.skybox_path = sky;
    frame(&mut r, &edit);
    edit.set_playing(true);
    frame(&mut r, &edit);
    let mut level = textured(&[]);
    level.skybox_path = other;
    level.set_playing(true);
    frame(&mut r, &level);
    edit.set_playing(false);
    let (_, stop) = frame(&mut r, &edit);
    assert_eq!(stop.uploads.count, 0, "Stop decodes the sky again");
}

fn menu_and_smoke(menu: &MapRef, smoke: &MapRef) -> (Scene, u32, u32) {
    let mut scene = textured(&[]);
    let pause = image(&mut scene, menu);
    let emitter = scene.add_entity("Smoke".to_string());
    let idle = ParticleEmitterComponent {
        texture: Some(smoke.clone()),
        ..Default::default()
    };
    scene.world.set_particles(emitter, Some(idle));
    (scene, pause, emitter)
}

fn emit(scene: &mut Scene, emitter: u32, alive: bool) {
    let at = Vec3::new(0.0, 0.5, -4.0);
    let live = alive.then(|| Particle::at(at, 0.5, [1.0; 4], 99.0));
    let particles = &mut scene
        .world
        .particles_mut(emitter)
        .unwrap()
        .runtime
        .particles;
    *particles = live.into_iter().collect();
}

#[test]
fn gpu_the_held_scenes_hidden_image_and_idle_emitter_keep_their_maps() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (menu, smoke) = (png("menu", [200, 0, 0]), png("smoke", [0; 3]));
    let (mut before, _, puff) = menu_and_smoke(&menu, &smoke);
    emit(&mut before, puff, true);
    frame(&mut r, &before);
    // The edit scene opened next has its pause menu hidden and its emitter idle:
    // it never draws either, so only a walk of it notes them.
    let (mut edit, pause, emitter) = menu_and_smoke(&menu, &smoke);
    edit.world.set_active(pause, false);
    frame(&mut r, &edit);
    edit.set_playing(true);
    frame(&mut r, &edit);
    let mut level = textured(&[&png("level", [0, 0, 200])]);
    level.set_playing(true);
    frame(&mut r, &level);
    edit.set_playing(false);
    let (_, stop) = frame(&mut r, &edit);
    assert_eq!(stop.uploads.count, 0, "Stop reads nothing again");
    edit.world.set_active(pause, true);
    emit(&mut edit, emitter, true);
    let (_, shown) = frame(&mut r, &edit);
    assert_eq!(
        shown.uploads.count, 0,
        "showing the menu and emitting read nothing"
    );
}
