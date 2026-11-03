//! A scene's reflection cubes live in its own `<scene>.reflections/` folder (#1692):
//! two scenes in one folder bake without overwriting each other's cubes, and a rebake
//! with fewer probes removes only its own extra cubes. The GPU half skips without an
//! adapter.

use std::path::Path;

use glam::Vec3;

use super::{bake_scene_reflections, prune_stale, reflection_dir};
use crate::dev::bake::lighting::{start_lighting, LightingBakeParams};
use crate::dev::bake::progress::BakeLog;
use crate::scene::Scene;

/// A scene of `probes` reflection probes at a small resolution, each at its own spot.
fn probed(probes: usize, x: f32) -> Scene {
    let mut scene = Scene::new();
    for i in 0..probes {
        let at = Vec3::new(x + i as f32 * 4.0, 1.0, 0.0);
        scene.reflection_probes.add(at, Vec3::splat(2.0));
    }
    scene.lighting_settings.reflections.resolution = 16;
    scene
}

fn fresh_dir(name: &str) -> std::path::PathBuf {
    let dir = crate::test_temp::dir().join(format!("rusty_1692_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn each_scene_names_its_own_folder() {
    assert_eq!(
        reflection_dir("a/kolvik.scene"),
        Path::new("a/kolvik.scene.reflections")
    );
    assert_ne!(reflection_dir("a/x.scene"), reflection_dir("a/y.scene"));
}

#[test]
fn prune_removes_only_cubes_past_the_count() {
    let dir = fresh_dir("prune");
    let names = ["reflection_probe_0.ktx2", "reflection_probe_1.ktx2"];
    let kept = [
        "reflection_probe_x.ktx2",
        "notes.txt",
        "reflection_probe_1.png",
    ];
    for name in names.iter().chain(&kept) {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    prune_stale(&dir, 1).unwrap();
    assert!(dir.join(names[0]).exists(), "within the count");
    assert!(!dir.join(names[1]).exists(), "past the count");
    for name in kept {
        assert!(dir.join(name).exists(), "{name} is not a cube");
    }
    prune_stale(&dir.join("missing"), 0).unwrap();
}

/// The game's repro: two scenes in one folder, each baked by Generate Lighting's
/// stepped bake, one after the other. The first keeps its own cubes, byte for byte.
#[test]
fn gpu_two_scenes_in_one_folder_keep_their_own_cubes() {
    let dir = fresh_dir("two_scenes");
    let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
    let (kolvik_path, default_path) = (path("kolvik.scene"), path("default.scene"));
    let mut kolvik = probed(2, 0.0);
    if !generate(&mut kolvik, &kolvik_path) {
        return; // no adapter
    }
    let mut default = probed(3, 50.0);
    assert!(generate(&mut default, &default_path));
    let cube = |scene: &Scene, i: usize| scene.reflection_probes.probes[i].cubemap_path.clone();
    let kolvik_dir = reflection_dir(&kolvik_path);
    assert!(Path::new(&cube(&kolvik, 0)).starts_with(&kolvik_dir));
    assert!(Path::new(&cube(&default, 0)).starts_with(reflection_dir(&default_path)));
    assert_ne!(cube(&kolvik, 0), cube(&default, 0));

    // A rebake of the first scene with fewer probes: its extra cube goes, and the
    // second scene's cubes stay.
    let before = std::fs::read(cube(&default, 2)).unwrap();
    kolvik.reflection_probes.remove(1);
    assert!(bake_scene_reflections(&mut kolvik, Some(&kolvik_path), &mut BakeLog::off()).unwrap());
    assert!(!kolvik_dir.join("reflection_probe_1.ktx2").exists());
    assert!(kolvik_dir.join("reflection_probe_0.ktx2").exists());
    assert_eq!(std::fs::read(cube(&default, 2)).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Run Generate Lighting's probe bake to its end and commit it. False with no adapter.
fn generate(scene: &mut Scene, path: &str) -> bool {
    let params = LightingBakeParams::default();
    let mut job = start_lighting(scene, Some(path), None, params).unwrap();
    if scene.reflection_probes.is_empty() || !job.report().ran_reflection_bake {
        return false;
    }
    while !job.step(scene, &mut |_| {}).unwrap() {}
    job.commit(scene).unwrap();
    true
}
