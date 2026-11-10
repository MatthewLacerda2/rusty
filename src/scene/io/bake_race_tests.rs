//! The issue's repro (#1982) for every bake sidecar the keep-newer rule covers: a world
//! that only loaded a bake never writes it back over a newer one another `Scene`
//! (another process) baked, a world that baked since does, Save As carries it, and
//! a cleared bake removes only the file this world saw.

use std::path::PathBuf;
use std::sync::Arc;

use glam::{UVec3, Vec3};

use crate::scene::lighting::occlusion::{occlusion_path, Grid, OcclusionData};
use crate::scene::Scene;

/// One sidecar kind: where it lives, how a world bakes `n` into it (`0` clears).
struct Kind {
    name: &'static str,
    path: fn(&str) -> PathBuf,
    bake: fn(&mut Scene, u8),
}

const KINDS: [Kind; 2] = [
    Kind {
        name: "occlusion",
        path: occlusion_path,
        bake: |s, n| {
            let grid = Grid {
                origin: Vec3::ZERO,
                cell: f32::from(n),
                dims: UVec3::ONE,
            };
            let data = OcclusionData::new(grid, Vec::new(), Vec::new(), vec![0]);
            *s.occlusion = (n > 0).then(|| Arc::new(data.expect("one cell")));
        },
    },
    Kind {
        name: "navmesh",
        path: crate::scene::nav_settings::navmesh_path,
        bake: |s, n| s.navmesh.bytes = (n > 0).then(|| Arc::from(vec![n; 4])),
    },
];

/// A saved scene with every kind baked as `1`; its path.
fn level(name: &str) -> String {
    let dir = crate::test_temp::dir().join("bake_race").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    KINDS.iter().for_each(|k| (k.bake)(&mut scene, 1));
    scene.save_to_file(&path).unwrap();
    path
}

fn opened(path: &str) -> Scene {
    let mut scene = Scene::new();
    scene.load_from_file(path).unwrap();
    scene
}

/// The editor opens the level, a headless session re-bakes it (as `2`) and saves.
fn raced(name: &str) -> (String, Scene) {
    let path = level(name);
    let editor = opened(&path);
    let mut headless = opened(&path);
    KINDS.iter().for_each(|k| (k.bake)(&mut headless, 2));
    headless.save_to_file(&path).unwrap();
    (path, editor)
}

fn read(kind: &Kind, path: &str) -> Option<Vec<u8>> {
    std::fs::read((kind.path)(path)).ok()
}

#[test]
fn a_save_that_baked_nothing_keeps_the_newer_bake_and_warns() {
    let (path, mut editor) = raced("kept");
    let newer: Vec<_> = KINDS.iter().map(|k| read(k, &path)).collect();
    editor.ambient_intensity = 0.25;
    editor.save_to_file(&path).unwrap();
    for (kind, newer) in KINDS.iter().zip(newer) {
        assert_eq!(
            read(kind, &path),
            newer,
            "{}: the newer bake survives",
            kind.name
        );
        let file = (kind.path)(&path);
        assert!(editor.bake_files.warned(&file), "{}: warned", kind.name);
    }
    assert_eq!(opened(&path).ambient_intensity, 0.25, "the edit is saved");
}

#[test]
fn a_world_that_baked_since_overwrites() {
    let (path, mut editor) = raced("overwrite");
    let theirs: Vec<_> = KINDS.iter().map(|k| read(k, &path)).collect();
    KINDS.iter().for_each(|k| (k.bake)(&mut editor, 3));
    editor.save_to_file(&path).unwrap();
    for (kind, theirs) in KINDS.iter().zip(theirs) {
        assert_ne!(
            read(kind, &path),
            theirs,
            "{}: the last write wins",
            kind.name
        );
    }
}

#[test]
fn save_as_writes_the_loaded_bake_to_the_new_path() {
    let path = level("save_as");
    let editor = opened(&path);
    let other = path.replace("level.scene", "copy.scene");
    editor.save_to_file(&other).unwrap();
    for kind in &KINDS {
        assert_eq!(read(kind, &other), read(kind, &path), "{}", kind.name);
    }
}

#[test]
fn a_cleared_bake_removes_only_the_file_this_world_saw() {
    let path = level("cleared");
    let mut editor = opened(&path);
    KINDS.iter().for_each(|k| (k.bake)(&mut editor, 0));
    editor.save_to_file(&path).unwrap();
    for kind in &KINDS {
        assert_eq!(read(kind, &path), None, "{}: removed", kind.name);
    }

    let (path, mut editor) = raced("cleared_raced");
    KINDS.iter().for_each(|k| (k.bake)(&mut editor, 0));
    editor.save_to_file(&path).unwrap();
    for kind in &KINDS {
        assert!(
            read(kind, &path).is_some(),
            "{}: another's file stays",
            kind.name
        );
    }
}
