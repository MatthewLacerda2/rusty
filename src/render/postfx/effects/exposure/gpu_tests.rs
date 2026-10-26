//! The exposure meter on lavapipe (#1444): a synthetic grey scene metered and
//! adapted step by step, read back exactly after each.

use super::params::Step;
use super::ExposureParams;
use crate::components::{AutoExposure, ExposureMode};
use crate::render::postfx::grade::gpu::{chain, exposure, Chain, W};

/// A grey scene of luminance 2^-6 meters bin 12 of 64, centred at -6.09375:
/// exposing it to middle grey (0.18) takes log2(0.18) + 6.09375 EV.
const DARK: f32 = 0.015625;
const DARK_TARGET: f32 = 3.619_819;
/// Luminance 4 meters bin 38, centred at 2.03125.
const BRIGHT: f32 = 4.0;
const BRIGHT_TARGET: f32 = -4.505_181;

fn scene(chain: &mut Chain, grey: f32) {
    chain.write_row(&[[grey; 3]; W as usize]);
}

/// Meter the chain's scene with `ae`, adapting by `step`, and read the EV back.
fn meter(chain: &mut Chain, ae: &AutoExposure, step: Step) -> f32 {
    let Chain { r, pfx, .. } = chain;
    let params = ExposureParams::new(ae, step);
    let gpu = (&r.device, &r.queue);
    pfx.exposure
        .run(gpu, &r.post_passes.exposure, &pfx.scene_hdr.view, &params);
    pfx.exposure.settle(&r.device);
    pfx.exposure.readout().expect("the meter read back")
}

const SNAP: Step = Step {
    dt: 0.0,
    snap: true,
    cut: true,
};

fn tick(dt: f32) -> Step {
    Step {
        dt,
        snap: false,
        cut: false,
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn gpu_a_dark_scene_adapts_brighter_and_settles_inside_the_limits() {
    let Some(mut c) = chain() else { return };
    let ae = AutoExposure::default();
    scene(&mut c, BRIGHT);
    let start = meter(&mut c, &ae, SNAP);
    assert!(near(start, BRIGHT_TARGET), "snapped to {start}");

    scene(&mut c, DARK);
    let mut ev = start;
    for _ in 0..4 {
        let next = meter(&mut c, &ae, tick(0.5));
        assert!(near(next, ev + 0.5 * ae.speed_up), "{ev} -> {next}");
        ev = next;
    }
    assert!(
        near(meter(&mut c, &ae, tick(0.0)), ev),
        "a paused game holds"
    );
    for _ in 0..20 {
        ev = meter(&mut c, &ae, tick(0.5));
    }
    assert!(near(ev, DARK_TARGET), "settled at {ev}");

    let capped = AutoExposure { max_ev: 2.0, ..ae };
    let ev = meter(&mut c, &capped, tick(5.0));
    assert!(near(ev, 2.0), "held to max_ev: {ev}");
    assert!((ae.min_ev..=ae.max_ev).contains(&ev));
}

#[test]
fn gpu_a_cut_snaps_and_darkening_runs_at_its_own_speed() {
    let Some(mut c) = chain() else { return };
    let ae = AutoExposure::default();
    scene(&mut c, DARK);
    meter(&mut c, &ae, SNAP);
    scene(&mut c, BRIGHT);
    let ev = meter(&mut c, &ae, tick(0.25));
    assert!(near(ev, DARK_TARGET - 0.25 * ae.speed_down), "{ev}");
    assert!(near(meter(&mut c, &ae, SNAP), BRIGHT_TARGET), "a cut snaps");

    // A cut forgets the readout, and the read in flight across it.
    let Chain { r, pfx, .. } = &mut c;
    let params = ExposureParams::new(&ae, SNAP);
    pfx.exposure.run(
        (&r.device, &r.queue),
        &r.post_passes.exposure,
        &pfx.scene_hdr.view,
        &params,
    );
    pfx.exposure.cut();
    pfx.exposure.settle(&r.device);
    assert_eq!(pfx.exposure.readout(), None);
}

#[test]
fn gpu_the_same_ticks_give_the_same_exposure_twice() {
    let run = || {
        let mut c = chain()?;
        let ae = AutoExposure::default();
        let mut evs = Vec::new();
        for (i, grey) in [BRIGHT, DARK, DARK, 0.3, DARK].into_iter().enumerate() {
            scene(&mut c, grey);
            let step = if i == 0 { SNAP } else { tick(0.4) };
            evs.push(meter(&mut c, &ae, step).to_bits());
        }
        Some(evs)
    };
    let Some(first) = run() else { return };
    assert_eq!(Some(first), run());
}

#[test]
fn gpu_the_composite_applies_the_adapted_exposure() {
    let Some(mut c) = chain() else { return };
    scene(&mut c, DARK);
    let ev = meter(&mut c, &AutoExposure::default(), SNAP);
    // The fixed run first: it bakes the grading LUT both draws read.
    let fixed = c.run(&[[DARK; 3]; W as usize], ev, None);
    let metered = c.composite(&exposure(0.0, ExposureMode::Automatic));
    assert_eq!(metered, fixed, "Automatic draws as Fixed at the adapted EV");
    let unexposed = c.run(&[[DARK; 3]; W as usize], 0.0, None);
    assert!(
        metered[0][0] > unexposed[0][0],
        "the dark scene was brightened"
    );
}
