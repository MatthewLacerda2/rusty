//! Render counters in the frame stats (#433): a harness screenshot folds what the
//! frame submitted — draw calls, triangles, visible entities, shadows — and the
//! renderer's CPU time into the run's stats.

use rusty::dev::harness::Harness;

#[test]
fn a_screenshot_records_render_counters() {
    let out = std::env::temp_dir().join(format!("rusty_stats_gpu_{}", std::process::id()));
    let mut h = Harness::new(&out, "");
    h.step(2);
    if !h.screenshot(out.join("shot.png")) {
        eprintln!("[stats] no GPU/software adapter — skipping render counters");
        return;
    }
    let stats = h.stats.borrow().clone();
    let max = |k: &str| stats.get(k).map_or(0.0, |s| s.max);
    assert!(max("draw_calls") > 0.0 && max("triangles") > 0.0);
    // The demo scene has meshes but no light entities (light counting is unit-tested).
    assert!(max("visible_entities") > 0.0 && stats.get("lights").is_some());
    assert!(stats.get("renderer_ms").is_some());
    // Instancing (#470) folds copies of a mesh into one call, so draws no longer bound
    // entities from above — but every caster draw is still a draw.
    assert!(max("draw_calls") >= max("shadow_draws") + 1.0);
    let _ = std::fs::remove_dir_all(out);
}
