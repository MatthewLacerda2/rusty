//! What a rebake through Generate Lighting's commit removes from a scene's
//! `<scene>.reflections/` folder (#1692): a scene left with no probes loses every cube,
//! and a bake dropped before its commit removes nothing. The GPU half skips without an
//! adapter.

use std::path::Path;

use super::reflection_dir;
use super::tests::{finished, fresh_dir, generate, probed};
use crate::bake::progress::BakeLog;

/// The cube files `dir` holds, by name, sorted.
fn cubes(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn gpu_a_scene_left_with_no_probes_loses_every_cube() {
    let dir = fresh_dir("no_probes");
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = probed(2, 0.0);
    if !generate(&mut scene, &path) {
        return; // no adapter
    }
    let folder = reflection_dir(&path);
    assert_eq!(cubes(&folder).len(), 2);
    scene.reflection_probes.clear();
    assert!(generate(&mut scene, &path));
    assert!(scene.reflection_probes.is_empty(), "nothing auto-placed");
    assert_eq!(cubes(&folder), Vec::<String>::new());

    // `Reflection.Bake()` on a probe-less scene does the same, adapter or not.
    std::fs::write(folder.join("reflection_probe_0.ktx2"), b"x").unwrap();
    let ran = super::bake_scene_reflections(&mut scene, Some(&path), &mut BakeLog::off());
    assert!(ran.unwrap());
    assert_eq!(cubes(&folder), Vec::<String>::new());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn gpu_a_dropped_bake_removes_nothing() {
    let dir = fresh_dir("dropped");
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = probed(2, 0.0);
    if !generate(&mut scene, &path) {
        return; // no adapter
    }
    let folder = reflection_dir(&path);
    let before = cubes(&folder);
    let kept = std::fs::read(folder.join("reflection_probe_1.ktx2")).unwrap();
    scene.reflection_probes.remove(1);
    let job = finished(&mut scene, &path).unwrap();
    drop(job); // finished, never committed
    assert_eq!(
        cubes(&folder),
        before,
        "no cube removed, no staged file left"
    );
    let after = std::fs::read(folder.join("reflection_probe_1.ktx2")).unwrap();
    assert_eq!(after, kept);
    let _ = std::fs::remove_dir_all(&dir);
}
