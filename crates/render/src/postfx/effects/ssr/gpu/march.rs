//! Where the march places a hit, and what its walk finds (#2228, `ssr_march.wgsl`'s
//! `clear`): a reflected edge lands within a texel of its mirror image, and a ray
//! skimming a mirror floor finds a rod 1 cm tall in every column, where a probe at
//! each step's end alone found it in some.

use glam::{Vec2, Vec3};

use super::fixture::{paint, place, tuned, MIRROR, RED};
use super::hits::Hits;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

const SIZE: (u32, u32) = (1280, 720);

/// A green block 0.5 m tall with a red one on it, their faces 4.5 m from the eye on the
/// fixture's mirror floor, the eye 1.5 m up looking 15° down.
fn blocks() -> (Scene, Camera) {
    let (mut s, _) = tuned(true, MIRROR, RED, |_| {});
    let cube = s.find_entity_by_name("Cube").unwrap();
    place(
        &mut s,
        cube,
        Vec3::new(0.0, 0.75, -3.0),
        Vec3::new(1.0, 0.5, 1.0),
    );
    let low = create_entity(&mut s, "Low", Some(Primitive::Box));
    place(
        &mut s,
        low,
        Vec3::new(0.0, 0.25, -3.0),
        Vec3::new(1.0, 0.5, 1.0),
    );
    paint(&mut s, low, "low", ([0.1, 0.9, 0.1], 0.0, 1.0));
    (s, Camera::new(Vec3::new(0.0, 1.5, 2.0), -90.0, -15.0))
}

/// Where `p` lands through `cam`, in pixels.
fn pixel(cam: &Camera, p: Vec3) -> Vec2 {
    let clip = cam.build_view_projection(SIZE.0 as f32 / SIZE.1 as f32) * p.extend(1.0);
    let ndc: Vec2 = clip.truncate().truncate() / clip.w;
    Vec2::new(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * Vec2::new(SIZE.0 as f32, SIZE.1 as f32)
}

/// The most a reflected edge may lie from its mirror image's, in rows: the
/// image's own rasterisation and a refinement placed within a texel. Measured
/// (lavapipe): 0.2, as on `main`.
const EDGE: f32 = 1.0;

#[test]
fn gpu_a_reflected_edge_lands_within_a_texel_of_its_image() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(SIZE.0, SIZE.1) else {
        return;
    };
    let (scene, cam) = blocks();
    let hits = Hits::of(&mut r, &scene, &cam, SIZE);
    let (left, right) = (
        pixel(&cam, Vec3::new(-0.4, 0.0, -2.5)),
        pixel(&cam, Vec3::new(0.4, 0.0, -2.5)),
    );
    // A ray leaves its surface 2 cm off it (`postfx/ssr.wgsl`'s `fs_trace`), so it
    // meets the face 2 cm higher than its mirror line.
    let image = pixel(&cam, Vec3::new(0.0, -0.48, -2.5)).y;
    let mut worst = 0.0f32;
    for x in left.x.ceil() as u32..right.x as u32 {
        // Down the column from the blocks' foot: the green block's image, then the red's.
        let foot = left.y.ceil() as u32 + 1;
        let edge = (foot..SIZE.1)
            .find(|&y| hits.is(x, y, 0))
            .expect("the red block reflects");
        worst = worst.max((edge as f32 - image).abs());
    }
    eprintln!("[ssr] the blocks' reflected edge: {worst:.1} rows at worst from its image (row {image:.1})");
    assert!(
        worst <= EDGE,
        "a reflected edge lands {worst:.1} rows from its image"
    );
}

/// A green rod 1 cm tall and 4 cm deep, 0.3 m over the fixture's mirror floor
/// and 8 m out, under a black sky, the eye 12 cm up: the floor's rays skim it at a
/// grazing angle for tens of texels before they reach the rod's height. On
/// `main` 266 of its 434 columns reflected it (lavapipe).
fn skim() -> (Scene, Camera) {
    let (mut s, _) = tuned(true, MIRROR, RED, |_| {});
    crate::test_gpu::black_sky(&mut s);
    let cube = s.find_entity_by_name("Cube").unwrap();
    place(&mut s, cube, Vec3::new(50.0, 0.5, -6.0), Vec3::ONE);
    let rod = create_entity(&mut s, "Rod", Some(Primitive::Box));
    place(
        &mut s,
        rod,
        Vec3::new(0.0, 0.3, -8.0),
        Vec3::new(6.0, 0.01, 0.04),
    );
    paint(&mut s, rod, "rod", ([0.1, 0.9, 0.1], 0.0, 1.0));
    (s, Camera::new(Vec3::new(0.0, 0.12, 0.0), -90.0, 0.0))
}

#[test]
fn gpu_a_ray_skimming_a_floor_finds_a_thin_rod_in_every_column() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(SIZE.0, SIZE.1) else {
        return;
    };
    let (scene, cam) = skim();
    let hits = Hits::of(&mut r, &scene, &cam, SIZE);
    let (left, right) = (
        pixel(&cam, Vec3::new(-2.0, 0.0, -8.0)),
        pixel(&cam, Vec3::new(2.0, 0.0, -8.0)),
    );
    let columns = (left.x as u32..right.x as u32)
        .filter(|&x| (0..SIZE.1).any(|y| hits.is(x, y, 1)))
        .count();
    let all = (right.x - left.x) as usize;
    eprintln!("[ssr] a rod over a mirror floor at grazing: {columns} of {all} columns reflect it");
    assert!(
        columns >= all,
        "a skimming ray misses a thin rod: {columns} of {all} columns"
    );
}
