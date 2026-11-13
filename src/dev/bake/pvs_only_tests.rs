//! Bake Occlusion (#1958): Generate Lighting's occlusion step alone, from a
//! script, written beside the scene and touching nothing else; and Generate
//! Lighting writes the PVS file only once its PVS is baked (#1982).

use super::{apply_occlusion, write_lighting};
use super::{bake_occlusion_only, OcclusionOnlyReport, UNSAVED_OCCLUSION};
use crate::dev::bake::progress::BakeLog;
use crate::dev::session::Session;
use crate::scene::lighting::occlusion::OcclusionBakeReport;
use crate::scene::lighting::occlusion::{fingerprint, occlusion_path, test_level::corridors};
use crate::scene::{MaterialComponent, Scene};
use crate::scripting::LogLevel;

/// The corridors level saved at a fresh path.
fn saved(tag: &str) -> String {
    let dir = crate::test_temp::dir().join(format!("occlusion_only_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    corridors().save_to_file(&path).unwrap();
    path
}

#[test]
fn it_writes_only_the_pvs_and_a_fresh_load_matches_it() {
    let path = saved("writes");
    let scene_text = std::fs::read(&path).unwrap();
    let mut scene = Scene::new();
    scene.load_from_file(&path).unwrap();
    let report = bake_occlusion_only(&mut scene, Some(&path), &mut BakeLog::off()).unwrap();
    assert!(report.occlusion.cells > 0 && report.occlusion.occluders > 0);
    assert!(report
        .summary()
        .starts_with("Baked occlusion, occlusion over"));
    assert!(scene.occlusion.is_some(), "applied");
    let pvs = std::fs::read(occlusion_path(&path)).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        scene_text,
        "the scene file untouched"
    );

    let mut back = Scene::new();
    back.load_from_file(&path).unwrap();
    let data = back.occlusion.as_deref().expect("the PVS loads");
    for baked in &data.objects {
        assert_eq!(fingerprint(&back, baked.entity), Some(baked.fingerprint));
    }
    bake_occlusion_only(&mut back, Some(&path), &mut BakeLog::off()).unwrap();
    let again = std::fs::read(occlusion_path(&path)).unwrap();
    assert_eq!(again, pvs, "a re-bake of the unchanged scene");
}

#[test]
fn it_is_refused_unsaved_and_while_the_editor_bakes() {
    let mut scene = corridors();
    let unsaved = bake_occlusion_only(&mut scene, None, &mut BakeLog::off());
    assert_eq!(unsaved.unwrap_err(), UNSAVED_OCCLUSION);
    scene.lighting_bake_running = true;
    let path = saved("refused");
    let busy = bake_occlusion_only(&mut scene, Some(&path), &mut BakeLog::off());
    assert!(busy.is_err() && scene.occlusion.is_none());
}

#[test]
fn the_script_verb_bakes_the_open_scene() {
    let path = saved("verb");
    let s = Session::new(&path).unwrap();
    let read = "local r = Lighting.BakeOcclusion() \
        return tostring(r.cells > 0 and r.occluders > 0 and r.objects >= r.occluders)";
    assert_eq!(s.eval(read).unwrap(), "true");
    assert!(occlusion_path(&path).is_file());
    let console = s.world().console().borrow();
    let said = console
        .messages
        .iter()
        .any(|(m, _)| m.starts_with("Baked occlusion"));
    assert!(said, "the summary reached the console");
}

#[test]
fn a_level_too_large_to_cull_claims_nothing_saved() {
    let occlusion = OcclusionBakeReport {
        objects: 3,
        occluders: 2,
        too_large: true,
        ..OcclusionBakeReport::default()
    };
    let line = OcclusionOnlyReport { occlusion, ms: 5 }.summary();
    assert!(line.contains("no occlusion culling"), "{line}");
    assert!(!line.ends_with("; saved"), "{line}");
}

/// The script's material, two-sided, is not in the file: the bake says so once.
#[test]
fn the_script_verb_warns_of_a_material_the_file_lacks() {
    let path = saved("warns");
    let mut level = Scene::new();
    level.load_from_file(&path).unwrap();
    for id in level.world.ids_with_mesh() {
        let glass = Some(MaterialComponent::new("glass"));
        level.world.set_material(id, glass);
    }
    level.save_to_file(&path).unwrap();
    let s = Session::new(&path).unwrap();
    s.eval(r#"Material.DefineAsset("glass", { render_face = "Both" })"#)
        .unwrap();
    s.eval("Lighting.BakeOcclusion()").unwrap();
    let console = s.world().console().borrow();
    let warned = console.messages.iter().filter(|(m, level)| {
        *level == LogLevel::Warning && m.starts_with(r#"[Occlusion] material(s) "glass""#)
    });
    assert_eq!(warned.count(), 1, "{:?}", console.messages);
}

#[test]
fn the_lighting_save_keeps_a_newer_pvs_and_a_failed_pvs_step_loses_nothing() {
    let path = saved("newer");
    let pvs = occlusion_path(&path);
    std::fs::remove_file(&pvs).ok();
    let mut editor = Scene::new();
    editor.load_from_file(&path).unwrap();
    // Another process bakes the PVS while the editor has the scene open.
    std::fs::write(&pvs, b"theirs").unwrap();
    let lit = write_lighting(&editor, &path);
    assert_eq!(lit, Ok(crate::scene::LightingSave::Written));
    assert_eq!(std::fs::read(&pvs).unwrap(), b"theirs", "the lighting save");
    // The PVS step fails, so `finish` never runs: a plain save keeps the newer file.
    editor.ambient_intensity = 0.5;
    editor.save_to_file(&path).unwrap();
    assert_eq!(std::fs::read(&pvs).unwrap(), b"theirs", "a save after");
    assert!(editor.bake_files().warned(&pvs), "the save says it kept it");
    // A PVS step that ends writes this world's bake (the last write wins).
    let baked = super::super::occlusion::bake_occlusion_logged(&editor, &mut BakeLog::off());
    apply_occlusion(&mut editor, &path, baked.unwrap()).unwrap();
    let ours = std::fs::read(&pvs).unwrap();
    assert!(
        crate::scene::lighting::occlusion::decode(&ours).is_ok(),
        "ours"
    );
}
