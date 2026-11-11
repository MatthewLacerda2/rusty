//! A shot's `settle` draws frames only when they can change the image (#2092): with
//! nothing on the view reading history (TAA off, no SSIL, fog or authored effect,
//! a cut), `Harness.Screenshot(path, { settle = 8 })` renders once; with TAA on it
//! still draws its eight frames and the shot.

use rusty::dev::harness::Harness;

/// Each shot's renders, from `Harness.Stats()`'s `renderer_ms` samples, and the
/// first frame's `history_passes`, expected by the scenario itself.
const SCENARIO: &str = r#"
Harness.Step(1)
local cam = Scene.CreateEntity("ShotCam")
Scene.AddComponent(cam, "Camera")
Camera.SetRenderOrder(cam, -100)
local function renders()
  local s = Harness.Stats().renderer_ms
  return s and s.samples or 0
end
local function shot(name, opts, want)
  Camera.ResetHistory()
  local before = renders()
  Harness.Expect(Harness.Screenshot(OUT .. name, opts), name .. " written")
  local drawn = renders() - before
  Harness.Expect(drawn == want, name .. " renders " .. want .. " (drew " .. drawn .. ")")
end
Graphics.SetTaaActive(false)
shot("still.png", { settle = 8 }, 1)
shot("still_gizmos.png", { settle = 8, gizmos = true }, 2)
shot("plain.png", nil, 1)
Graphics.SetTaaActive(true)
shot("taa.png", { settle = 8 }, 9)
Harness.Expect(Harness.Stats().history_passes.last >= 1, "TAA reads history")
"#;

#[test]
fn a_settle_draws_frames_only_when_the_view_reads_history() {
    let out = crate::temp::dir().join(format!("rusty_settle_{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let scenario = out.join("settle.lua");
    let prefix = format!(
        "local OUT = {:?}\n",
        rusty::core::paths::to_slash(&out) + "/"
    );
    std::fs::write(&scenario, prefix + SCENARIO).unwrap();
    let mut harness = Harness::new(&out, rusty::scene::default_scene::BOT_SCRIPT);
    harness.set_size(160, 90);
    let report = rusty::dev::scenario::run_file(harness, &scenario).expect("the scenario runs");
    if !out.join("still.png").exists() {
        let msg = "RUSTY_REQUIRE_GPU=1 but no adapter";
        assert!(!rusty::render::gpu_required(), "{msg}");
        return;
    }
    assert!(report.passed, "see {}", report.results_path.display());
    let shot = |name: &str| image::open(out.join(name)).expect("decodes").to_rgba8();
    assert_eq!(
        shot("still.png"),
        shot("plain.png"),
        "a skipped settle is the same shot"
    );
    let _ = std::fs::remove_dir_all(out);
}
