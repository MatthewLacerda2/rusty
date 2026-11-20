//! Generate Lighting keeps an unchanged PVS (#2348): a lamp moved between two
//! Generates re-bakes no occlusion and leaves its file byte for byte, a static box
//! moved re-bakes it, and Bake Occlusion always re-bakes.

use glam::Vec3;
use serde_json::Value;

use super::{bake_occlusion_only, generate_lighting, GenerateReport};
use crate::bake::progress::{BakeLog, Line};
use crate::bake::report::{read, Kind};
use rusty_lighting::occlusion::test_level::corridors;
use rusty_scene::lighting::occlusion::occlusion_path;
use rusty_scene::Scene;

/// The corridors level saved at a fresh path with cheap lightmaps, and loaded back.
fn saved() -> (Scene, String) {
    let dir = rusty_test_support::temp::dir().join("generate_kept");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut level = corridors();
    let s = &mut level.lighting_settings.lightmaps;
    (s.texels_per_unit, s.samples, s.bounces, s.max_lightmap_size) = (1.0, 2, 1, 32);
    rusty_edit::save_to_file(&level, &path).unwrap();
    let mut scene = Scene::new();
    rusty_edit::load_from_file(&mut scene, &path).unwrap();
    (scene, path)
}

/// One Generate, with the lines its log said.
fn generate(scene: &mut Scene, path: &str) -> (GenerateReport, Vec<String>) {
    let lines = std::cell::RefCell::new(Vec::new());
    let mut log = BakeLog::new(std::time::Duration::default, |l: Line| {
        lines.borrow_mut().push(l.text)
    });
    let report = generate_lighting(scene, Some(path), None, &mut |_| {}, &mut log);
    drop(log);
    (report.unwrap(), lines.into_inner())
}

/// The occlusion report's stage names.
fn stages(path: &str) -> Vec<String> {
    let report = read(path, Kind::Occlusion).expect("an occlusion report");
    let names = report["stages"].as_array().unwrap().iter();
    names
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect()
}

fn moved(scene: &mut Scene, name: &str) {
    let id = scene.find_entity_by_name(name).expect(name);
    scene.world.transform_mut(id).unwrap().position += Vec3::X * 0.5;
}

#[test]
fn gpu_an_unchanged_pvs_is_kept_and_a_moved_static_box_re_bakes_it() {
    let (mut scene, path) = saved();
    let (first, _) = generate(&mut scene, &path);
    assert!(!first.occlusion_kept && first.occlusion.cells > 0);
    let pvs = std::fs::read(occlusion_path(&path)).unwrap();

    moved(&mut scene, "Lamp");
    let (second, lines) = generate(&mut scene, &path);
    assert!(second.occlusion_kept, "only a light moved");
    assert_eq!(second.occlusion, first.occlusion, "the kept PVS's counts");
    assert!(second.summary().contains("occlusion unchanged, kept"));
    let said = lines
        .iter()
        .any(|l| l.ends_with("occlusion unchanged, kept"));
    assert!(said, "{lines:?}");
    assert_eq!(stages(&path), ["skipped: unchanged"]);
    let report = read(&path, Kind::Occlusion).unwrap();
    assert_eq!(report["cells"], Value::from(first.occlusion.cells));
    assert_eq!(std::fs::read(occlusion_path(&path)).unwrap(), pvs);

    moved(&mut scene, "Crate");
    let (third, _) = generate(&mut scene, &path);
    assert!(!third.occlusion_kept, "a static box moved");
    assert_eq!(stages(&path), ["bake", "write"]);
    assert_ne!(std::fs::read(occlusion_path(&path)).unwrap(), pvs);
}

#[test]
fn bake_occlusion_re_bakes_an_unchanged_scene() {
    let (mut scene, path) = saved();
    bake_occlusion_only(&mut scene, Some(&path), &mut BakeLog::off()).unwrap();
    let pvs = std::fs::read(occlusion_path(&path)).unwrap();
    bake_occlusion_only(&mut scene, Some(&path), &mut BakeLog::off()).unwrap();
    assert_eq!(stages(&path), ["bake", "write"], "baked again");
    assert_eq!(std::fs::read(occlusion_path(&path)).unwrap(), pvs);
}
