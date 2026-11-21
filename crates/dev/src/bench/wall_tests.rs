//! `wall_ms` (#2494): each measured frame end to end on the wall, its sim step
//! and its draw until the GPU finished.

use super::stress::{self, StressSpec};
use super::*;

/// A primitive scene measured for real: a frame on the wall holds the renderer's
/// CPU time and the GPU's raw time, since the draw waits for the GPU to finish.
#[test]
fn gpu_wall_ms_holds_the_renderers_time_and_the_gpus() {
    let out = rusty_test_support::temp::dir().join(format!("bench_wall_{}", std::process::id()));
    let mut h = Harness::new(out, "");
    let spec = StressSpec {
        enemies: 1,
        lights: 2,
        decals: 2,
        particle_systems: 0,
        ..StressSpec::default()
    };
    stress::load(&mut h.world.borrow().scene().borrow_mut(), &spec);
    let report = measure(&mut h, 4);
    let row = |k: &str| report.metrics.iter().find(|(m, _)| m == k).map(|r| r.1);
    let wall = row("wall_ms").expect("every measured frame has a wall time");
    let frame = row("frame_ms").expect("a stepped frame has a tick time");
    assert!(
        wall.avg >= frame.avg,
        "wall {} < frame {}",
        wall.avg,
        frame.avg
    );
    let Some(renderer) = row("renderer_ms") else {
        assert!(
            !rusty_render::gpu_required(),
            "no adapter, but one is required"
        );
        return;
    };
    assert!(
        wall.avg >= renderer.avg,
        "wall {} < renderer {}",
        wall.avg,
        renderer.avg
    );
    if let Some(gpu) = row("gpu_raw_ms") {
        assert!(
            wall.avg >= gpu.avg,
            "wall {} < gpu raw {}",
            wall.avg,
            gpu.avg
        );
    }
}
