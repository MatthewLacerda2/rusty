use std::cell::RefCell;
use std::rc::Rc;

use super::stress::{self, Spawned, StressSpec};
use super::*;

fn harness(name: &str) -> Harness {
    let out = rusty_test_support::temp::dir().join(format!("bench_{name}_{}", std::process::id()));
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
    assert!(row("frame_ms").is_some() && row("cpu.update_scripts").is_some());
    assert!(row("cpu_ms").is_some() && row("cpu_clock_pct").is_some_and(|s| s.avg > 0.0));
    // The clock hold and the frame share the harness's renderer: without one both
    // skip, which is only allowed where no GPU is required (#885).
    let rendered = h.renderer().is_some();
    assert_eq!(row("renderer_ms").is_some(), rendered);
    if !rendered {
        assert!(
            !rusty_render::gpu_required(),
            "no adapter, but one is required"
        );
        return;
    }
    // A frame draws, begins passes, submits and writes buffers (#2341).
    for k in [
        "draw_calls",
        "triangles",
        "render_passes",
        "submits",
        "buffer_writes",
    ] {
        assert!(row(k).is_some_and(|s| s.avg > 0.0), "{k}");
    }
    for k in [
        "hiz_tested",
        "hiz_culled",
        "hiz_late",
        "cluster_tested",
        "cluster_culled",
    ] {
        assert!(row(k).is_some(), "{k}");
    }
    assert!(row("compute_passes").is_some() && row("bind_groups_created").is_some());
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
/// raw total and the clock get rows of their own, and the GPU's wait for the CPU
/// stays as measured. A frame the probe missed has no GPU rows, so an unscaled time
/// never mixes in.
#[test]
fn gpu_rows_are_scaled_by_the_frame_clock_speed() {
    use rusty_render::{GpuPass, GpuTimes};
    let gpu = GpuTimes {
        frame: 1,
        passes: vec![(GpuPass::Shadows, 2.0), (GpuPass::Forward, 6.0)],
        other_ms: 0.4,
        between_submits_ms: 0.6,
        ..Default::default()
    };
    let frame = RenderedFrame {
        gpu: Some(gpu),
        ..RenderedFrame::default()
    };
    let stats = FrameStats::default();
    let mut samples = Samples::default();
    let half = Speeds {
        gpu: Some(0.5),
        ..Speeds::default()
    };
    sample(&mut samples, &stats, Some(&frame), half);
    sample(&mut samples, &stats, Some(&frame), Speeds::default());
    let report = samples.report();
    let avg = |k: &str| report.metrics.iter().find(|(m, _)| m == k).unwrap().1.avg;
    assert_eq!(avg("gpu_ms"), 4.0);
    assert_eq!(avg("gpu.shadows"), 1.0);
    assert_eq!(avg("gpu.forward"), 3.0);
    assert_eq!(avg("gpu.ui"), 0.0);
    // Untimed work is GPU work, scaled like the rows; the wait for the CPU is not.
    assert_eq!(avg("gpu.other"), 0.2);
    assert_eq!(avg("gpu_between_submits_ms"), 0.6, "a wait as measured");
    // The `gpu.` rows, `other` included, are the GPU's work at the reference clock.
    let rows = report.metrics.iter().filter(|(m, _)| m.starts_with("gpu."));
    let work: f64 = rows.map(|(_, s)| s.avg).sum();
    assert!((work - 4.2).abs() < 1e-9, "gpu. rows {work} vs 4.2");
    assert_eq!(avg("gpu_raw_ms"), 8.0);
    assert_eq!(avg("gpu_clock_pct"), 50.0);
    // Without timestamps (lavapipe) no GPU row, `gpu.other` included.
    let mut untimed = Samples::default();
    sample(&mut untimed, &stats, Some(&RenderedFrame::default()), half);
    assert!(!untimed
        .report()
        .metrics
        .iter()
        .any(|(m, _)| m.starts_with("gpu")));
}

/// CPU rows mirror the GPU's (#1719): `cpu_ms` and `cpu.<system>` are scaled, `frame_ms`
/// stays as measured, and a frame the CPU probe missed has no scaled rows.
#[test]
fn cpu_rows_are_scaled_by_the_cpu_clock_speed() {
    let mut stats = FrameStats::default();
    stats.record("frame_ms", 4.0);
    stats.record_system("tick_nav", 2.0);
    let mut samples = Samples::default();
    let cpu = |cpu| Speeds { cpu, gpu: None };
    sample(&mut samples, &stats, None, cpu(Some(0.5)));
    sample(&mut samples, &stats, None, cpu(None));
    assert_eq!(samples.values("frame_ms"), [4.0, 4.0]);
    assert_eq!(samples.values("cpu_ms"), [2.0]);
    assert_eq!(samples.values("cpu.tick_nav"), [1.0]);
    assert_eq!(samples.values("cpu_clock_pct"), [50.0]);
    assert!(samples.values("tick_nav").is_empty(), "no raw system rows");
}

/// One frame's report rows, sampled at the reference clocks.
fn rows_of(frame: &RenderedFrame) -> Vec<(String, f64)> {
    let speed = Speeds {
        gpu: Some(1.0),
        ..Speeds::default()
    };
    let mut samples = Samples::default();
    sample(&mut samples, &FrameStats::default(), Some(frame), speed);
    let rows = samples.report().metrics.into_iter();
    rows.map(|(m, s)| (m, s.avg)).collect()
}

/// The Hi-Z stage's parts (#2401): with the cull on, `gpu.occlusion.<part>` and
/// `render.occlusion.<part>` sit beside their stage's row, which stays the
/// stage's whole time.
#[test]
fn the_occlusion_parts_have_rows_inside_their_stage_s() {
    use rusty_render::{GpuPart, GpuPass, GpuTimes};
    let parts: Vec<_> = GpuPart::ALL.iter().map(|&p| (p, 1.0)).collect();
    let gpu = GpuTimes {
        passes: vec![(GpuPass::Occlusion, 5.0), (GpuPass::Forward, 2.0)],
        parts,
        ..GpuTimes::default()
    };
    let stages = vec![
        ("solids", 1.0),
        ("occlusion.test", 0.5),
        ("occlusion.select", 0.25),
        ("occlusion.test", 0.5),
        ("occlusion", 0.25),
        ("scene", 2.0),
    ];
    let on = RenderedFrame {
        cpu_ms: 4.5,
        stages,
        gpu: Some(gpu),
        ..RenderedFrame::default()
    };
    let rows = rows_of(&on);
    let avg = |k: &str| rows.iter().find(|(m, _)| m == k).map(|(_, v)| *v);
    assert_eq!(avg("gpu.occlusion"), Some(5.0));
    for part in ["select", "phase_one", "pyramid", "test", "clusters"] {
        assert_eq!(avg(&format!("gpu.occlusion.{part}")), Some(1.0), "{part}");
    }
    let passes = rows.iter().filter(|(m, _)| m.matches('.').count() == 1);
    let passes: f64 = passes
        .filter(|(m, _)| m.starts_with("gpu."))
        .map(|(_, v)| v)
        .sum();
    assert_eq!(passes, 7.0, "a part is in its pass's row, not beside it");
    assert_eq!(avg("render.occlusion"), Some(1.5), "the stage's whole time");
    assert_eq!(
        avg("render.occlusion.test"),
        Some(1.0),
        "a part's laps summed"
    );
    assert_eq!(avg("render.occlusion.select"), Some(0.25));
    assert_eq!(avg("render.other"), Some(0.0), "nothing counted twice");
}

/// With the cull off the stage laps whole and no part is timed: no part rows.
#[test]
fn the_occlusion_parts_have_no_rows_with_the_cull_off() {
    let off = RenderedFrame {
        cpu_ms: 3.0,
        stages: vec![("occlusion", 0.0), ("scene", 3.0)],
        gpu: Some(rusty_render::GpuTimes::default()),
        ..RenderedFrame::default()
    };
    let rows = rows_of(&off);
    assert!(rows.iter().any(|(m, _)| m == "render.occlusion"));
    assert!(!rows.iter().any(|(m, _)| m.starts_with("gpu.occlusion.")));
    assert!(!rows.iter().any(|(m, _)| m.starts_with("render.occlusion.")));
}

/// `cpu_ms` is the whole tick (#2342): the systems' rows, `cpu.flush_bounds` and
/// `cpu.other` (what no system held) fold back to it; `taps_ms`, `probe_ms` beside.
#[test]
fn cpu_rows_fold_to_the_timed_tick() {
    let mut stats = FrameStats::default();
    for (key, ms) in [("frame_ms", 5.0), ("taps_ms", 1.0), ("probe_ms", 0.5)] {
        stats.record(key, ms); // a demo's taps and the probe's own work: beside the tick
    }
    for (name, ms) in [
        ("tick_nav", 2.0),
        ("flush_bounds", 0.25),
        ("flush_bounds", 0.25),
    ] {
        stats.record_system(name, ms);
    }
    let mut samples = Samples::default();
    let at_reference = Speeds {
        cpu: Some(1.0),
        gpu: None,
    };
    sample(&mut samples, &stats, None, at_reference);
    let flush = samples.values("cpu.flush_bounds");
    assert_eq!(flush, [0.5], "summed over the stages");
    let other = samples.values("cpu.other");
    assert_eq!(other, [2.5], "the tick less what its systems held");
    let parts = ["cpu.tick_nav", "cpu.flush_bounds", "cpu.other"];
    let sum: f64 = parts.iter().map(|k| samples.values(k)[0]).sum();
    assert_eq!(
        sum,
        samples.values("cpu_ms")[0],
        "the taps are in no cpu. row"
    );
    let beside = ["taps_ms", "probe_ms"].map(|k| samples.values(k));
    assert_eq!(beside, [[1.0], [0.5]]);
}
