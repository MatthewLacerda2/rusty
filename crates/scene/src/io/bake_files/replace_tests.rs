//! The bake files a save replaces (#1982's review): a new scene saved over a baked
//! one drops that scene's bake files, a re-bake that gives the loaded bytes still
//! wins over a newer file, and the SH sidecar always travels with its document, so
//! SH never lands on probes it was not baked for.

use std::path::PathBuf;
use std::sync::Arc;

use glam::{UVec3, Vec3};

use crate::lighting::io::sidecar_path;
use crate::lighting::occlusion::{occlusion_path, Grid, OcclusionData};
use crate::lighting::sh::Sh9;
use crate::nav_settings::navmesh_path;
use crate::Scene;

fn pvs(cell: f32) -> OcclusionData {
    let grid = Grid {
        origin: Vec3::ZERO,
        cell,
        dims: UVec3::ONE,
    };
    OcclusionData::new(grid, Vec::new(), Vec::new(), vec![0]).expect("one cell")
}

/// A saved scene at `<name>/level.scene` with every sidecar baked; its path.
fn baked(name: &str) -> String {
    let dir = rusty_test_support::temp::dir()
        .join("bake_replace")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    scene.probes.add_probe(Vec3::ZERO);
    scene.probes.probes_mut()[0].sh.coeffs[0] = [1.0; 3];
    scene.put_baked_occlusion(Some(pvs(1.0)));
    scene.put_baked_navmesh(Arc::from(vec![1u8; 4]));
    scene.save_to_file(&path).unwrap();
    path
}

fn sidecars(path: &str) -> [PathBuf; 3] {
    [occlusion_path(path), navmesh_path(path), sidecar_path(path)]
}

fn opened(path: &str) -> Scene {
    let mut scene = Scene::new();
    scene.load_from_file(path).unwrap();
    scene
}

#[test]
fn a_new_scene_saved_over_a_baked_one_drops_its_bake_files() {
    let path = baked("new_over");
    assert!(sidecars(&path).iter().all(|p| p.exists()));
    // File ▸ New (and Reset Scene) clear the scene, its record included.
    let mut scene = opened(&path);
    scene.clear();
    scene.save_to_file(&path).unwrap();
    for file in sidecars(&path) {
        assert!(!file.exists(), "{} is the old scene's", file.display());
    }
    let back = opened(&path);
    assert!(back.occlusion.is_none() && back.navmesh.bytes.is_none());
}

#[test]
fn a_rebake_of_the_loaded_bytes_wins_over_a_newer_file() {
    let path = baked("rebake");
    let mut editor = opened(&path);
    let mut headless = opened(&path);
    headless.put_baked_occlusion(Some(pvs(2.0)));
    headless.put_baked_navmesh(Arc::from(vec![2u8; 4]));
    headless.save_to_file(&path).unwrap();
    // The editor re-bakes an unchanged scene: the bytes it loaded.
    editor.put_baked_occlusion(Some(pvs(1.0)));
    editor.put_baked_navmesh(Arc::from(vec![1u8; 4]));
    editor.save_to_file(&path).unwrap();
    let back = opened(&path);
    assert_eq!(back.occlusion.as_ref().map(|d| d.grid.cell), Some(1.0));
    assert_eq!(back.navmesh.bytes.as_deref(), Some(&[1u8; 4][..]));
}

#[test]
fn sh_is_saved_with_its_probes_and_never_zipped_onto_others() {
    let path = baked("sh");
    let editor = opened(&path);
    // Another world re-places the probes and bakes them (Generate Lighting).
    let mut headless = opened(&path);
    headless.probes.add_probe(Vec3::X);
    headless.probes.probes_mut()[1].sh.coeffs[0] = [2.0; 3];
    headless.save_to_file(&path).unwrap();
    // The editor saves its own document: its SH goes with it.
    editor.save_to_file(&path).unwrap();
    let back = opened(&path);
    assert_eq!(back.probes.probes().len(), 1);
    assert_eq!(back.probes.probes()[0].sh.coeffs[0], [1.0; 3]);

    // A sidecar saved for other probes is refused, not applied by index.
    let mut moved = opened(&path);
    moved.probes.probes_mut()[0].position = Vec3::Y;
    let sidecar = std::fs::read(sidecar_path(&path)).unwrap();
    moved.save_to_file(&path).unwrap();
    std::fs::write(sidecar_path(&path), sidecar).unwrap();
    let back = opened(&path);
    assert_eq!(back.probes.probes()[0].position, Vec3::Y);
    assert_eq!(back.probes.probes()[0].sh, Sh9::zero(), "refused");
}
