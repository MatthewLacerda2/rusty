use super::preview_tests::{preview_pages, run, showing};
use super::tests::{quick, scene, scene_path};
use super::*;

#[test]
fn an_entity_deleted_during_a_bake_is_gone_from_the_save_and_the_cancel() {
    let (mut scene, floor, _) = scene();
    let path = scene_path("preview_delete");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    assert!(scene.lightmaps.get(floor).is_some());
    let (mut job, _) = showing(&mut scene, &path);
    assert!(scene.lightmaps.get(floor).is_some(), "the preview shows it");
    scene.destroy_entity(floor);
    assert!(
        scene.lightmaps.get(floor).is_none(),
        "gone from the preview"
    );
    assert!(
        scene.lightmaps.saved().get(floor).is_none(),
        "and from what it hides"
    );
    while job.show_preview(&mut scene).is_none() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(scene.lightmaps.get(floor).is_none(), "the next pass too");
    let saved = rusty_edit::to_scene_data(&scene).lightmaps;
    assert!(
        saved.get(floor).is_none(),
        "a save meanwhile has no entry for it"
    );
    let name = |p: &String| p.rsplit('/').next().unwrap_or_default().to_string();
    assert!(saved
        .pages
        .iter()
        .all(|p| !name(p).starts_with("lightmap_preview_")));
    job.cancel();
    assert_eq!(run(&mut job, &mut scene).0, BakeOutcome::Cancelled);
    assert!(scene.lightmaps.shown_over.is_none());
    assert!(
        scene.lightmaps.get(floor).is_none(),
        "the cancel does not bring it back"
    );
    assert!(!scene.lightmaps.pages.is_empty(), "the old pages are back");
    assert_eq!(preview_pages(&path), 0);
}

#[test]
fn lightmaps_cleared_during_a_bake_stay_cleared_on_save_and_cancel() {
    let (mut scene, _, _) = scene();
    let path = scene_path("preview_clear");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let (mut job, _) = showing(&mut scene, &path);
    scene.lightmaps.clear();
    assert!(scene.lightmaps.saved().is_empty());
    let saved = rusty_edit::to_scene_data(&scene).lightmaps;
    assert!(
        saved.is_empty() && saved.pages.is_empty(),
        "a save writes none"
    );
    job.cancel();
    assert_eq!(run(&mut job, &mut scene).0, BakeOutcome::Cancelled);
    assert!(scene.lightmaps.is_empty() && scene.lightmaps.pages.is_empty());
    assert_eq!(preview_pages(&path), 0);
}

fn saved(scene: &Scene, wall: u32) -> Option<rusty_scene::lighting::lightmap_set::LightmapEntry> {
    rusty_edit::to_scene_data(scene)
        .lightmaps
        .get(wall)
        .cloned()
}

#[test]
fn an_undone_delete_after_a_bake_brings_the_wall_back_with_the_new_entry() {
    let (mut scene, wall, _) = scene();
    let path = scene_path("preview_delete_undo");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let mut stale = scene.lightmaps.get(wall).cloned().unwrap();
    stale.scale_offset = [9.0; 4];
    scene.lightmaps.insert(stale.clone());
    let settings = BakeSettings {
        samples: 64,
        samples_per_pass: 1,
        ..quick()
    };
    let mut job = LightmapBakeJob::start(&mut scene, Some(&path), settings, None).unwrap();
    scene.begin_edits();
    scene.destroy_entity(wall);
    scene.commit_edits(None);
    let (outcome, _) = run(&mut job, &mut scene);
    assert_eq!(outcome, BakeOutcome::Baked(0), "only live entities count");
    let new = scene
        .lightmaps
        .get(wall)
        .cloned()
        .expect("the bake's entry");
    assert_ne!(new, stale);
    assert_eq!(saved(&scene, wall), None, "a save names no dead entity");
    scene.undo();
    assert!(scene.world.contains(wall));
    assert_eq!(
        scene.lightmaps.get(wall),
        Some(&new),
        "the new bake's entry"
    );
    assert_eq!(saved(&scene, wall), Some(new));
}
