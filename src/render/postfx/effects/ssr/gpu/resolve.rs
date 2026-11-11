//! The resolve's edges (#2028's review): a mirror's misses beside a satin strip's
//! hits stay misses, so the mirror's reflected side stays sharp; and a hit
//! that reads `inf` from the image is a miss, so no `inf` or NaN reaches the frame.

use glam::Vec3;

use super::edge::lifted;
use super::fixture::{paint, place};
use super::silhouette::{at, shot, BRONZE, H, W};
use crate::core::color::f16_to_f32;
use crate::render::readback::read_texture_bytes;
use crate::render::{RenderView, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// The lifted cube over a mirror floor; when `strip`, a satin (roughness 0.5)
/// bronze strip set in the floor just right of the cube's reflection (its left
/// side on the line from the eye's foot through a point `GAP` right of the
/// cube's), with a blue cube standing on it: a contact reflection, so its hits
/// keep their weight and their radius dominates the mirror's misses beside it.
fn beside(on: bool, strip: bool) -> Scene {
    let mut s = lifted(on, 0.04);
    if strip {
        let along = Vec3::new(0.5 + GAP, 0.0, -6.0).normalize();
        let right = Vec3::new(-along.z, 0.0, along.x);
        let id = create_entity(&mut s, "Strip", Some(Primitive::Box));
        let centre = along * 4.5 + right * 0.7 + Vec3::Y * 0.01;
        place(&mut s, id, centre, Vec3::new(1.4, 0.02, 7.0));
        let turn = glam::Quat::from_rotation_y(-along.x.atan2(-along.z));
        s.world.transform_mut(id).unwrap().rotation = turn;
        paint(&mut s, id, "satin", (BRONZE, 1.0, 0.5));
        let id = create_entity(&mut s, "Over", Some(Primitive::Box));
        place(
            &mut s,
            id,
            along * 4.0 + right * 0.7 + Vec3::Y * 0.5,
            Vec3::ONE,
        );
        paint(&mut s, id, "over", ([0.1, 0.1, 1.0], 0.0, 1.0));
    }
    s
}

/// How far right of the cube's side the strip begins, at the cube's depth.
const GAP: f32 = 0.15;

/// The redness SSR adds along the floor's row whose middle the cube's reflection
/// reddens most (`row`, or that row of this frame when `None`): the row, and per
/// column what `on` adds over `off`.
fn added(on: &[u8], off: &[u8], row: Option<u32>) -> (u32, Vec<i32>) {
    let red = |f: &[u8], x: u32, y: u32| at(f, x, y)[0] - at(f, x, y)[2];
    let add = |y: u32, x: u32| red(on, x, y) - red(off, x, y);
    let y = row.unwrap_or_else(|| (H / 2..H).max_by_key(|&y| add(y, W / 2)).unwrap());
    (y, (0..W).map(|x| add(y, x)).collect())
}

#[test]
fn gpu_a_mirror_beside_a_satin_strip_keeps_its_reflection_sharp() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let mut frames = |strip| {
        let on = shot(&mut r, &beside(true, strip), &camera());
        (on, shot(&mut r, &beside(false, strip), &camera()))
    };
    let ((alone_on, bare), (on, off)) = (frames(false), frames(true));
    let (row, alone) = added(&alone_on, &bare, None);
    let (_, strip) = added(&on, &off, Some(row));
    // Right of the reflection's middle, up to where the strip shows: the mirror's
    // own pixels, the reflection's right side and the misses past it.
    let moved = |x: u32| {
        (0..3)
            .map(|c| (at(&off, x, row)[c] - at(&bare, x, row)[c]).abs())
            .sum::<i32>()
    };
    let end = (W / 2..W).find(|&x| moved(x) > 6).unwrap_or(W);
    let spread = (W / 2..end)
        .map(|x| (strip[x as usize] - alone[x as usize]).abs())
        .max()
        .unwrap_or(0);
    let share = spread as f32 / alone[W as usize / 2].max(1) as f32;
    eprintln!("[ssr] the mirror's reflection beside a satin strip, columns {}..{end}: off from its own alone by at most {share:.2} of its redness", W / 2);
    assert!(
        end >= W / 2 + 25,
        "the strip starts past the reflection's side"
    );
    assert!(
        share <= 0.05,
        "a mirror's reflection stays sharp beside a satin surface"
    );
}

/// The lifted cube's view, from above the floor.
fn camera() -> Camera {
    Camera::new(Vec3::new(0.0, 1.0, 0.0), -90.0, -12.0)
}

/// The lower half of the HDR image the SSR pass resolved into, as RGBA floats,
/// after a second frame of `scene` whose SSR read `inf` over the upper half.
fn poisoned(r: &mut crate::render::Renderer, scene: &Scene) -> Vec<[f32; 4]> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, &camera(), &out, false);
    view.post_fx.poison_ssr_source();
    r.render(&mut view, scene, &camera(), &out, false);
    let texture = &view.post_fx.scene_hdr.texture;
    let bytes = read_texture_bytes(&r.device, &r.queue, texture, (W, H), 8);
    let half = |i: usize| f16_to_f32(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
    let texel = |t: usize| std::array::from_fn(|c| half(t * 8 + c * 2));
    ((W * H / 2) as usize..(W * H) as usize)
        .map(texel)
        .collect()
}

/// Whether texel `c` is spoilt: non-finite, or black, which is what a NaN
/// becomes on a driver whose `max(NaN, 0)` gives 0 (lavapipe).
fn spoilt(c: &[f32; 4]) -> bool {
    !c.iter().all(|v| v.is_finite()) || c[..3].iter().all(|v| *v == 0.0)
}

#[test]
fn gpu_an_inf_ssr_hits_reflects_nothing_non_finite() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    // The cube is in the upper half; the floor reflects it in the lower. A
    // mirror and a near-mirror read their hits from the image's first level
    // (rougher ones read mips, which the pyramid's filter already cleans).
    for roughness in [0.04, 0.1, 0.35] {
        let lower = poisoned(&mut r, &lifted(true, roughness));
        let n = lower.iter().filter(|c| spoilt(c)).count();
        eprintln!("[ssr] an inf where a roughness-{roughness} floor's rays hit: {n} texels below it spoilt");
        assert_eq!(n, 0, "a non-finite hit is a miss");
    }
}
