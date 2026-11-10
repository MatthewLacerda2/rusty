//! A camera teleport under TAA (#2003): a shot after `Camera.ResetHistory()` holds
//! no trace of the view before the jump, one without the cut does, and
//! `Harness.Screenshot(path, { settle = n })` converges on the new view.

use image::RgbaImage;

/// A red sphere (view A) and, far off, a blue cube (view B); the camera settles on
/// A, jumps to B and shoots: after a cut (`cut.png`), without one (`ghost.png`),
/// and after a cut and 32 settled frames (`settled.png`).
const SCENARIO: &str = r#"
Harness.Step(1)
local cam = Scene.CreateEntity("ShotCam")
Scene.AddComponent(cam, "Camera")
Camera.SetRenderOrder(cam, -100)
Graphics.SetTaaActive(true)
local a = Scene.CreateEntity("A", "Sphere")
Transform.SetPosition(a, 0, 1, -3)
Material.SetColor(a, {1, 0, 0})
local b = Scene.CreateEntity("B", "Cube")
Transform.SetPosition(b, 500.6, 1, -4)
Material.SetColor(b, {0, 0, 1})
local function at(x)
  Camera.SetPosition(x, 1, 0)
  Camera.SetYaw(-90)
  Camera.SetPitch(0)
end
local function jump(cut, name, opts)
  at(0)
  for _ = 1, 32 do Harness.Render() end
  at(500)
  if cut then Camera.ResetHistory() end
  Harness.Screenshot(OUT .. name, opts)
end
jump(true, "cut.png")
jump(false, "ghost.png")
jump(true, "settled.png", { settle = 32 })
jump(false, "faded.png", { settle = 64 })
Harness.Expect(not pcall(Harness.Screenshot, OUT .. "bad.png", { settle = -1 }), "a negative settle")
"#;

/// Pixels plainly red: only the sphere of view A is.
fn reddish(img: &RgbaImage) -> usize {
    let red =
        |p: &&image::Rgba<u8>| p[0] as i32 > p[1] as i32 + 40 && p[0] as i32 > p[2] as i32 + 40;
    img.pixels().filter(red).count()
}

#[test]
fn a_cut_drops_the_view_before_a_teleport_and_settle_converges() {
    let out = crate::temp::dir().join(format!("rusty_taa_cut_{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let scenario = out.join("cut.lua");
    let prefix = format!("local OUT = {:?}\n", format!("{}/", out.display()));
    std::fs::write(&scenario, prefix + SCENARIO).unwrap();
    let report = rusty::dev::scenario::run_isolated(&scenario, &out).expect("the scenario runs");
    assert!(report.passed, "see {}", report.results_path.display());
    if !out.join("cut.png").exists() {
        assert!(
            !rusty::render::gpu_required(),
            "RUSTY_REQUIRE_GPU=1 but no adapter"
        );
        return;
    }
    let read = |name: &str| image::open(out.join(name)).expect("decodes").to_rgba8();
    let (cut, ghost, settled) = (read("cut.png"), read("ghost.png"), read("settled.png"));
    let blue = |img: &RgbaImage| {
        img.pixels()
            .filter(|p| p[2] as i32 > p[0] as i32 + 40)
            .count()
    };
    assert!(blue(&cut) > 100, "view B shows its cube: {}", blue(&cut));
    assert_eq!(reddish(&cut), 0, "no trace of view A after the cut");
    assert!(
        reddish(&ghost) > 20,
        "without the cut A ghosts in: {}",
        reddish(&ghost)
    );
    assert_eq!(reddish(&settled), 0, "a settled cut is B alone");
    let faded = reddish(&read("faded.png"));
    eprintln!(
        "ghost {} faded {faded} blue {}",
        reddish(&ghost),
        blue(&cut)
    );
    assert!(faded < reddish(&ghost), "settling fades a ghost: {faded}");
    let _ = std::fs::remove_dir_all(out);
}
