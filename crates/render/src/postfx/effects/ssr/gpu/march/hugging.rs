//! A ray that hugs a surface runs its walk out of rounds (#2228, `ssr_march.wgsl`'s
//! `clear`, `CLEAR_ROUNDS`): rays from a mirror floor climb a dull wall seen at
//! grazing, a few centimetres off it, so nearly every cell under them holds the
//! wall nearer than they are and is probed texel by texel. Out of rounds, the step
//! ends where the walk got to and the next walks on from there; a step taken as
//! clear to its end instead is probed only there, and a thin rod on the wall in
//! the stretch it skipped is missed. The wall's own reflection is counted too:
//! a ray hugging it is behind its texels by more than its object at times, and
//! still lands on it, or on the rod beside it.

use glam::Vec3;

use super::super::fixture::{paint, place, tuned, MIRROR, RED};
use super::hits::Hits;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

/// 1080p: a step there spans 68 texels, past what 32 rounds walk along the wall.
const SIZE: (u32, u32) = (1920, 1080);
const GREEN: usize = 1;

/// A dull blue wall in the plane x = 0 down the -Z axis on the fixture's mirror floor,
/// a green rod 1 cm tall standing 2 cm out of it 0.4 m up, under a black sky; the
/// eye 1 m up, 10 cm from the wall's plane, looking 2° away from it and 5° down.
fn wall_and_rod() -> (Scene, Camera) {
    let (mut s, _) = tuned(true, MIRROR, RED, |_| {});
    crate::test_gpu::black_sky(&mut s);
    let cube = s.find_entity_by_name("Cube").unwrap();
    place(&mut s, cube, Vec3::new(50.0, 0.5, -6.0), Vec3::ONE);
    let wall = create_entity(&mut s, "Wall", Some(Primitive::Box));
    let (at, size) = (Vec3::new(-0.05, 1.5, -10.0), Vec3::new(0.1, 3.0, 20.0));
    place(&mut s, wall, at, size);
    paint(&mut s, wall, "wall", ([0.1, 0.1, 0.9], 0.0, 1.0));
    let rod = create_entity(&mut s, "Rod", Some(Primitive::Box));
    let (at, size) = (Vec3::new(0.01, 0.4, -10.0), Vec3::new(0.02, 0.01, 16.0));
    place(&mut s, rod, at, size);
    paint(&mut s, rod, "rod", ([0.1, 0.9, 0.1], 0.0, 1.0));
    (s, Camera::new(Vec3::new(0.1, 1.0, 0.0), -92.0, -5.0))
}

/// The fewest rows the rod's reflection may span. Measured (lavapipe): 550, and
/// 433 with a ray behind the rod by more than its object taken as past it for good;
/// a step out of rounds taken as clear to its end, 381.
const ROWS: usize = 410;
/// The fewest pixels the wall's and the rod's reflections may hold together.
/// Measured (lavapipe): 13530, 13528 without the grazing allowance (`grazes`), and
/// 12821 with a ray behind the rod taken as past it for good.
const REFLECTION: usize = 13200;
const BLUE: usize = 2;

#[test]
fn gpu_a_ray_hugging_a_wall_past_its_walks_rounds_finds_a_thin_rod() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(SIZE.0, SIZE.1) else {
        return;
    };
    let (scene, cam) = wall_and_rod();
    let hits = Hits::of(&mut r, &scene, &cam, SIZE);
    let rows = hits.rows(GREEN).len();
    let both = hits.count(GREEN) + hits.count(BLUE);
    eprintln!("[ssr] a rod on a wall the rays hug: its reflection spans {rows} rows, the wall's and the rod's {both} px");
    assert!(
        rows >= ROWS,
        "a ray hugging a wall misses a thin rod past its walk's rounds: {rows} rows"
    );
    assert!(
        both >= REFLECTION,
        "a ray hugging a wall loses the wall's reflection: {both} px"
    );
}
