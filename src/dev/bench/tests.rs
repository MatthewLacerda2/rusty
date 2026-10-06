use std::cell::RefCell;
use std::rc::Rc;

use super::stress::{self, Spawned, StressSpec};
use super::*;

fn harness(name: &str) -> Harness {
    let out = crate::test_temp::dir().join(format!("bench_{name}_{}", std::process::id()));
    Harness::new(out, "")
}

#[test]
fn the_default_stress_scene_is_the_shooter_shaped_worst_case() {
    let h = harness("load");
    let world = h.world.borrow();
    let mut scene = world.scene().borrow_mut();
    let spawned = stress::load(&mut scene, &StressSpec::default());
    let want = Spawned {
        enemies: 50,
        lights: 32,
        decals: 256,
        particle_systems: 4,
    };
    assert_eq!(spawned, want);
    // Every soldier is a skinned, animated nav agent, standing on the deck.
    let soldiers: Vec<u32> = (0..50)
        .map(|i| scene.find_entity_by_name(&format!("Soldier_{i}")).unwrap())
        .collect();
    for &id in &soldiers {
        let w = &scene.world;
        assert!(w.mesh(id).is_some_and(|m| m.skin.is_some()));
        assert!(w.animator(id).is_some() && w.nav_agent(id).is_some());
    }
    // Spots never repeat, so no two soldiers start inside each other.
    let mut spots: Vec<_> = (0..50)
        .map(|i| stress::soldier_spot(i).to_array())
        .collect();
    spots.sort_by(|a, b| a.partial_cmp(b).unwrap());
    spots.dedup();
    assert_eq!(spots.len(), 50);
}

#[test]
fn an_unknown_load_stress_option_is_an_error() {
    let lua = mlua::Lua::new();
    let h = Rc::new(RefCell::new(harness("opts")));
    let t = lua.create_table().unwrap();
    register(&lua, &h, &t).unwrap();
    lua.globals().set("Harness", t).unwrap();
    let err = lua
        .load("Harness.LoadStress{ enemys = 3 }")
        .exec()
        .unwrap_err();
    assert!(err.to_string().contains("unknown option `enemys`"), "{err}");
    let n: u32 = lua
        .load("return Harness.LoadStress{ enemies = 2, lights = 0, decals = 1 }.enemies")
        .eval()
        .unwrap();
    assert_eq!(n, 2);
}

/// A small stress scene measured for real: the rows a report always carries, and
/// GPU rows exactly when the adapter has timestamps.
#[test]
fn gpu_bench_reports_frame_render_and_counter_rows() {
    let mut h = harness("measure");
    let spec = StressSpec {
        enemies: 2,
        lights: 4,
        decals: 8,
        particle_systems: 1,
        ..StressSpec::default()
    };
    stress::load(&mut h.world.borrow().scene().borrow_mut(), &spec);
    let report = measure(&mut h, 3);
    assert_eq!(report.frames, 3);
    let row = |k: &str| report.metrics.iter().find(|(m, _)| m == k).map(|r| r.1);
    assert!(row("frame_ms").is_some() && row("update_scripts").is_some());
    // The clock hold and the frame share the harness's renderer: without one both
    // skip, which is only allowed where no GPU is required (#885).
    let rendered = h.renderer().is_some();
    assert_eq!(row("renderer_ms").is_some(), rendered);
    if !rendered {
        assert!(
            !crate::render::gpu_required(),
            "no adapter, but one is required"
        );
        return;
    }
    assert!(row("draw_calls").is_some_and(|s| s.avg > 0.0));
    assert!(row("triangles").is_some_and(|s| s.avg > 0.0));
    let timed = h.stats.borrow().get("gpu_ms").is_some();
    assert_eq!(row("gpu_ms").is_some(), timed);
    if timed {
        assert!(row("gpu.forward").is_some_and(|s| s.avg > 0.0));
        assert!(row("gpu.ribbons").is_some(), "every pass has a row");
        assert!(row("gpu_raw_ms").is_some_and(|s| s.avg > 0.0));
        assert!(row("gpu_clock_pct").is_some_and(|s| s.avg > 0.0));
    }
}

/// GPU rows are the frame's times scaled by the clock speed the probe read; the
/// raw total and the clock get rows of their own. A frame the probe missed has no
/// GPU rows, so an unscaled time never mixes in.
#[test]
fn gpu_rows_are_scaled_by_the_frame_clock_speed() {
    use crate::render::{GpuPass, GpuTimes};
    let gpu = GpuTimes {
        passes: vec![(GpuPass::Shadows, 2.0), (GpuPass::Forward, 6.0)],
    };
    let frame = RenderedFrame {
        gpu: Some(gpu),
        ..RenderedFrame::default()
    };
    let stats = FrameStats::default();
    let mut samples = Samples::default();
    sample(&mut samples, &stats, Some(&frame), Some(0.5));
    sample(&mut samples, &stats, Some(&frame), None);
    let report = samples.report();
    let avg = |k: &str| report.metrics.iter().find(|(m, _)| m == k).unwrap().1.avg;
    assert_eq!(avg("gpu_ms"), 4.0);
    assert_eq!(avg("gpu.shadows"), 1.0);
    assert_eq!(avg("gpu.forward"), 3.0);
    assert_eq!(avg("gpu.ui"), 0.0);
    assert_eq!(avg("gpu_raw_ms"), 8.0);
    assert_eq!(avg("gpu_clock_pct"), 50.0);
}
