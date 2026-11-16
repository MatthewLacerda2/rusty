//! How deep the march takes an object at a thick setting (#2228, `ssr_march.wgsl`'s
//! `object`): HDRP's `behind ≤ thickness × z`, `z` the depth of the surface the
//! ray is behind, so at 0.5 an object is taken half its depth deep and at 1 all of
//! it. A narrow red card stands 2 m out before a mirror wall 6 m out, right of the
//! eye; the mirror's rays come back towards the eye and pass behind the card on
//! screen, each at its own least depth behind it, so how far the card's
//! reflection reaches across the mirror says how deep the march took the card.

use glam::{Vec2, Vec3};

use super::super::fixture::{paint, place, tuned, MIRROR, RED};
use super::hits::Hits;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

const SIZE: (u32, u32) = (640, 360);
/// The card's front and the mirror's, metres out; the card's edges, as slopes
/// `x / depth` from the eye.
const CARD: f32 = 2.0;
const MIRROR_AT: f32 = 6.0;
const LEFT: f32 = 0.3;
const RIGHT: f32 = 0.5;

/// The card and the mirror at `thickness`, the eye 1 m up looking down -Z.
fn card_before_a_mirror(thickness: f32) -> (Scene, Camera) {
    let (mut s, _) = tuned(true, ([0.3; 3], 0.0, 1.0), RED, |v| {
        v.ssr.thickness = thickness;
    });
    let cube = s.find_entity_by_name("Cube").unwrap();
    place(&mut s, cube, Vec3::new(50.0, 0.5, -6.0), Vec3::ONE);
    let wall = create_entity(&mut s, "Mirror", Some(Primitive::Box));
    let wall_at = Vec3::new(0.0, 1.5, -MIRROR_AT - 0.04);
    place(&mut s, wall, wall_at, Vec3::new(20.0, 3.0, 0.08));
    paint(&mut s, wall, "mirror", MIRROR);
    let card = create_entity(&mut s, "Card", Some(Primitive::Box));
    let width = (RIGHT - LEFT) * CARD;
    let at = Vec3::new((LEFT + RIGHT) * 0.5 * CARD, 1.0, -CARD - 0.005);
    place(&mut s, card, at, Vec3::new(width, 1.0, 0.01));
    paint(&mut s, card, "card", RED);
    (s, Camera::new(Vec3::new(0.0, 1.0, 0.0), -90.0, 0.0))
}

/// Where `p` lands through `cam`, in pixels.
fn pixel(cam: &Camera, p: Vec3) -> Vec2 {
    let clip = cam.build_view_projection(SIZE.0 as f32 / SIZE.1 as f32) * p.extend(1.0);
    let ndc: Vec2 = clip.truncate().truncate() / clip.w;
    Vec2::new(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * Vec2::new(SIZE.0 as f32, SIZE.1 as f32)
}

/// The mirror's column, as a slope `u`, out to which the card's reflection
/// reaches at `thickness`. A ray from the mirror at `u` comes back along the line
/// from the eye's mirror image, `x = u (2M - d)` at depth `d`, so it lies on the
/// card's right edge on screen at `d = 2M u / (RIGHT + u)`, its least depth behind
/// it, `d - CARD`; it hits while that is within `thickness × CARD`.
fn reach(thickness: f32) -> f32 {
    let d = CARD * (1.0 + thickness);
    RIGHT * d / (2.0 * MIRROR_AT - d)
}

/// How many columns past `reach` the reflection may end, either way: the card's
/// raster edge and where the refinement places a hit. Measured (lavapipe): 0.4
/// at 0.5 and at 1; HDRP's model read from the ray's own depth instead, 36.6 and
/// 21.4 (the card's whole near edge: nothing behind it was ever passed).
const SLOP: f32 = 3.0;

/// The card's reflection's right end on the eye's row, in pixels, and where
/// `reach` puts it.
fn reflection(thickness: f32) -> Option<(f32, f32)> {
    let mut r = crate::test_gpu::headless_or_skip(SIZE.0, SIZE.1)?;
    let (scene, cam) = card_before_a_mirror(thickness);
    let hits = Hits::of(&mut r, &scene, &cam, SIZE);
    let y = pixel(&cam, Vec3::new(0.0, 1.0, -MIRROR_AT)).y as u32;
    let card = pixel(&cam, Vec3::new(LEFT * CARD, 1.0, -CARD)).x as u32;
    let end = (0..card).filter(|&x| hits.is(x, y, 0)).max()? as f32 + 1.0;
    let u = reach(thickness);
    let want = pixel(&cam, Vec3::new(u * MIRROR_AT, 1.0, -MIRROR_AT)).x;
    eprintln!("[ssr] a card before a mirror at thickness {thickness}: its reflection ends at column {end}, the model's {want:.1}");
    Some((end, want))
}

#[test]
fn gpu_half_thickness_takes_an_object_half_its_depth_deep() {
    let Some((end, want)) = reflection(0.5) else {
        return;
    };
    assert!(
        (end - want).abs() <= SLOP,
        "at thickness 0.5 the card's reflection ends at column {end}, not {want:.1}"
    );
}

#[test]
fn gpu_full_thickness_takes_an_object_all_its_depth_deep() {
    let Some((end, want)) = reflection(1.0) else {
        return;
    };
    assert!(
        (end - want).abs() <= SLOP,
        "at thickness 1 the card's reflection ends at column {end}, not {want:.1}"
    );
}
