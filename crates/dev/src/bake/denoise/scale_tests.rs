//! The denoise at Dust2 scale (#1884's review), measured by hand in a release
//! build: `cargo test --release --features dev --lib denoised_dust2_bake_cost --
//! --ignored --nocapture`. A measurement, never a gate. The level is the lightmap
//! cost bench's Dust2-sized one (6344 meshes, 2400 lamps), baked whole at the
//! default samples: the trace alone, OIDN on a device per worker (what a bake
//! lends), OIDN on one shared device of every core (this branch's first design),
//! and the Gaussian at the old default of 128. It prints each bake's wall time,
//! OIDN's time summed over its devices, and the charts it cleaned.
//!
//! At [`DENSITY`], not the default 8: the chart count, which is what a per-chart
//! denoise pays for, is the same at any density, smaller charts make OIDN's cost
//! per call weigh more, and at 8 texels a unit the level has about 16 times the
//! texels, an hour a bake on an 8-core machine.

use std::sync::Arc;
use std::time::Instant;

use super::{available, Oidn};
use crate::bake::lightmap_cost_tests::lit_level;
use rusty_lighting::lightmap::{bake_with_progress, lightmap_size, BakeProgress};
use rusty_scene::lighting::settings::{max_map_edge, BakeSettings, Denoiser};

/// Lightmap texels per world unit the level is baked at.
const DENSITY: f32 = 2.0;

/// One bake: its name, samples, denoiser, and how many OIDN devices it is lent.
type Row = (&'static str, u32, Denoiser, usize);

#[test]
// lint: allow(skip) a release-build measurement of a Dust2-sized bake, run by hand
#[ignore = "a release-build measurement, run by hand"]
fn denoised_dust2_bake_cost() {
    available().expect("this measurement needs OIDN");
    let scene = lit_level(20);
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let base = BakeSettings {
        texels_per_unit: DENSITY,
        ..Default::default()
    };
    let max = max_map_edge(base.page());
    let sizes = scene.meshes.iter().filter(|m| m.receives_lightmap());
    let sizes: Vec<u32> = sizes
        .filter_map(|m| lightmap_size(m, DENSITY * m.scale_in_lightmap, max))
        .collect();
    let texels: u64 = sizes.iter().map(|s| u64::from(s * s)).sum();
    println!(
        "Dust2-sized at {DENSITY} texels/unit: {} meshes, {} charts, {texels} texels, {cores} cores",
        scene.meshes.len(),
        sizes.len()
    );
    println!("| bake | spp | wall | OIDN, summed over devices | charts denoised |");
    let samples = BakeSettings::DEFAULT_SAMPLES;
    let rows: [Row; 4] = [
        ("trace alone", samples, Denoiser::None, 0),
        ("OIDN, a device per worker", samples, Denoiser::Oidn, cores),
        ("OIDN, one shared device", samples, Denoiser::Oidn, 1),
        ("Gaussian (old default)", 128, Denoiser::Filter, 0),
    ];
    for (name, samples, denoiser, devices) in rows {
        let settings = BakeSettings {
            samples,
            denoiser,
            ..base
        };
        let oidn = Arc::new(Oidn::with_devices(devices));
        let progress = match devices {
            0 => BakeProgress::default(),
            _ => BakeProgress::default().with_denoiser(oidn.clone()),
        };
        let t = Instant::now();
        let maps = bake_with_progress(&scene, &settings, &progress).expect("not cancelled");
        let wall = t.elapsed().as_secs_f64();
        let (charts, busy) = oidn.spent();
        let busy = busy.as_secs_f64();
        assert_eq!(maps.len(), sizes.len(), "every chart baked");
        println!("| {name} | {samples} | {wall:.1} s | {busy:.1} s | {charts} |");
    }
}
