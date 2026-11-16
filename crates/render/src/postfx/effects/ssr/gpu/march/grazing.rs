//! A floor seen near grazing keeps its hits (#2228, `ssr_march.wgsl`'s `grazes`):
//! a mirror panel turned 45° sends the eye's rays sideways at an even depth onto
//! a floor 8 m out, seen 3° from the line of sight at 1080p. A ray entering a
//! row of that floor is behind the row's depth by the row's step, about 2% of
//! the depth, past the object's 1%: the next row holds the floor's depth past the
//! ray, which is what tells it from a ray passing behind a thin object.

use glam::{Mat4, Quat, Vec2, Vec3, Vec4Swizzles};

use super::super::fixture::{paint, place, tuned, MIRROR, RED};
use super::hits::Hits;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

const SIZE: (u32, u32) = (1920, 1080);
/// The panel's centre and its turn about the vertical.
const PANEL: Vec3 = Vec3::new(1.5, 0.5, -8.0);
const TURN: f32 = -std::f32::consts::FRAC_PI_4;

/// A red floor, the mirror panel 2 m wide on it, under a black sky; the eye
/// 0.45 m up looking down -Z, 1° down.
fn panel_over_a_floor() -> (Scene, Camera) {
    let (mut s, _) = tuned(true, ([0.9, 0.1, 0.1], 0.0, 1.0), RED, |_| {});
    crate::test_gpu::black_sky(&mut s);
    let cube = s.find_entity_by_name("Cube").unwrap();
    place(&mut s, cube, Vec3::new(50.0, 0.5, -6.0), Vec3::ONE);
    let id = create_entity(&mut s, "Mirror", Some(Primitive::Box));
    place(&mut s, id, PANEL, Vec3::new(2.0, 1.0, 0.02));
    s.world.transform_mut(id).unwrap().rotation = Quat::from_rotation_y(TURN);
    paint(&mut s, id, "mirror", MIRROR);
    (s, Camera::new(Vec3::new(0.0, 0.45, 0.0), -90.0, -1.0))
}

/// Whether pixel `(x, y)` sees the panel's lowest 15 cm, away from its ends, and
/// its mirror ray meets the floor within 3 m: the pixels judged.
fn judged(cam: &Camera, inv: Mat4, (x, y): (u32, u32)) -> bool {
    let ndc = Vec2::new(
        (x as f32 + 0.5) / SIZE.0 as f32 * 2.0 - 1.0,
        1.0 - (y as f32 + 0.5) / SIZE.1 as f32 * 2.0,
    );
    let far = inv * ndc.extend(1.0).extend(1.0);
    let dir = (far.xyz() / far.w - cam.position).normalize();
    let (n, along) = (
        Quat::from_rotation_y(TURN) * Vec3::Z,
        Quat::from_rotation_y(TURN) * Vec3::X,
    );
    let t = (PANEL - cam.position).dot(n) / dir.dot(n);
    let m = cam.position + dir * t;
    let r = dir - 2.0 * dir.dot(n) * n;
    let on = t > 0.0 && (m - PANEL).dot(along).abs() < 0.95 && (0.01..0.15).contains(&m.y);
    on && r.y < 0.0 && (r * (-m.y / r.y)).length() < 3.0
}

/// The most of the judged pixels that may miss the floor. Measured (lavapipe): 0;
/// judged by the object's thickness alone, without the grazing allowance, 328 of
/// 4786 (6.9 %), in bands one row of the floor apart.
const MISSED: f32 = 0.005;

#[test]
fn gpu_a_floor_seen_near_grazing_keeps_its_hits() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(SIZE.0, SIZE.1) else {
        return;
    };
    let (scene, cam) = panel_over_a_floor();
    let hits = Hits::of(&mut r, &scene, &cam, SIZE);
    let inv = cam
        .build_view_projection(SIZE.0 as f32 / SIZE.1 as f32)
        .inverse();
    let pixels = (0..SIZE.1).flat_map(|y| (0..SIZE.0).map(move |x| (x, y)));
    let seen: Vec<_> = pixels.filter(|&p| judged(&cam, inv, p)).collect();
    let missed = seen.iter().filter(|&&(x, y)| !hits.is(x, y, 0)).count();
    let share = missed as f32 / seen.len().max(1) as f32;
    eprintln!(
        "[ssr] a floor seen at 3°: {missed} of its reflection's {} px miss it",
        seen.len()
    );
    assert!(seen.len() > 1000, "the floor's reflection is off screen");
    assert!(
        share <= MISSED,
        "a floor seen near grazing loses {share:.3} of its hits"
    );
}
