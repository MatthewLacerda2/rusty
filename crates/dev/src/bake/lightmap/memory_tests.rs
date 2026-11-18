//! A lightmap bake's memory is bounded by its work in flight, not by its scene
//! (#2312): a scene of four times the meshes at the same density peaks higher only
//! by the extra lightmaps it keeps, with the Gaussian and with OIDN, whose own
//! share is held to its budget. What is in flight is per worker, so both bakes
//! run on [`WORKERS`]. Each test is its own process under nextest, so the
//! peak (`memory::rss_peak_bytes`) is this test's bakes'.

use std::sync::Arc;

use glam::{Vec2, Vec3};

use crate::bake::denoise::{self, budget, Oidn};
use crate::memory::rss_peak_bytes;
use rusty_lighting::lightmap::{
    bake_on, BakeLight, BakeMesh, BakeProgress, BakeRun, BakeScene, LightShape, Lightmap,
};
use rusty_scene::lighting::settings::{BakeSettings, Denoiser};

/// Lightmap texels per unit: each [`SIDE`]-unit plane is a 512² map.
const DENSITY: f32 = 25.0;
const SIDE: f32 = 20.0;

/// The bake's workers: what is in flight is per worker, so the two bakes compared
/// have as many each, and the smaller has a mesh for every one.
const WORKERS: usize = 2;

/// Slack on top of the extra results: the allocator's and the pool's.
const MARGIN: u64 = 48 << 20;

/// `n` planes side by side, under a sun.
fn planes(n: usize) -> BakeScene {
    let plane = |i: usize| {
        let x = i as f32 * (SIDE + 1.0);
        let p = |a: f32, b: f32| Vec3::new(x + a, 0.0, b);
        BakeMesh {
            entity: i as u32 + 1,
            positions: vec![p(0.0, SIDE), p(SIDE, SIDE), p(SIDE, 0.0), p(0.0, 0.0)],
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
fn kept(maps: &[Lightmap]) -> u64 {
    let one = |m: &Lightmap| m.texels.len() * 12 + (m.directions.len() + m.masks.len()) * 16;
    maps.iter().map(one).sum::<usize>() as u64
}

/// Bake `n` planes with `denoiser`, lent OIDN for it (a device per worker, as a
/// bake lends): the process's peak after, and what the maps keep.
fn bake(n: usize, denoiser: Denoiser) -> (u64, u64) {
    // As every bake does before it starts (`fits::fits`).
    crate::memory::return_large_frees();
    let progress = match denoiser {
        Denoiser::Oidn => {
            BakeProgress::default().with_denoiser(Arc::new(Oidn::with_devices(WORKERS)))
        }
        _ => BakeProgress::default(),
    };
    let maps = bake_on(&planes(n), &settings(denoiser), &progress, WORKERS).expect("ran");
    assert_eq!(maps.len(), n);
    (
        rss_peak_bytes().expect("read on Linux and macOS"),
        kept(&maps),
    )
}

/// The 4× bake's peak past the 1× one's, against what its extra maps keep.
fn holds(one: (u64, u64), four: (u64, u64), what: &str) {
    let (grew, extra) = (four.0.saturating_sub(one.0), four.1 - one.1);
    let mb = |b: u64| b >> 20;
    assert!(
        grew <= extra + MARGIN,
        "{what}: four times the meshes peaked {} MB higher, but their extra lightmaps \
         keep only {} MB: something is held per texel of the whole scene",
        mb(grew),
        mb(extra)
    );
}

#[test]
fn four_times_the_meshes_peak_higher_only_by_their_lightmaps() {
    if rss_peak_bytes().is_none() {
        return;
    }
    let one = bake(2, Denoiser::Filter);
    let four = bake(8, Denoiser::Filter);
    holds(one, four, "Gaussian");
}

#[test]
fn oidn_adds_at_most_its_budget_and_holds_the_bound_too() {
    if rss_peak_bytes().is_none() || denoise::available().is_err() {
        // `RUSTY_REQUIRE_OIDN`'s canary is what fails a CI run without it.
        return;
    }
    let filtered = bake(2, Denoiser::Filter);
    let one = bake(2, Denoiser::Oidn);
    let texels = (DENSITY * SIDE) as usize * (DENSITY * SIDE) as usize;
    let added = one.0.saturating_sub(filtered.0);
    let bound = (WORKERS as u64 * budget::cost(texels)).min(budget::BUDGET);
    assert!(
        added <= bound + MARGIN,
        "OIDN added {added} bytes, past {bound}"
    );
    let four = bake(8, Denoiser::Oidn);
    holds(one, four, "OIDN");
}
