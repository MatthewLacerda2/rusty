//! A camera jump under TAA (#2003): after `Camera.ResetHistory()` a shot is the same
//! whatever the view showed before the jump (nothing of it survives, in TAA, motion
//! vectors, SSIL, fog or exposure); without the cut it is not, and
//! `Harness.Screenshot(path, { settle = n })` converges on the new view.

use image::RgbaImage;

/// The camera settles on one view (`a`: a green sphere in the arena; `c`: the sky),
/// jumps to view B (the far side of the arena, the sphere behind it) and shoots,
/// with and without a cut and a settle.
const SCENARIO: &str = r#"
Harness.Step(1)
local cam = Scene.CreateEntity("ShotCam")
Scene.AddComponent(cam, "Camera")
Camera.SetRenderOrder(cam, -100)
Graphics.SetTaaActive(true)
local a = Scene.CreateEntity("A", "Sphere")
Transform.SetPosition(a, 0, 1, -3)
Material.SetColor(a, {0, 1, 0})
local function look(x, yaw, pitch)
  Camera.SetPosition(x, 1, 0)
  Camera.SetYaw(yaw)
  Camera.SetPitch(pitch)
end
local before = { a = function() look(0, -90, 0) end, c = function() look(-6, 0, 80) end }
local function jump(from, cut, name, opts)
  before[from]()
  for _ = 1, 32 do Harness.Render() end
  look(3, 90, -10)
  if cut then Camera.ResetHistory() end
  Harness.Screenshot(OUT .. name, opts)
end
jump("a", true, "cut_a.png")
jump("c", true, "cut_c.png")
jump("a", false, "ghost_a.png")
jump("c", false, "ghost_c.png")
jump("a", true, "settled_a.png", { settle = 32 })
jump("c", true, "settled_c.png", { settle = 32 })
jump("a", false, "faded_a.png", { settle = 64 })
Harness.Expect(not pcall(Harness.Screenshot, OUT .. "bad.png", { settle = -1 }), "a negative settle")
Harness.Expect(not pcall(Harness.Screenshot, OUT .. "bad.png", { settle = 1025 }), "past 1024")
"#;

/// The mean absolute difference of two shots, per channel, in 0..255.
fn diff(x: &RgbaImage, y: &RgbaImage) -> f64 {
    let sum: u64 = x
        .as_raw()
        .iter()
        .zip(y.as_raw())
        .map(|(a, b)| a.abs_diff(*b) as u64)
        .sum();
    sum as f64 / x.as_raw().len() as f64
}

#[test]
fn a_cut_drops_the_view_before_a_jump_and_settle_converges() {
    let out = crate::temp::dir().join(format!("rusty_taa_cut_{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let scenario = out.join("cut.lua");
    let prefix = format!(
        "local OUT = {:?}\n",
        rusty::core::paths::to_slash(&out) + "/"
    );
    std::fs::write(&scenario, prefix + SCENARIO).unwrap();
    let report = rusty::dev::scenario::run_isolated(&scenario, &out).expect("the scenario runs");
    assert!(report.passed, "see {}", report.results_path.display());
    if !out.join("cut_a.png").exists() {
        let msg = "RUSTY_REQUIRE_GPU=1 but no adapter";
        assert!(!rusty::render::gpu_required(), "{msg}");
        return;
    }
    let shot = |name: &str| image::open(out.join(name)).expect("decodes").to_rgba8();
    let pair = |x: &str, y: &str| diff(&shot(x), &shot(y));
    let (cut, ghost) = (
        pair("cut_a.png", "cut_c.png"),
        pair("ghost_a.png", "ghost_c.png"),
    );
    let (settled, faded) = (
        pair("settled_a.png", "settled_c.png"),
        pair("faded_a.png", "settled_a.png"),
    );
    eprintln!("cut {cut} ghost {ghost} settled {settled} faded {faded}");
    assert!(
        cut < 0.01,
        "after a cut nothing of the last view survives: {cut}"
    );
    assert!(ghost > 0.5, "without one it does: {ghost}");
    assert!(
        settled < 0.01,
        "a settled cut is the new view alone: {settled}"
    );
    assert!(faded < ghost / 4.0, "settling fades what survives: {faded}");
    let _ = std::fs::remove_dir_all(out);
}
