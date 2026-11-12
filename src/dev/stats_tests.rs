use super::*;
use crate::render::{GpuPass, GpuTimes, RenderCounters};

#[test]
fn stats_table_has_frames_metrics_and_systems() {
    let mut stats = FrameStats {
        frames: 2,
        ..Default::default()
    };
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
        gpu: Some(GpuTimes { passes }),
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

#[test]
fn a_load_timer_reports_a_swapped_in_load_and_drops_a_failed_one() {
    let stats = Rc::new(RefCell::new(FrameStats::default()));
    let mut timer = LoadTimer::new(Rc::clone(&stats));
    let mut console = ConsoleLogs::new();
    let mut ran = 0;
    timer.step("read", &mut || ran += 1);
    timer.finish(None, &mut console);
    assert!(stats.borrow().metrics.is_empty() && console.messages.is_empty());
    timer.step("read", &mut || ran += 1);
    timer.step("physics", &mut || ran += 1);
    timer.finish(Some("a.scene"), &mut console);
    assert_eq!(ran, 3, "each step ran once");
    let stats = stats.borrow();
    let keys: Vec<&str> = stats.metrics.keys().map(String::as_str).collect();
    assert_eq!(keys, ["load_ms", "load_physics_ms", "load_read_ms"]);
    let line = &console.messages[0].0;
    assert!(
        line.starts_with("Scene load a.scene: ") && line.contains("(read "),
        "{line}"
    );
}
