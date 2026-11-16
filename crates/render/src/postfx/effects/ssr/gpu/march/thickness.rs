//! How deep the march takes what it hits (#2228, `ssr_march.wgsl`'s `object`:
//! the volume's `thickness`, a fraction of the depth): a crate keeps its hidden
//! side in a mirror behind it at a thick setting and leaks it at the default, a
//! thin pole over a mirror floor reflects no further than its own mirror image,
//! and a march of steep 1 m steps, which no pyramid walk narrows, leaves no holes
//! in it.

use glam::{Vec2, Vec3};

use super::super::fixture::{paint, place, tuned, MIRROR, RED};
use super::hits::Hits;
use rusty_core::settings::quality::EffectQuality;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

const SIZE: (u32, u32) = (640, 360);
const GREEN: usize = 1;

/// A 50 cm red crate on a dull floor, a mirror wall 35 cm behind it, seen from
/// the side and above, the volume's `thickness` at `thickness`: the mirror shows
/// the crate's far side, which the depth target never holds.
fn crate_and_mirror(thickness: f32) -> (Scene, Camera) {
    let (mut s, _) = tuned(true, ([0.3; 3], 0.0, 1.0), RED, |v| {
        v.ssr.thickness = thickness;
    });
    let cube = s.find_entity_by_name("Cube").unwrap();
    place(&mut s, cube, Vec3::new(0.0, 0.25, -3.0), Vec3::splat(0.5));
    let wall = create_entity(&mut s, "Mirror", Some(Primitive::Box));
    place(
        &mut s,
        wall,
        Vec3::new(0.0, 1.5, -3.6),
        Vec3::new(8.0, 3.0, 0.08),
    );
    paint(&mut s, wall, "mirror", MIRROR);
    (s, Camera::new(Vec3::new(0.9, 1.1, -1.2), -105.0, -20.0))
}

/// The fewest red pixels the crate's reflection may hold at `thickness` 0.15
/// (45 cm at its 3 m): a band of the floor along its silhouette, where its far
/// side should be, takes them where a ray passes behind it further than that.
/// Measured (lavapipe): 6034 (a fixed 15 cm, the march without a pyramid's, 4954).
const CRATE: usize = 4850;
/// The same at the default `thickness`, 0.01 (3 cm): HDRP's, which lets the
/// crate's hidden side leak, the trade for a thin strip's reflection as thin as
/// it. Measured (lavapipe): 4089; 2942 with a ray behind the crate by more than
/// its object taken as past it for good.
const CRATE_DEFAULT: usize = 3800;

/// The crate's reflection in red pixels, at `thickness`.
fn crate_reflection(thickness: f32) -> Option<usize> {
    let mut r = crate::test_gpu::headless_or_skip(SIZE.0, SIZE.1)?;
    let (scene, cam) = crate_and_mirror(thickness);
    let red = Hits::of(&mut r, &scene, &cam, SIZE).count(0);
    eprintln!("[ssr] the crate's reflection at thickness {thickness}: {red} px");
    Some(red)
}

#[test]
fn gpu_a_thick_setting_keeps_a_crates_hidden_side_in_a_mirror_behind_it() {
    let Some(red) = crate_reflection(0.15) else {
        return;
    };
    assert!(
        red >= CRATE,
        "the crate's far side drops out of its reflection at thickness 0.15: {red} px"
    );
}

#[test]
fn gpu_the_default_thickness_leaks_no_more_of_a_crates_hidden_side_than_measured() {
    let Some(red) = crate_reflection(0.01) else {
        return;
    };
    assert!(
        red >= CRATE_DEFAULT,
        "the crate's reflection at the default thickness: {red} px, measured 4089"
    );
}

/// A green pole 2 cm square and 1 m tall, 5 m out on the fixture's mirror floor,
/// marched in steps of `step` metres (0: the default level), and the eye 1.5 m up
/// looking 20° down.
fn pole(step: f32) -> (Scene, Camera) {
    let (mut s, _) = tuned(true, MIRROR, RED, |v| {
        if step > 0.0 {
            v.ssr.quality = EffectQuality::Custom;
            (v.ssr.steps, v.ssr.step_length) = (64, step);
        }
    });
    let cube = s.find_entity_by_name("Cube").unwrap();
    place(&mut s, cube, Vec3::new(50.0, 0.5, -6.0), Vec3::ONE);
    let id = create_entity(&mut s, "Pole", Some(Primitive::Box));
    place(
        &mut s,
        id,
        Vec3::new(0.0, 0.5, -5.0),
        Vec3::new(0.02, 1.0, 0.02),
    );
    paint(&mut s, id, "pole", ([0.1, 0.9, 0.1], 0.0, 1.0));
    (s, Camera::new(Vec3::new(0.0, 1.5, 0.0), -90.0, -20.0))
}

/// The row `p` lands on through `cam`.
fn row(cam: &Camera, p: Vec3) -> f32 {
    let clip = cam.build_view_projection(SIZE.0 as f32 / SIZE.1 as f32) * p.extend(1.0);
    let ndc: Vec2 = clip.truncate().truncate() / clip.w;
    (0.5 - ndc.y * 0.5) * SIZE.1 as f32
}

/// The pole's reflection, against its mirror image's rows (its base to its top
/// mirrored below the floor): how many rows past the image it reaches, and how
/// many rows of the image it leaves out.
fn reflection(step: f32) -> Option<(i64, usize)> {
    let mut r = crate::test_gpu::headless_or_skip(SIZE.0, SIZE.1)?;
    let (scene, cam) = pole(step);
    let rows = Hits::of(&mut r, &scene, &cam, SIZE).rows(GREEN);
    let base = row(&cam, Vec3::new(0.0, 0.0, -4.99)).ceil() as u32 + 1;
    let top = row(&cam, Vec3::new(0.0, -1.0, -4.99)).floor() as u32;
    let past = i64::from(*rows.last()?) - i64::from(top);
    let holes = (base..top).filter(|y| !rows.contains(y)).count();
    eprintln!("[ssr] a 2 cm pole, steps of {step} m: its reflection reaches {past} rows past its image, with {holes} of {} rows missing", top - base);
    Some((past, holes))
}

/// The most rows a thin pole's reflection may reach past its mirror image: rays
/// passing over its top that count as hitting it from behind. Measured
/// (lavapipe): -2 at either step (the march without a pyramid -5 and -2: the image's far end fades).
const PAST: i64 = 2;
/// The most rows of its image the reflection may leave out: the one at its foot,
/// where the pole meets the floor. Measured (lavapipe): 1 at either step.
const HOLES: usize = 1;

#[test]
fn gpu_a_thin_pole_over_a_mirror_floor_reflects_no_further_than_its_image() {
    let Some((past, holes)) = reflection(0.0) else {
        return;
    };
    assert!(
        past <= PAST,
        "a 2 cm pole over-reflects: {past} rows past its image"
    );
    assert!(holes <= HOLES, "a 2 cm pole's reflection has {holes} holes");
}

#[test]
fn gpu_a_thin_poles_reflection_from_steep_steps_keeps_no_holes() {
    let Some((past, holes)) = reflection(1.0) else {
        return;
    };
    assert!(
        holes <= HOLES,
        "1 m steps leave {holes} holes in a 2 cm pole's reflection"
    );
    assert!(
        past <= PAST,
        "a 2 cm pole over-reflects: {past} rows past its image"
    );
}
