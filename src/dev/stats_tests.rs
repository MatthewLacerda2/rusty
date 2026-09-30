use super::*;

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
    record_render(&mut stats, &counters, 1.5);
    assert_eq!(stats.get("draw_calls").map(|s| s.max), Some(3.0));
    assert_eq!(stats.get("renderer_ms").map(|s| s.last), Some(1.5));
}
