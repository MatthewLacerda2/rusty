//! Play-test a test game: generate it twice and compare, run every scenario as
//! `play` does, take the shots, and refuse a shot that is all one colour.

use std::path::{Path, PathBuf};

use super::{generate, tree, Game, Params, SHOTS};
use crate::core::project::{boot_scene, Access, Opened};
use crate::dev::harness::Harness;
use crate::dev::scenario;

/// Where in the generated project `out` a check writes: one folder per scenario,
/// and `shots/` (`cache/` is the project's regenerable folder).
fn test_dir(out: &Path) -> PathBuf {
    out.join("cache").join("test")
}

/// What a [`check`] found: one line per step, and whether all of them passed.
#[derive(Debug, Default)]
pub struct Report {
    pub lines: Vec<String>,
    pub passed: bool,
}

impl Report {
    fn step(&mut self, ok: bool, line: String) {
        self.passed &= ok;
        self.lines
            .push(format!("{} {line}", if ok { "ok  " } else { "FAIL" }));
    }
}

/// Generate `game` into `out`, prove a second generation writes the same files,
/// then run each of its `scenarios/*.lua` (in name order) on the project's startup
/// scene and take its shots into `cache/test/shots/`. An `Err` is a step that could
/// not run at all; a step that ran and failed is in the [`Report`].
pub fn check(game: &Game, out: &Path, params: &Params) -> Result<Report, String> {
    let out = std::path::absolute(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut report = Report {
        passed: true,
        ..Report::default()
    };
    report.step(
        deterministic(game, &out, params)?,
        "generating twice writes the same files".into(),
    );
    let opened = crate::core::project::open(&out, Access::Run)?;
    let scene = startup_scene(&opened)?;
    for path in scenarios(&out)? {
        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let dir = test_dir(&out).join(&stem);
        let run = scenario::run(
            &path,
            &dir,
            &scene,
            opened.settings.clone(),
            Default::default(),
        )?;
        let results = run.results_path.display();
        report.step(run.passed, format!("scenario {stem} ({results})"));
    }
    if out.join(SHOTS).is_file() {
        shots(&out, &scene, &opened, &mut report)?;
    }
    Ok(report)
}

/// Generate into `out`, move it aside to `<out>.first`, generate again into the
/// same path (the project's own path is written into some of its files) and
/// compare. The aside folder is only ever deleted through [`tree::remove`].
fn deterministic(game: &Game, out: &Path, params: &Params) -> Result<bool, String> {
    generate(game, out, params)?;
    let first = tree::beside(out, "first")?;
    tree::remove(&first)?;
    // lint: allow(atomic-write) moves the first generation aside whole; it writes nothing
    std::fs::rename(out, &first).map_err(|e| format!("{}: {e}", out.display()))?;
    generate(game, out, params)?;
    let differ = compared_differences(&first, out)?;
    tree::remove(&first)?;
    for path in &differ {
        eprintln!(
            "[Game] {}: a second generation wrote other bytes",
            path.display()
        );
    }
    Ok(differ.is_empty())
}

/// The files that differ between two generations, leaving out a scene's bake
/// reports (`<scene>.bake/`): they are measurements by design, the clock and
/// stage times included (`docs/api/Lighting.md`, *Bake reports*), while the baked
/// data they describe (SH sidecar, cubemaps, lightmap pages, PVS) stays compared.
fn compared_differences(first: &Path, second: &Path) -> Result<Vec<PathBuf>, String> {
    let mut differ = tree::differences(first, second)?;
    differ.retain(|rel| !crate::dev::bake::report::in_report_dir(rel));
    Ok(differ)
}

/// The scene a run of the project boots, as `play` picks it.
fn startup_scene(opened: &Opened) -> Result<String, String> {
    let startup = &opened.settings.build.startup_scene;
    let plan = boot_scene(&opened.root, startup, Access::Run)?;
    plan.scenes
        .into_iter()
        .next()
        .ok_or_else(|| "the project names no startup scene".into())
}

/// The project's `scenarios/*.lua`, by name.
fn scenarios(out: &Path) -> Result<Vec<PathBuf>, String> {
    let dir = out.join("scenarios");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "lua"))
        .collect();
    found.sort();
    Ok(found)
}

/// The scenario that takes the shots: for each entry of `shots.lua`, put the main
/// camera at its `position` looking at its `target`, start the view's history
/// over and screenshot. A shot `Screenshot` declines (no GPU or software adapter)
/// is logged as skipped, as every GPU test skips without one.
const SHOTS_SCENARIO: &str = r#"
for _, shot in ipairs(dofile("shots.lua")) do
    local p, t = shot.position, shot.target
    local dx, dy, dz = t[1] - p[1], t[2] - p[2], t[3] - p[3]
    Camera.SetPosition(p[1], p[2], p[3])
    Camera.SetYaw(math.deg(math.atan(dz, dx)))
    Camera.SetPitch(math.deg(math.atan(dy, math.sqrt(dx * dx + dz * dz))))
    Camera.ResetHistory()
    local path = "cache/test/shots/" .. shot.name .. ".png"
    if not Harness.Screenshot(path, { settle = 8 }) then
        Harness.Log("shot skipped (no GPU adapter): " .. shot.name)
    end
end
"#;

/// Take the shots and check each PNG written is not all one colour.
fn shots(out: &Path, scene: &str, opened: &Opened, report: &mut Report) -> Result<(), String> {
    let dir = test_dir(out).join("shots");
    let harness = Harness::in_user_workspace(&dir, scene, opened.settings.clone())?;
    let run = scenario::run_code(harness, SHOTS_SCENARIO, SHOTS)?;
    report.step(
        run.passed,
        format!("shots ({})", run.results_path.display()),
    );
    let mut pngs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .collect();
    pngs.sort();
    for png in pngs {
        let flat = one_colour(&png)?;
        let verdict = if flat {
            "is all one colour"
        } else {
            "has an image"
        };
        report.step(!flat, format!("shot {} {verdict}", png.display()));
    }
    Ok(())
}

/// Whether every pixel of the PNG at `path` is the same.
fn one_colour(path: &Path) -> Result<bool, String> {
    let image = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pixels = image.to_rgba8();
    let mut all = pixels.pixels();
    let first = all.next().copied();
    Ok(all.all(|p| Some(*p) == first))
}

#[cfg(test)]
mod tests {
    use super::{compared_differences, one_colour};

    fn png(name: &str, image: &image::RgbaImage) -> std::path::PathBuf {
        let path = crate::test_temp::dir().join(name);
        image.save(&path).unwrap();
        path
    }

    #[test]
    fn a_flat_image_is_one_colour_and_one_differing_pixel_is_not() {
        let mut image = image::RgbaImage::from_pixel(8, 4, image::Rgba([0, 0, 0, 255]));
        assert!(one_colour(&png("flat.png", &image)).unwrap());
        image.put_pixel(7, 3, image::Rgba([0, 0, 1, 255]));
        assert!(!one_colour(&png("dot.png", &image)).unwrap());
    }

    #[test]
    fn a_scenes_bake_reports_are_left_out_and_everything_else_is_compared() {
        let dir = crate::test_temp::dir().join("bake-reports");
        let reports = std::path::Path::new("assets/scenes/x.scene.bake");
        let differing = [
            reports.join("probes.json"),
            reports.join("sub/probes.json"),
            "assets/bread.bake/probes.json".into(),
            "assets/scenes/x.scene.lighting.json".into(),
        ];
        for (side, text) in [("a", "1"), ("b", "2")] {
            for rel in &differing {
                let path = dir.join(side).join(rel);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, text).unwrap();
            }
        }
        let differ = compared_differences(&dir.join("a"), &dir.join("b")).unwrap();
        assert_eq!(differ, &differing[2..], "the reports alone are left out");
    }

    #[test]
    fn a_file_that_is_no_png_is_an_error() {
        let path = crate::test_temp::dir().join("broken.png");
        std::fs::write(&path, b"not a png").unwrap();
        assert!(one_colour(&path).is_err());
    }
}
