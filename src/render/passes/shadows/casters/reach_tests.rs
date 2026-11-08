//! A light volume culled through the caster index keeps exactly what testing every
//! candidate keeps, in the same order (#1874), adapter-free: over a seeded level of
//! multi-run entities, some unbounded, some moved since they were indexed, some
//! never indexed, and volumes that reach a few, all, or none of them.

use glam::camera::rh::proj::directx;
use glam::camera::rh::view::look_to_mat4;
use glam::{Mat4, Vec3};

use super::buffer::CasterData;
use super::cull::{cull, Candidate, LightVolume, ShadowCulls};
use super::reach::Reach;
use super::CasterKey;
use crate::render::draw::batch::IndexRun;
use crate::render::gpu::pipelines::winding::Facing;
use crate::render::scene::{CasterIndex, MeshHandle, OcclusionBinding};

/// A seeded xorshift's next value in `0..1`.
fn next(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 40) as f32 / (1u64 << 24) as f32
}

/// `n` entities of one to three runs over a 100 m level; every seventh small. The
/// index holds every one but each fifth, with every eleventh's bounds moved since
/// and every thirteenth unbounded.
fn level(n: u32, s: &mut u64) -> (Vec<Candidate>, CasterIndex) {
    let (mut candidates, mut indexed) = (Vec::new(), Vec::new());
    for id in 0..n {
        let lo = Vec3::new(next(s), next(s) * 0.1, next(s)) * 100.0;
        let hi = lo + Vec3::splat(0.2 + next(s) * 3.0);
        if id % 5 != 0 {
            let shift = Vec3::splat(if id % 11 == 0 { 9.0 } else { 0.0 });
            indexed.push((id, (lo + shift, hi + shift)));
        }
        let bounds = (id % 13 != 0).then_some((lo, hi));
        for run in 0..1 + id % 3 {
            candidates.push(Candidate {
                id,
                key: CasterKey {
                    facing: Facing::Ccw,
                    clip: None,
                    mesh: MeshHandle(id % 4),
                    indices: IndexRun {
                        start: run * 6,
                        end: run * 6 + 6,
                    },
                },
                caster: CasterData::new(Mat4::from_translation(lo), id),
                bounds,
                small: id % 7 == 0,
                two_sided: false,
            });
        }
    }
    (
        candidates,
        CasterIndex::build(indexed.into_iter().map(|(id, b)| (id, Some(b), false))),
    )
}

#[test]
fn a_volume_through_the_index_keeps_what_testing_every_caster_keeps() {
    let pvs = OcclusionBinding::default();
    let mut s = 1874;
    for n in [0, 1, 40, 1500] {
        let (candidates, index) = level(n, &mut s);
        let reach = Reach::new(&candidates, Some(&index));
        let face = directx::perspective(std::f32::consts::FRAC_PI_2, 1.0, 0.05, 12.0);
        let mut views = vec![
            directx::orthographic(-80.0, 80.0, -80.0, 80.0, -200.0, 200.0)
                * look_to_mat4(Vec3::splat(50.0), -Vec3::Y, Vec3::Z),
            Mat4::ZERO,
        ];
        for _ in 0..20 {
            let at = Vec3::new(next(&mut s), 0.05, next(&mut s)) * 100.0;
            views.push(face * look_to_mat4(at, Vec3::X, Vec3::Y));
            views.push(face * look_to_mat4(at, -Vec3::Y, Vec3::Z));
        }
        for view_proj in views {
            let volume = LightVolume::open(view_proj);
            let (mut all, mut some) = (ShadowCulls::default(), ShadowCulls::default());
            let every = cull(&candidates, volume, &pvs, &mut all);
            let picked = reach.of(view_proj);
            let through = cull(
                picked.into_iter().flat_map(|r| &candidates[r]),
                volume,
                &pvs,
                &mut some,
            );
            assert_eq!(every, through, "{n} entities: what a volume draws moved");
            let drawn = |c: ShadowCulls| c.tested - c.frustum - c.size;
            assert_eq!((drawn(all), all.size), (drawn(some), some.size), "{n}");
            assert!(some.tested <= all.tested);
        }
    }
}

#[test]
fn with_no_index_every_volume_tests_every_caster() {
    let (candidates, _) = level(30, &mut 5);
    let reach = Reach::new(&candidates, None);
    let picked: Vec<_> = reach.of(Mat4::IDENTITY).into_iter().flatten().collect();
    assert_eq!(picked, (0..candidates.len()).collect::<Vec<_>>());
}
