//! A mirror seen head-on (#2228, `ssr_march.wgsl`'s `trace_hit`, `placed`): its
//! rays come back almost along the lines of sight, so a step moves under a texel
//! on screen and spans a long stretch of depth, which no pyramid walk narrows. A
//! thin pole before it is hit from behind (the depth target holds only its front)
//! somewhere in such a step, and its hit is judged give or take the step's depth:
//! by the object's thickness alone, a column of the pole's image goes missing.
//! The rays the mirror sends past the pole's foot pass a few centimetres behind
//! it, further than its 3 cm at the default thickness, on their way to the floor
//! behind it: they keep the floor's hit.

use glam::{Vec2, Vec3};

use super::super::fixture::{paint, place, tuned, MIRROR, RED};
use super::hits::Hits;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

const SIZE: (u32, u32) = (640, 360);
const GREEN: usize = 1;

/// A green pole 2 cm square and 1.5 m tall, 1 m before a mirror wall, on a dull
/// floor, the eye 1 m up and 0.6 m to its side, looking straight at the wall.
fn pole_before_a_mirror() -> (Scene, Camera) {
    let (mut s, _) = tuned(true, ([0.3; 3], 0.0, 1.0), RED, |_| {});
    let cube = s.find_entity_by_name("Cube").unwrap();
    place(&mut s, cube, Vec3::new(50.0, 0.5, -6.0), Vec3::ONE);
    let wall = create_entity(&mut s, "Mirror", Some(Primitive::Box));
    let wall_at = Vec3::new(0.0, 1.5, -4.04);
    place(&mut s, wall, wall_at, Vec3::new(8.0, 3.0, 0.08));
    paint(&mut s, wall, "mirror", MIRROR);
    let id = create_entity(&mut s, "Pole", Some(Primitive::Box));
    let at = Vec3::new(0.0, 0.75, -3.0);
    place(&mut s, id, at, Vec3::new(0.02, 1.5, 0.02));
    paint(&mut s, id, "pole", ([0.1, 0.9, 0.1], 0.0, 1.0));
    (s, Camera::new(Vec3::new(0.6, 1.0, 0.0), -90.0, 0.0))
}

/// Where `p` lands through `cam`, in pixels.
fn pixel(cam: &Camera, p: Vec3) -> Vec2 {
    let clip = cam.build_view_projection(SIZE.0 as f32 / SIZE.1 as f32) * p.extend(1.0);
    let ndc: Vec2 = clip.truncate().truncate() / clip.w;
    Vec2::new(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * Vec2::new(SIZE.0 as f32, SIZE.1 as f32)
}

/// The least share of the pole's mirror image (its rows times its width on
/// screen) its reflection may cover, and the most: its raster edges round out by
/// a pixel, a band past them is a thin thing over-reflected. Measured (lavapipe):
/// 1.16; judged by the object's thickness alone, without the step's depth
/// (`placed`), 0.68, a column of the image lost down its length.
const COVER: (f32, f32) = (0.85, 1.5);

#[test]
fn gpu_a_thin_pole_before_a_mirror_seen_head_on_keeps_its_reflection() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(SIZE.0, SIZE.1) else {
        return;
    };
    let (scene, cam) = pole_before_a_mirror();
    let hits = Hits::of(&mut r, &scene, &cam, SIZE);
    // The pole's mirror image stands 1 m behind the wall, at z = -5.
    let (top, foot) = (
        pixel(&cam, Vec3::new(-0.01, 1.5, -5.0)),
        pixel(&cam, Vec3::new(0.01, 0.0, -5.0)),
    );
    let image = (foot.x - top.x) * (foot.y - top.y);
    let cover = hits.count(GREEN) as f32 / image;
    eprintln!("[ssr] a 2 cm pole before a mirror, head-on: its reflection covers {cover:.2} of its image ({image:.0} px)");
    assert!(
        (COVER.0..=COVER.1).contains(&cover),
        "a thin pole's reflection seen head-on covers {cover:.2} of its image"
    );
}

/// The most mirror pixels between the pole's foot and its image's that may hit
/// nothing. Measured (lavapipe): 0; a ray behind the pole taken as past it for
/// good, which the next refinement collapsed back onto the pole, 25: a dotted line
/// of misses from the foot to the image's foot.
const LOST: usize = 2;

#[test]
fn gpu_a_ray_passing_behind_a_thin_pole_keeps_the_floor_behind_it() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(SIZE.0, SIZE.1) else {
        return;
    };
    let (scene, cam) = pole_before_a_mirror();
    let hits = Hits::of(&mut r, &scene, &cam, SIZE);
    // The pole's foot, the mirror's foot and the image's foot, on screen.
    let pole = pixel(&cam, Vec3::new(0.01, 0.0, -3.0));
    let (foot, image) = (
        pixel(&cam, Vec3::new(0.0, 0.0, -4.0)),
        pixel(&cam, Vec3::new(0.0, 0.0, -5.0)),
    );
    let rows = (image.y.ceil() as u32 + 1)..(foot.y.floor() as u32);
    let columns = (pole.x.ceil() as u32 + 1)..(image.x.floor() as u32);
    let lost = rows
        .flat_map(|y| columns.clone().map(move |x| (x, y)))
        .filter(|&(x, y)| hits.at(x, y).iter().all(|c| *c < 0.01))
        .count();
    eprintln!("[ssr] a 2 cm pole before a mirror, head-on: {lost} pixels of the floor behind its foot hit nothing");
    assert!(
        lost <= LOST,
        "rays passing behind a thin pole lose the floor behind it: {lost} pixels"
    );
}
