//! Generate Lighting writes the PVS file only once its PVS is baked (#1982's second
//! review): its lighting save leaves a newer file another process baked alone, so a
//! PVS step that never ends (an error, the editor quitting) loses nothing.

use super::{apply_occlusion, write_lighting};
use crate::dev::bake::occlusion::bake_occlusion_logged;
use crate::dev::bake::progress::BakeLog;
use crate::scene::lighting::occlusion::{occlusion_path, test_level::corridors};
use crate::scene::{LightingSave, Scene};

/// The corridors level saved at a fresh path, with no PVS beside it, opened.
fn opened(tag: &str) -> (String, Scene) {
    let dir = crate::test_temp::dir().join(format!("pvs_write_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    corridors().save_to_file(&path).unwrap();
    std::fs::remove_file(occlusion_path(&path)).ok();
    let mut scene = Scene::new();
    scene.load_from_file(&path).unwrap();
    (path, scene)
}

#[test]
fn the_lighting_save_keeps_a_newer_pvs_and_a_failed_pvs_step_loses_nothing() {
    let (path, mut editor) = opened("newer");
    let pvs = occlusion_path(&path);
    // Another process bakes the scene's PVS while the editor has it open.
    std::fs::write(&pvs, b"theirs").unwrap();
    assert_eq!(write_lighting(&editor, &path), Ok(LightingSave::Written));
    assert_eq!(std::fs::read(&pvs).unwrap(), b"theirs", "the lighting save");
    // The PVS step fails, so `finish` never runs: a plain save keeps the newer file.
    editor.ambient_intensity = 0.5;
    editor.save_to_file(&path).unwrap();
    assert_eq!(std::fs::read(&pvs).unwrap(), b"theirs", "a save after");
    assert!(editor.bake_files.warned(&pvs), "the save says it kept it");
    // A PVS step that ends writes this world's bake (the last write wins).
    let baked = bake_occlusion_logged(&editor, &mut BakeLog::off()).unwrap();
    apply_occlusion(&mut editor, &path, baked).unwrap();
    let ours = std::fs::read(&pvs).unwrap();
    assert_ne!(ours, b"theirs", "the bake is written");
    assert!(crate::scene::lighting::occlusion::decode(&ours).is_ok());
}
