//! Particle render modes (#440): a stretched spark streaks along its velocity, a
//! horizontal splash lies flat (edge-on at eye height), a vertical sprite stands
//! up, and a flipbook shows the frame its life has reached.

use glam::Vec3;
use rusty::components::{Flipbook, ParticleRender, ParticleRenderMode};
use rusty::dev::capture::CaptureHost;

use super::particle_scene::{is_red, one, pixel, scene_with, sheet_sprite};

fn render(mode: ParticleRenderMode) -> ParticleRender {
    ParticleRender {
        mode,
        ..Default::default()
    }
}

#[test]
fn a_stretched_particle_streaks_along_its_velocity() {
    let mut host = CaptureHost::new();
    let mut stretched = render(ParticleRenderMode::Stretched);
    stretched.speed_scale = 1.0;
    let mut spark = one(Vec3::ZERO, 0.4);
    spark[0].velocity = Vec3::new(6.0, 0.0, 0.0);
    let (moving, _) = scene_with(stretched.clone(), spark);
    let (billboard, _) = scene_with(ParticleRender::default(), one(Vec3::ZERO, 0.4));
    let (Some(streak), Some(dot)) = (
        pixel(&mut host, &moving, "stretched", 46, 32),
        pixel(&mut host, &billboard, "stretched_ref", 46, 32),
    ) else {
        return;
    };
    assert!(is_red(streak), "the streak reaches off-centre: {streak:?}");
    assert!(
        !is_red(dot),
        "a billboard of the same size does not: {dot:?}"
    );
    // Across the streak it stays as thin as the particle's size.
    let Some(above) = pixel(&mut host, &moving, "stretched_above", 46, 20) else {
        return;
    };
    assert!(!is_red(above), "the streak is thin across: {above:?}");
}

#[test]
fn horizontal_lies_flat_and_vertical_stands_up() {
    let mut host = CaptureHost::new();
    let at = Vec3::new(0.0, 0.0, 0.0);
    let (flat, _) = scene_with(render(ParticleRenderMode::Horizontal), one(at, 2.0));
    let (upright, _) = scene_with(render(ParticleRenderMode::Vertical), one(at, 2.0));
    let (Some(edge_on), Some(standing)) = (
        pixel(&mut host, &flat, "horizontal", 32, 20),
        pixel(&mut host, &upright, "vertical", 32, 20),
    ) else {
        return;
    };
    assert!(
        !is_red(edge_on),
        "a flat quad at eye height is edge-on: {edge_on:?}"
    );
    assert!(
        is_red(standing),
        "an upright quad covers above centre: {standing:?}"
    );
}

#[test]
fn a_flipbook_shows_the_frame_its_particle_is_on() {
    let mut host = CaptureHost::new();
    let book = ParticleRender {
        flipbook: Flipbook {
            columns: 2,
            rows: 1,
            cycles: 1.0,
            random_start: false,
        },
        ..Default::default()
    };
    let mut first = one(Vec3::ZERO, 3.0);
    first[0].color = [1.0; 4];
    let mut second = first.clone();
    second[0].start_frame = 1;
    let (mut a, id_a) = scene_with(book.clone(), first);
    let (mut b, id_b) = scene_with(book, second);
    for (scene, id) in [(&mut a, id_a), (&mut b, id_b)] {
        scene.world.particles_mut(id).unwrap().texture = Some(sheet_sprite());
    }
    let (Some(frame0), Some(frame1)) = (
        pixel(&mut host, &a, "flipbook0", 32, 32),
        pixel(&mut host, &b, "flipbook1", 32, 32),
    ) else {
        return;
    };
    assert!(
        frame0[0] > 200 && frame0[1] < 40,
        "frame 0 is the red cell: {frame0:?}"
    );
    assert!(
        frame1[1] > 200 && frame1[0] < 40,
        "frame 1 is the green cell: {frame1:?}"
    );
}
