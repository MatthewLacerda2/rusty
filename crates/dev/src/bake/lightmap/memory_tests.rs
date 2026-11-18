//! A lightmap bake's memory is bounded by its work in flight, not by its scene
//! (#2312): a scene of four times the meshes at the same density peaks higher only
//! by the extra lightmaps it keeps. What is in flight is per worker, so both bakes
//! run on [`WORKERS`]. The page stage's bound, on maps that cover a sliver of
//! their squares: `pages_memory_tests`. Each test is its own process under nextest, so the peak
//! (`memory::rss_peak_bytes`) is this test's bakes'. The planes and bakes here
//! are OIDN's memory tests' too (`denoise::memory_tests`).

use std::sync::Arc;

use glam::{Vec2, Vec3};

use crate::bake::denoise::Oidn;
use crate::bake::progress::MeshClocks;
use crate::memory::rss_peak_bytes;
use rusty_lighting::lightmap::{
    bake_on, BakeLight, BakeMesh, BakeProgress, BakeRun, BakeScene, LightShape, Lightmap,
};
use rusty_scene::lighting::settings::{BakeSettings, Denoiser};

/// Lightmap texels per unit: each [`SIDE`]-unit plane is a 500² map.
pub(crate) const DENSITY: f32 = 25.0;
pub(crate) const SIDE: f32 = 20.0;

/// The bake's workers: what is in flight is per worker, so the two bakes compared
/// have as many each, and the smaller has a mesh for every one.
pub(crate) const WORKERS: usize = 2;

/// The scene sizes compared, the smaller first. macOS's allocator keeps a reserve
/// of freed large blocks in the process's footprint until it is full, which a
/// 2-plane bake does not fill: there the pair starts at 8 planes, so the reserve
/// is filled at 1× and only what the scene adds is compared.
pub(crate) const PAIR: (usize, usize) = match cfg!(target_os = "macos") {
    true => (8, 32),
    false => (2, 8),
};

/// Slack on top of the extra results: the allocator's and the pool's.
pub(crate) const MARGIN: u64 = 48 << 20;

/// `n` planes of `side` units side by side, under a sun.
pub(crate) fn planes(n: usize, side: f32) -> BakeScene {
    let plane = |i: usize| {
        let x = i as f32 * (side + 1.0);
        let p = |a: f32, b: f32| Vec3::new(x + a, 0.0, b);
        BakeMesh {
            entity: i as u32 + 1,
            positions: vec![p(0.0, side), p(side, side), p(side, 0.0), p(0.0, 0.0)],
            normals: vec![Vec3::Y; 4],
            lightmap_uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            indices: vec![0, 1, 2, 0, 2, 3],
            runs: vec![BakeRun {
                end: 6,
                albedo: Vec3::splat(0.5),
                emissive: Vec3::ZERO,
                occludes: true,
            }],
            ..Default::default()
        }
    };
    let sun = BakeLight {
        shape: LightShape::Directional {
            direction: Vec3::new(0.3, -1.0, 0.2).normalize(),
        },
        radiance: Vec3::splat(3.0),
        bakes_direct: true,
        mask_channel: None,
    };
    BakeScene {
        meshes: (0..n).map(plane).collect(),
        lights: vec![sun],
        ..Default::default()
    }
}

/// The cheapest trace (the trace's cost is not what is measured), directional.
fn settings(denoiser: Denoiser) -> BakeSettings {
    BakeSettings {
        texels_per_unit: DENSITY,
        samples: 1,
        bounces: 0,
        denoiser,
        filter_radius: 1,
        directional: true,
        ..Default::default()
    }
}

/// What finished lightmaps keep, in bytes.
pub(crate) fn kept(maps: &[Lightmap]) -> u64 {
    maps.iter().map(Lightmap::kept_bytes).sum::<usize>() as u64
}

/// Bake `n` planes of `side` units with `denoiser`, lent OIDN for it (a device per
/// worker, as a bake lends): the process's peak after, and what the maps keep.
pub(crate) fn bake(n: usize, side: f32, denoiser: Denoiser) -> (u64, u64) {
    // As every bake does before it starts (`fits::fits`).
    crate::memory::return_large_frees();
    // Observed as every bake is, whose observer hands freed memory back.
    let observed = BakeProgress::observed(Arc::new(MeshClocks::default()));
    let progress = match denoiser {
        Denoiser::Oidn => observed.with_denoiser(Arc::new(Oidn::with_devices(WORKERS))),
        _ => observed,
    };
    let maps = bake_on(&planes(n, side), &settings(denoiser), &progress, WORKERS).expect("ran");
    assert_eq!(maps.len(), n);
    let (peak, kept) = (
        rss_peak_bytes().expect("read on Linux and macOS"),
        kept(&maps),
    );
    // Shown when a bound fails: whether the allocator held freed memory or the
    // bake still used it.
    let heap = crate::memory::heap();
    println!(
        "{n} × {side}: peak {} MB, maps {} MB, {heap:?}",
        peak >> 20,
        kept >> 20
    );
    (peak, kept)
}

/// The 4× bake's peak past `one`'s, against what its extra maps keep.
pub(crate) fn holds(one: (u64, u64), four: (u64, u64), what: &str) {
    let (grew, extra) = (four.0.saturating_sub(one.0), four.1 - one.1);
    let mb = |b: u64| b >> 20;
    assert!(
        grew <= extra + MARGIN,
        "{what}: four times the meshes peaked {} MB higher ({} MB against {} MB), but \
         their extra lightmaps keep only {} MB: something is held per texel of the whole \
         scene (the allocator now: {:?})",
        mb(grew),
        mb(four.0),
        mb(one.0),
        mb(extra),
        crate::memory::heap()
    );
}

#[test]
fn four_times_the_meshes_peak_higher_only_by_their_lightmaps() {
    if rss_peak_bytes().is_none() {
        return;
    }
    let (one, four) = PAIR;
    let one = bake(one, SIDE, Denoiser::Filter);
    let four = bake(four, SIDE, Denoiser::Filter);
    holds(one, four, "Gaussian");
}
