//! A lightmap bake's memory is bounded by its work in flight, not by its scene
//! (#2312): four times the meshes at the same density peak higher only by the
//! extra lightmaps kept, on [`WORKERS`] each (what is in flight is per worker).
//! The page stage's bound: `pages_memory_tests`. Each test is its own process
//! under nextest, so the peak (`memory::rss_peak_bytes`) is its bakes'; OIDN's
//! memory tests share the planes and bakes (`denoise::memory_tests`).

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
/// of freed large blocks in the footprint, which 2 planes do not fill: there the
/// pair starts at 8, so only what the scene adds is compared.
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
    let (peak, kept) = (rss_peak_bytes().expect("read"), kept(&maps));
    // Shown when a bound fails: whether the allocator held freed memory or the
    // bake still used it.
    let (heap, peak_mb, kept_mb) = (crate::memory::heap(), peak >> 20, kept >> 20);
    println!("{n} × {side}: peak {peak_mb} MB, maps {kept_mb} MB, {heap:?}");
    (peak, kept)
}

/// What macOS's footprint may keep counted of each extra chart's freed buffers:
/// libmalloc keeps freed blocks counted even after `memory::relieve`, though its
/// in-use figure after each bake is the maps' (8 MB: the measured 7.9 MB a chart
/// at 24 extra charts, rounded up; a bake that kept its charts' buffers, 20 MB a
/// chart, still fails). Linux's freed large blocks leave (`return_large_frees`).
pub(crate) const FREED_PER_CHART: u64 = (cfg!(target_os = "macos") as u64) * (8 << 20);

/// The 4× bake's peak past `one`'s, against what its `charts` extra charts' maps
/// keep, plus [`FREED_PER_CHART`] each.
pub(crate) fn holds(one: (u64, u64), four: (u64, u64), charts: usize, what: &str) {
    let (grew, extra) = (four.0.saturating_sub(one.0), four.1 - one.1);
    let freed = charts as u64 * FREED_PER_CHART;
    let mb = |b: u64| b >> 20;
    let (heap, four_mb, one_mb) = (crate::memory::heap(), mb(four.0), mb(one.0));
    assert!(
        grew <= extra + freed + MARGIN,
        "{what}: 4× the meshes peaked {} MB higher ({four_mb} against {one_mb} MB), past \
         their extra maps' {} MB + {} MB for {charts} charts' freed buffers (macOS's \
         bound; Linux's is 0) + margin: something is held per texel of the whole \
         scene (the allocator now: {heap:?})",
        mb(grew),
        mb(extra),
        mb(freed)
    );
}

#[test]
fn four_times_the_meshes_peak_higher_only_by_their_lightmaps() {
    if rss_peak_bytes().is_none() {
        return;
    }
    let (small, large) = PAIR;
    let one = bake(small, SIDE, Denoiser::Filter);
    let four = bake(large, SIDE, Denoiser::Filter);
    holds(one, four, large - small, "Gaussian");
}
