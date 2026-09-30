//! GPU instancing's regression guard (#470): a prop-heavy level — 600 copies of three
//! meshes over the demo scene — must render in a handful of draw calls, not one per
//! prop. The budget is a draw-call count, deterministic on every adapter; the CPU time
//! is printed for measurement only (`--nocapture`). `RUSTY_BENCH_FRAMES` renders more
//! frames for a steadier average.

use rusty::dev::harness::Harness;

/// 600 props (crates, barrels, pillars) on a 20×30 grid, sharing the default material.
const PROPS: &str = r#"
local kinds = { "Box", "Cylinder", "Sphere" }
local n = 0
for x = -10, 9 do
    for z = -15, 14 do
        n = n + 1
        local id = Scene.CreateEntity("Prop_" .. n, kinds[(n % 3) + 1])
        Transform.SetPosition(id, x * 1.5, 0.5, z * 1.5)
        Transform.SetScale(id, 0.6, 0.6, 0.6)
    end
end
"#;

/// Per-prop draws put this far out of reach: the visible props plus their casters
/// alone are several hundred calls.
const DRAW_CALL_BUDGET: f64 = 100.0;

#[test]
fn a_prop_heavy_level_renders_in_few_draw_calls() {
    let out = std::env::temp_dir().join(format!("rusty_instancing_{}", std::process::id()));
    let mut h = Harness::new(&out, "");
    h.step(1); // the Lua runtime exists once play has begun
    h.world
        .borrow()
        .script_manager()
        .eval(PROPS)
        .expect("props spawn");
    h.step(2);
    let frames: usize = std::env::var("RUSTY_BENCH_FRAMES")
        .ok()
        .and_then(|f| f.parse().ok())
        .unwrap_or(1);
    for _ in 0..frames {
        if !h.screenshot(out.join("props.png")) {
            eprintln!("[instancing] no GPU/software adapter — skipping");
            return;
        }
    }
    let stats = h.stats.borrow().clone();
    let get = |k: &str| stats.get(k).map_or((0.0, 0.0), |s| (s.max, s.avg));
    eprintln!(
        "[instancing] draw_calls {} shadow_draws {} visible {} triangles {} renderer_ms avg {:.2}",
        get("draw_calls").0,
        get("shadow_draws").0,
        get("visible_entities").0,
        get("triangles").0,
        get("renderer_ms").1,
    );
    assert!(
        get("visible_entities").0 >= 300.0,
        "most props are on screen"
    );
    assert!(h.assert_budget(&[("draw_calls".to_string(), DRAW_CALL_BUDGET)]));
    let _ = std::fs::remove_dir_all(out);
}
