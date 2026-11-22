use std::sync::Arc;
use std::time::Instant;

use super::{available, Oidn};
use crate::bake::lightmap_cost_tests::lit_level;
use rusty_lighting::lightmap::max_map_edge;
use rusty_lighting::lightmap::{bake_with_progress, lightmap_size, BakeProgress};
use rusty_scene::lighting::settings::{BakeSettings, Denoiser};

/// Lightmap texels per unit, below the default 8: the chart count a per-chart denoise pays
/// for is the same at any density, and 8 bakes 16 times the texels.
const DENSITY: f32 = 2.0;

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
