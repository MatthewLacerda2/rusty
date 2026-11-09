//! Bake Occlusion (#1958): Generate Lighting's occlusion step alone, from a
//! script, written beside the scene and touching nothing else.

use super::{bake_occlusion_only, UNSAVED};
use crate::dev::bake::progress::BakeLog;
use crate::dev::session::Session;
use crate::scene::lighting::occlusion::{fingerprint, occlusion_path, test_level::corridors};
use crate::scene::Scene;

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
    assert_eq!(unsaved.unwrap_err(), UNSAVED);
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
