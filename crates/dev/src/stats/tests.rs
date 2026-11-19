use super::*;
use rusty_render::{GpuPass, GpuTimes, RenderCounters};

#[test]
fn stats_table_has_frames_metrics_and_systems() {
    let mut stats = FrameStats::default();
    stats.frames = 2;
    stats.record("draw_calls", 7.0);
    stats.record_system("tick_nav", 0.25);
    let lua = mlua::Lua::new();
    lua.globals()
        .set("s", to_lua(&lua, &stats).unwrap())
        .unwrap();
    let got: (u64, f64, f64) = lua
        .load("return s.frames, s.draw_calls.max, s.systems.tick_nav.last")
        .eval()
        .unwrap();
    assert_eq!(got, (2, 7.0, 0.25));
}

#[test]
fn budgets_come_back_sorted_by_metric() {
    let lua = mlua::Lua::new();
    let t: mlua::Table = lua
        .load("return { update_ms = 4, draw_calls = 2000, entities = 50 }")
        .eval()
        .unwrap();
    let names: Vec<String> = budgets_from_lua(t)
        .unwrap()
        .into_iter()
        .map(|b| b.0)
        .collect();
    assert_eq!(names, ["draw_calls", "entities", "update_ms"]);
}

#[test]
fn render_counters_fold_in_with_the_renderer_time() {
    let mut stats = FrameStats::default();
    let counters = RenderCounters {
        draw_calls: 3,
        ..Default::default()
    };
    let frame = RenderedFrame {
        counters,
        cpu_ms: 1.5,
        stages: Vec::new(),
        gpu: None,
    };
    record_render(&mut stats, &frame);
    assert_eq!(stats.get("draw_calls").map(|s| s.max), Some(3.0));
    assert_eq!(stats.get("renderer_ms").map(|s| s.last), Some(1.5));
    // No timestamps: no GPU key at all, in the stats, the Lua table or the JSON.
    assert!(stats.get("gpu_ms").is_none() && stats.gpu_passes.is_empty());
    let lua = mlua::Lua::new();
    let t = to_lua(&lua, &stats).unwrap();
    assert!(t.get::<mlua::Value>("gpu_ms").unwrap().is_nil());
    assert!(t.get::<mlua::Value>("gpu_passes").unwrap().is_nil());
    assert!(stats.to_json(true).get("gpu_passes").is_none());
}

#[test]
fn gpu_times_fold_in_as_a_total_and_per_pass() {
    let mut stats = FrameStats::default();
    let passes = vec![(GpuPass::Shadows, 0.5), (GpuPass::Forward, 2.0)];
    let frame = RenderedFrame {
        gpu: Some(GpuTimes {
            frame: 1,
            passes,
            other_ms: 0.0,
            between_submits_ms: 0.0,
            ..Default::default()
        }),
        ..Default::default()
    };
    record_render(&mut stats, &frame);
    assert_eq!(stats.get("gpu_ms").map(|s| s.last), Some(2.5));
    let lua = mlua::Lua::new();
    lua.globals()
        .set("s", to_lua(&lua, &stats).unwrap())
        .unwrap();
    let got: (f64, f64) = lua
        .load("return s.gpu_passes.shadows.last, s.gpu_passes.forward.avg")
        .eval()
        .unwrap();
    assert_eq!(got, (0.5, 2.0));
    // Timings stay out of the replay-stable JSON.
    assert!(stats.to_json(false).get("gpu_passes").is_none());
    assert!(stats.to_json(true)["gpu_passes"]["forward"].is_object());
}

/// `frame_ms` is the whole tick (#2342), not its stages' sum: it holds at least the
/// stages, `flush_bounds` is a system row, and an edit tick records no frame.
#[test]
fn frame_ms_times_the_whole_tick() {
    let (mut gw, stats, _) = super::stats_load_tests::world_with_level("whole_tick");
    gw.tick(1.0 / 60.0);
    assert_eq!(stats.borrow().frames, 0, "an edit tick is no frame");
    gw.set_playing(true);
    gw.tick(1.0 / 60.0);
    gw.tick(1.0 / 60.0);
    let stats = stats.borrow();
    let last = |k: &str| stats.get(k).map(|s| s.last).unwrap_or_default();
    let stages = [
        "fixed_update_ms",
        "update_ms",
        "late_update_ms",
        "render_stage_ms",
    ];
    let staged: f64 = stages.iter().map(|k| last(k)).sum();
    assert_eq!(stats.get("frame_ms").map(|s| s.samples), Some(2));
    assert!(
        last("frame_ms") >= staged,
        "{} < {staged}",
        last("frame_ms")
    );
    assert!(stats.tick_systems.iter().any(|(n, _)| *n == "flush_bounds"));
}
