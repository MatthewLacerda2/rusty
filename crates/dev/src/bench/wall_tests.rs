use super::stress::{self, StressSpec};
use super::*;

#[test]
fn gpu_wall_ms_holds_the_ticks_the_renderers_and_the_gpus_time_each_frame() {
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
    measure(&mut h, 2);
    let probe = h.renderer().and_then(|r| ClockProbe::new(r));
    let (_, samples) = measure_posed(&mut h, 4, probe.as_ref(), &mut |_| {});
    let wall = samples.values("wall_ms");
    if wall.is_empty() {
        assert!(
            !rusty_render::gpu_required(),
            "no adapter, but one is required"
        );
        return;
    }
    let frame = samples.values("frame_ms");
    let renderer = samples.values("renderer_ms");
    assert_eq!((wall.len(), renderer.len(), frame.len()), (4, 4, 4));
    let gpu = samples.values("gpu_raw_ms");
    for (i, &w) in wall.iter().enumerate() {
        assert!(w >= frame[i], "frame {i}: wall {w} < tick {}", frame[i]);
        assert!(
            w >= renderer[i],
            "frame {i}: wall {w} < renderer {}",
            renderer[i]
        );
        if gpu.len() == wall.len() {
            assert!(w >= gpu[i], "frame {i}: wall {w} < gpu raw {}", gpu[i]);
        }
    }
}

#[test]
fn wall_ms_leaves_out_only_what_this_tick_recorded_beside_frame_ms() {
    let mut stats = FrameStats::default();
    stats.record("taps_ms", 3.0);
    stats.end_tick();
    let before = beside_counts(&stats);
    stats.record("loads_ms", 2.0);
    stats.record("probe_ms", 0.5);
    stats.end_tick();
    assert_eq!(
        beside_ms(&stats, before),
        2.5,
        "the stale taps_ms is not this tick's"
    );
    let mut fresh = FrameStats::default();
    fresh.record("taps_ms", 1.0);
    fresh.end_tick();
    assert_eq!(
        beside_ms(&fresh, (5, [1, 0, 0])),
        1.0,
        "a restart counts every row"
    );
}
