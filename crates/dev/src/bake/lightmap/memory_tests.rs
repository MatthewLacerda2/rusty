use std::sync::Arc;

use glam::{Vec2, Vec3};

use crate::bake::denoise::Oidn;
use crate::bake::progress::MeshClocks;
use crate::memory::rss_peak_bytes;
use rusty_lighting::lightmap::{
    bake_on, BakeLight, BakeMesh, BakeProgress, BakeRun, BakeScene, LightShape, Lightmap,
};
use rusty_scene::lighting::settings::{BakeSettings, Denoiser};

pub(crate) const DENSITY: f32 = 25.0;
pub(crate) const SIDE: f32 = 20.0;

pub(crate) const WORKERS: usize = 2;

pub(crate) const PAIR: (usize, usize) = match cfg!(target_os = "macos") {
    true => (8, 32),
    false => (2, 8),
};

pub(crate) const MARGIN: u64 = 48 << 20;

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

pub(crate) fn kept(maps: &[Lightmap]) -> u64 {
    maps.iter().map(Lightmap::kept_bytes).sum::<usize>() as u64
}

pub(crate) fn bake(n: usize, side: f32, denoiser: Denoiser) -> (u64, u64) {
    crate::memory::return_large_frees();
    let observed = BakeProgress::observed(Arc::new(MeshClocks::default()));
    let progress = match denoiser {
        Denoiser::Oidn => observed.with_denoiser(Arc::new(Oidn::with_devices(WORKERS))),
        _ => observed,
    };
    let maps = bake_on(&planes(n, side), &settings(denoiser), &progress, WORKERS).expect("ran");
    assert_eq!(maps.len(), n);
    let (peak, kept) = (rss_peak_bytes().expect("read"), kept(&maps));
    let (heap, peak_mb, kept_mb) = (crate::memory::heap(), peak >> 20, kept >> 20);
    println!("{n} × {side}: peak {peak_mb} MB, maps {kept_mb} MB, {heap:?}");
    (peak, kept)
}

pub(crate) const FREED_PER_CHART: u64 = (cfg!(target_os = "macos") as u64) * (8 << 20);

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
    rusty_test_support::alone!(|| {
        let (small, large) = PAIR;
        let one = bake(small, SIDE, Denoiser::Filter);
        let four = bake(large, SIDE, Denoiser::Filter);
        holds(one, four, large - small, "Gaussian");
    });
}
