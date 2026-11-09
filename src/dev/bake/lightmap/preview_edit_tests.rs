//! Edits to a scene while a progressive bake's preview is shown (#1970's review):
//! a despawn and `Lighting.ClearLightmaps()` reach the scene's own lightmaps
//! hidden under the preview, so a save and a cancel both see them.

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
    let saved = crate::scene::to_scene_data(&scene).lightmaps;
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
    // What `Lighting.ClearLightmaps()` does.
    scene.lightmaps.clear();
    assert!(scene.lightmaps.saved().is_empty());
    let saved = crate::scene::to_scene_data(&scene).lightmaps;
    assert!(
        saved.is_empty() && saved.pages.is_empty(),
        "a save writes none"
    );
    job.cancel();
    assert_eq!(run(&mut job, &mut scene).0, BakeOutcome::Cancelled);
    assert!(scene.lightmaps.is_empty() && scene.lightmaps.pages.is_empty());
    assert_eq!(preview_pages(&path), 0);
}

#[test]
fn a_bake_finished_after_a_delete_gives_the_deleted_entity_no_lightmap() {
    let (mut scene, floor, _) = scene();
    let path = scene_path("preview_delete_done");
    let settings = BakeSettings {
        samples: 64,
        samples_per_pass: 1,
        ..quick()
    };
    let mut job = LightmapBakeJob::start(&mut scene, Some(&path), settings, None).unwrap();
    scene.destroy_entity(floor);
    let (outcome, _) = run(&mut job, &mut scene);
    assert!(matches!(outcome, BakeOutcome::Baked(_)), "{outcome:?}");
    assert!(scene.lightmaps.get(floor).is_none());
}
