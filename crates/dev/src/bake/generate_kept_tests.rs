use glam::Vec3;
use serde_json::Value;

use super::{apply_occlusion, bake_occlusion_only, generate_lighting, GenerateReport};
use crate::bake::occlusion::{generate_occlusion_logged, reach};
use crate::bake::progress::{BakeLog, Line};
use crate::bake::report::{read, Kind};
use rusty_lighting::occlusion::bake_occlusion;
use rusty_lighting::occlusion::test_level::{block, corridors};
use rusty_scene::lighting::occlusion::{encode, occlusion_path};
use rusty_scene::Scene;

fn saved() -> (Scene, String) {
    let dir = rusty_test_support::temp::dir().join("generate_kept");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let level = cheap(corridors());
    rusty_edit::save_to_file(&level, &path).unwrap();
    (reloaded(&path), path)
}

fn cheap(mut level: Scene) -> Scene {
    let s = &mut level.lighting_settings.lightmaps;
    (s.texels_per_unit, s.samples, s.bounces, s.max_lightmap_size) = (0.5, 1, 1, 16);
    let r = &mut level.lighting_settings.reflections;
    // Reflection probes are most of a Generate's time on lavapipe.
    (r.resolution, r.max_probes) = (16, 1);
    level
}

fn reloaded(path: &str) -> Scene {
    let mut scene = Scene::new();
    rusty_edit::load_from_file(&mut scene, path).unwrap();
    scene
}

fn another_writers_pvs() -> Vec<u8> {
    let mut other = corridors();
    block(&mut other, "Extra", Vec3::splat(3.0), Vec3::splat(3.5));
    encode(&bake_occlusion(&other, 2).0.expect("occluders"))
}

fn generate(scene: &mut Scene, path: &str) -> (GenerateReport, Vec<String>) {
    let lines = std::cell::RefCell::new(Vec::new());
    let mut log = BakeLog::new(std::time::Duration::default, |l: Line| {
        lines.borrow_mut().push(l.text)
    });
    let report = generate_lighting(scene, Some(path), None, &mut |_| {}, &mut log);
    drop(log);
    (report.unwrap(), lines.into_inner())
}

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
    rusty_edit::save_to_file(&scene, &path).unwrap();
    let mut scene = reloaded(&path);
    assert!(scene.occlusion.is_some(), "the PVS read from disk");
    std::fs::write(occlusion_path(&path), another_writers_pvs()).unwrap();

    moved(&mut scene, "Lamp");
    let (second, lines) = generate(&mut scene, &path);
    assert!(second.occlusion_kept, "only a light moved");
    assert_eq!(second.occlusion, first.occlusion, "the kept PVS's counts");
    assert!(second.summary().contains("occlusion unchanged, kept"));
    let said = lines
        .iter()
        .any(|l| l.ends_with("occlusion unchanged, kept"));
    assert!(said, "{lines:?}");
    assert_eq!(stages(&path), ["skipped: unchanged", "write"]);
    let report = read(&path, Kind::Occlusion).unwrap();
    assert_eq!(report["cells"], Value::from(first.occlusion.cells));
    let file = std::fs::read(occlusion_path(&path)).unwrap();
    assert_eq!(file, pvs, "the kept PVS's file, over another writer's");

    moved(&mut scene, "Crate");
    let third =
        generate_occlusion_logged(&scene, reach(&scene, None), &mut BakeLog::off()).unwrap();
    assert!(!third.kept, "a static box moved");
    apply_occlusion(&mut scene, &path, third).unwrap();
    assert_eq!(stages(&path), ["bake", "write"]);
    assert_ne!(std::fs::read(occlusion_path(&path)).unwrap(), pvs);
}

#[test]
fn bake_occlusion_re_bakes_an_unchanged_scene() {
    let (mut scene, path) = saved();
    bake_occlusion_only(&mut scene, Some(&path), None, &mut BakeLog::off()).unwrap();
    let pvs = std::fs::read(occlusion_path(&path)).unwrap();
    bake_occlusion_only(&mut scene, Some(&path), None, &mut BakeLog::off()).unwrap();
    assert_eq!(stages(&path), ["bake", "write"], "baked again");
    assert_eq!(std::fs::read(occlusion_path(&path)).unwrap(), pvs);
}
