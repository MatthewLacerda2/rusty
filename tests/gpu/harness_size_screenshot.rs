//! The harness renders at a chosen size (#1583): the run's size (`play --size`,
//! `Harness::set_size`) and one `Screenshot`'s own (`{ width =, height = }`).

use std::path::Path;

use rusty::dev::harness::Harness;

/// The PNG's pixel size.
fn png_size(path: &Path) -> (u32, u32) {
    image::image_dimensions(path).expect("the screenshot decodes")
}

/// Whether a shot that came back `false` is a real skip (no adapter) — only that
/// may pass (#885).
fn skipped(what: &str) -> bool {
    assert!(
        !rusty::render::gpu_required(),
        "RUSTY_REQUIRE_GPU=1 but no {what}"
    );
    eprintln!("[harness size] no GPU/software adapter — skipping {what}");
    true
}

#[test]
fn the_run_size_sizes_shots_and_the_ui_and_one_shot_may_name_its_own() {
    let out = crate::temp::dir().join(format!("rusty_size_gpu_{}", std::process::id()));
    let mut h = Harness::new(&out, "");
    assert_eq!(h.size(), (1280, 720), "the default stays a 720p Game view");
    h.set_size(320, 180);
    // The sim lays its UI out on the run's size, as a window of that size would.
    let screen = h.world.borrow().resources.ui_view().screen;
    assert_eq!(screen, glam::Vec2::new(320.0, 180.0));
    h.step(2);

    let (run, own, after) = (
        out.join("run.png"),
        out.join("own.png"),
        out.join("after.png"),
    );
    if !h.screenshot(&run) && skipped("run-size shot") {
        return;
    }
    assert_eq!(png_size(&run), (320, 180));
    assert!(h.screenshot_at(&own, (200, 400)));
    assert_eq!(
        png_size(&own),
        (200, 400),
        "a shot's own size, portrait included"
    );
    assert!(h.screenshot(&after));
    assert_eq!(
        png_size(&after),
        (320, 180),
        "the run keeps its size after it"
    );
    let _ = std::fs::remove_dir_all(out);
}

#[test]
fn a_scenario_names_a_shot_size_and_a_bad_one_is_an_error() {
    let out = crate::temp::dir().join(format!("rusty_size_lua_{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let shot = out.join("sized.png");
    let scenario = out.join("size.lua");
    let lua = format!(
        "Harness.Step(1)\n\
         local ok = Harness.Screenshot({shot:?}, {{ width = 240, height = 100 }})\n\
         Harness.Log('shot ' .. tostring(ok))\n\
         local bad = pcall(Harness.Screenshot, {shot:?}, {{ width = 0, height = 100 }})\n\
         Harness.Expect(not bad, 'a zero width is refused')\n\
         Harness.Expect(not pcall(Harness.Screenshot, {shot:?}, {{ width = 64 }}), 'height is required')\n"
    );
    std::fs::write(&scenario, lua).unwrap();
    let report = rusty::dev::scenario::run_isolated(&scenario, &out).expect("the scenario runs");
    assert!(report.passed, "see {}", report.results_path.display());
    if !shot.exists() && skipped("scenario shot") {
        return;
    }
    assert_eq!(png_size(&shot), (240, 100));
    let _ = std::fs::remove_dir_all(out);
}
