//! GPU instancing's regression guard (#470): a prop-heavy level — 600 copies of three
//! meshes over the demo scene — must render in a handful of draw calls, not one per
//! prop. The budget is a draw-call count, deterministic on every adapter; the CPU time
//! is printed for measurement only (`--nocapture`). `RUSTY_BENCH_FRAMES` renders more
//! frames for a steadier average.
//!
//! The props are also tinted one by one (#1706): `Material.SetColor` gives each its
//! own library material, and equal records must still share a draw and a caster run.

use rusty::dev::harness::Harness;

/// 600 props (crates, barrels, pillars) on a 20×30 grid, sharing the default material
/// unless a global `tint` is set.
const PROPS: &str = r#"
local kinds = { "Box", "Cylinder", "Sphere" }
local n = 0
for x = -10, 9 do
    for z = -15, 14 do
        n = n + 1
        local id = Scene.CreateEntity("Prop_" .. n, kinds[(n % 3) + 1])
        Transform.SetPosition(id, x * 1.5, 0.5, z * 1.5)
        Transform.SetLocalScale(id, 0.6, 0.6, 0.6)
        if tint then Material.SetColor(id, tint) end
    end
end
"#;

/// Run before [`PROPS`]: each prop is then tinted one grey through its own material
/// (`entity_<id>_material`).
const TINTED: &str = "tint = { 0.5, 0.5, 0.5 }";

/// Per-prop draws put this far out of reach: the visible props plus their casters
/// alone are several hundred calls.
const DRAW_CALL_BUDGET: f64 = 100.0;

#[test]
fn a_prop_heavy_level_renders_in_few_draw_calls() {
    check_budget("default", PROPS);
}

#[test]
fn props_tinted_one_colour_each_still_share_their_draws() {
    check_budget("tinted", &format!("{TINTED}\n{PROPS}"));
}

/// Spawn `script`'s props, render, and hold the camera's and the shadows' draw calls
/// within [`DRAW_CALL_BUDGET`].
fn check_budget(name: &str, script: &str) {
    let out = crate::temp::dir().join(format!("rusty_instancing_{name}_{}", std::process::id()));
    let mut h = Harness::new(&out, "");
    h.step(1); // the Lua runtime exists once play has begun
    h.world
        .borrow()
        .script_manager()
        .eval(script)
        .expect("props spawn");
    h.step(2);
    let frames: usize = std::env::var("RUSTY_BENCH_FRAMES")
        .ok()
        .and_then(|f| f.parse().ok())
        .unwrap_or(1);
    for _ in 0..frames {
        if !h.screenshot(out.join("props.png")) {
            // The harness folds errors into `false`; only a real skip may pass (#885).
            assert!(
                !rusty::render::gpu_required(),
                "RUSTY_REQUIRE_GPU=1 but no shot"
            );
            eprintln!("[instancing] no GPU/software adapter — skipping");
            return;
        }
    }
    let stats = h.stats.borrow().clone();
    let get = |k: &str| stats.get(k).map_or((0.0, 0.0), |s| (s.max, s.avg));
    eprintln!(
        "[instancing {name}] draw_calls {} shadow_draws {} visible {} triangles {} renderer_ms avg {:.2}",
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
    let budget = |k: &str| (k.to_string(), DRAW_CALL_BUDGET);
    assert!(h.assert_budget(&[budget("draw_calls"), budget("shadow_draws")]));
    let _ = std::fs::remove_dir_all(out);
}
