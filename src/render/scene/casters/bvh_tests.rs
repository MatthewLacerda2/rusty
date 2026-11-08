//! The caster index (#1874), adapter-free: it reaches every entry a light volume's
//! frustum keeps, over seeded random levels and volumes (point-light faces, spots,
//! cascades, one degenerate), reaches few of them for a small volume, and answers
//! for an entity only under the bounds it was indexed with.

use glam::camera::rh::proj::directx;
use glam::camera::rh::view::look_to_mat4;
use glam::{Mat4, Vec3};

use super::CasterIndex;
use crate::render::Frustum;

/// A seeded xorshift's next value in `0..1`.
fn next(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 40) as f32 / (1u64 << 24) as f32
}

/// `n` boxes from a tenth of a metre to a few metres across, over a 200 m level.
fn level(n: usize, seed: u64) -> Vec<(u32, (Vec3, Vec3))> {
    let mut s = seed;
    let mut v = |scale: f32| Vec3::new(next(&mut s), next(&mut s), next(&mut s)) * scale;
    (0..n as u32)
        .map(|id| {
            let lo = v(200.0) - Vec3::new(0.0, 100.0, 0.0);
            (id, (lo, lo + v(4.0) + Vec3::splat(0.1)))
        })
        .collect()
}

/// A point light's six faces and a spot at `at`, reaching `range`; a cascade over
/// the level; and a degenerate view-projection.
fn volumes(at: Vec3, range: f32) -> Vec<Mat4> {
    let face = directx::perspective(std::f32::consts::FRAC_PI_2, 1.0, 0.05, range);
    let spot = directx::perspective(0.6, 1.0, 0.05, range);
    let mut out: Vec<Mat4> = [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z]
        .map(|d| face * look_to_mat4(at, d, Vec3::Y))
        .into_iter()
        .collect();
    out.push(face * look_to_mat4(at, Vec3::Y, Vec3::Z));
    out.push(face * look_to_mat4(at, -Vec3::Y, Vec3::Z));
    out.push(spot * look_to_mat4(at, Vec3::new(1.0, -1.0, 0.3), Vec3::Y));
    let sun = look_to_mat4(at, Vec3::new(-0.3, -1.0, 0.2), Vec3::Z);
    out.push(directx::orthographic(-30.0, 30.0, -30.0, 30.0, -150.0, 150.0) * sun);
    out.push(Mat4::ZERO);
    out
}

/// The entries `index` reaches through `view_proj`, as entity ids, sorted.
fn reached(index: &CasterIndex, view_proj: Mat4) -> Vec<u32> {
    let mut hits = Vec::new();
    index.visit(&Frustum::from_view_proj(view_proj), |e| {
        hits.push(index.id(e))
    });
    hits.sort_unstable();
    hits
}

#[test]
fn the_index_reaches_every_caster_a_light_volume_keeps() {
    for (n, seed) in [(1, 7), (5, 11), (300, 3), (2000, 1874)] {
        let boxes = level(n, seed);
        let index = CasterIndex::build(boxes.iter().map(|&(id, b)| (id, Some(b))));
        assert_eq!(index.len(), n);
        let mut s = seed * 31 + 1;
        for _ in 0..12 {
            let at = Vec3::new(
                next(&mut s) * 200.0,
                next(&mut s) * 10.0,
                next(&mut s) * 200.0,
            );
            for view_proj in volumes(at, 2.0 + next(&mut s) * 20.0) {
                let frustum = Frustum::from_view_proj(view_proj);
                let hits = reached(&index, view_proj);
                for &(id, (lo, hi)) in &boxes {
                    if frustum.intersects_aabb(lo, hi) {
                        assert!(hits.binary_search(&id).is_ok(), "{n}: {id} missed");
                    }
                }
                assert!(hits.windows(2).all(|w| w[0] < w[1]), "an entry twice");
            }
        }
    }
}

#[test]
fn a_small_volume_reaches_few_of_a_large_level() {
    let boxes = level(6000, 20);
    let index = CasterIndex::build(boxes.iter().map(|&(id, b)| (id, Some(b))));
    let at = Vec3::new(100.0, 2.0, 100.0);
    let face = directx::perspective(std::f32::consts::FRAC_PI_2, 1.0, 0.05, 9.0);
    let hits = reached(&index, face * look_to_mat4(at, Vec3::X, Vec3::Y));
    assert!(hits.len() < 200, "{} of 6000 reached", hits.len());
}

#[test]
fn an_entity_is_answered_for_only_under_the_bounds_it_was_indexed_with() {
    let a = (Vec3::ZERO, Vec3::ONE);
    let nan = (Vec3::ZERO, Vec3::new(f32::NAN, 1.0, 1.0));
    let mut index = CasterIndex::build([(4, Some(a)), (9, Some(nan)), (7, None)].into_iter());
    assert_eq!(index.len(), 1, "a box that is not finite is left out");
    assert_eq!(index.entry(4, Some(a)), Some(0));
    assert_eq!(index.entry(4, Some((Vec3::ZERO, Vec3::splat(2.0)))), None);
    assert_eq!(
        index.entry(4, None),
        None,
        "unbounded: tested by every volume"
    );
    assert_eq!(
        (index.entry(9, Some(nan)), index.entry(5, Some(a))),
        (None, None)
    );
    // Out of every volume: only the loose ones are handed over, then a stray too.
    let nowhere = [Frustum::from_view_proj(directx::orthographic(
        90.0, 91.0, 90.0, 91.0, 0.0, 1.0,
    ))];
    assert_eq!(index.near(&nowhere), [9, 7]);
    index.touch(4);
    assert_eq!((index.entry(4, Some(a)), index.strays()), (None, 1));
    assert_eq!(index.near(&nowhere), [9, 7, 4]);
    let empty = CasterIndex::build(std::iter::empty());
    let mut hits = 0;
    empty.visit(&Frustum::from_view_proj(Mat4::ZERO), |_| hits += 1);
    assert_eq!(hits, 0);
}
