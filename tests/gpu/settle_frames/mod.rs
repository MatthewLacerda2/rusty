//! A shot's `settle` draws frames only when they can change the image (#2092): with
//! nothing reading history, `Harness.Screenshot(path, { settle = 8 })` renders once
//! and writes what a full settle writes; with TAA, SSIL or a shown render texture
//! it still draws its eight frames and the shot; and a pending shadow tile resize
//! ([`shadow`]) keeps it settling too.

mod shadow;

use std::path::{Path, PathBuf};

use rusty::dev::harness::Harness;

/// The Lua every case's scenario starts with: `renders()` counts the frames drawn
/// (`Harness.Stats()`'s `renderer_ms` samples) on a main camera with a volume, and `shot(name, opts, want)`
/// screenshots after a cut and expects `want` renders.
const PRELUDE: &str = r#"
Harness.Step(1)
local main = Scene.CreateEntity("ShotCam")
Scene.AddComponent(main, "PostProcessing")
Camera.SetRenderOrder(main, -100)
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
-- What a settle drew before #2092: `n` frames, then the shot.
local function full(name, n)
  Camera.ResetHistory()
  for _ = 1, n do Harness.Render() end
  Harness.Expect(Harness.Screenshot(OUT .. name), name .. " written")
end
"#;

/// Run `body` after [`PRELUDE`] in a fresh harness; its output folder, or `None`
/// when no adapter drew its first shot, `first`.
pub(super) fn run(tag: &str, body: &str, first: &str) -> Option<PathBuf> {
    let out = crate::temp::dir().join(format!("rusty_settle_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let scenario = out.join("settle.lua");
    let out_lua = rusty::core::paths::to_slash(&out) + "/";
    let lua = format!("local OUT = {out_lua:?}\n{PRELUDE}{body}");
    std::fs::write(&scenario, lua).unwrap();
    let mut harness = Harness::new(&out, rusty::scene::default_scene::BOT_SCRIPT);
    harness.set_size(160, 90);
    let report = rusty::dev::scenario::run_file(harness, &scenario).expect("the scenario runs");
    if !out.join(first).exists() {
        let msg = "RUSTY_REQUIRE_GPU=1 but no adapter";
        assert!(!rusty::render::gpu_required(), "{msg}");
        return None;
    }
    assert!(report.passed, "see {}", report.results_path.display());
    Some(out)
}

/// The decoded PNG `name` in `out`.
pub(super) fn png(out: &Path, name: &str) -> image::RgbaImage {
    image::open(out.join(name)).expect("decodes").to_rgba8()
}

const CASES: &str = r#"
Graphics.SetTaaActive(false)
shot("still.png", { settle = 8 }, 1)
full("full.png", 8)
shot("still_gizmos.png", { settle = 8, gizmos = true }, 2)
shot("plain.png", nil, 1)
Graphics.SetSsilActive(true)
shot("ssil.png", { settle = 8 }, 9)
Graphics.SetSsilActive(false)
local cam = Scene.CreateEntity("MonitorCam")
Scene.AddComponent(cam, "Camera")
Camera.SetTargetTexture(cam, "monitor", 32, 32)
Image.SetTexture(UI.Create("Image"), "rt:monitor")
shot("monitor.png", { settle = 8 }, 9)
Graphics.SetTaaActive(true)
shot("taa.png", { settle = 8 }, 9)
Harness.Expect(Harness.Stats().history_passes.last >= 1, "TAA reads history")
"#;

#[test]
fn a_settle_draws_frames_only_when_the_view_reads_history() {
    let Some(out) = run("cases", CASES, "still.png") else {
        return;
    };
    let still = png(&out, "still.png");
    assert_eq!(
        still,
        png(&out, "full.png"),
        "a skipped settle is a full one"
    );
    assert_eq!(still, png(&out, "plain.png"), "and the first frame");
    let _ = std::fs::remove_dir_all(out);
}
