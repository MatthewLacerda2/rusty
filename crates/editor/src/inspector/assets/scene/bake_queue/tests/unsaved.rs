//! The queue and the user's edits, and the frames an untouched window still gets.

use super::{ask, editor, frames, states};
use rusty_dev::bake::queue::live::UNSAVED;
use rusty_dev::bake::queue::State;
use rusty_scene::lighting::bake_status::BakeStatus;
use rusty_scene::Scene;

#[test]
fn resume_over_an_unsaved_scene_is_refused_and_keeps_the_edit() {
    let (root, mut editor, mut scene) = editor("editor_queue_unsaved_origin");
    scene.add_entity("Unsaved".into());
    scene.mark_unsaved();
    ask(&root, &mut scene, &["a.scene"]);
    frames(&mut editor, &mut scene, |_| true);
    assert!(editor.dev.bake_queue.running.is_none());
    let BakeStatus::Refused(why) = &scene.lighting_bake_status else {
        panic!("{:?}", scene.lighting_bake_status)
    };
    assert!(why.contains(UNSAVED), "{why}");
    assert!(editor.is_dirty && scene.find_entity_by_name("Unsaved").is_some());
    assert!(editor
        .current_scene_path()
        .unwrap()
        .ends_with("start.scene"));
    assert_eq!(states(&root), [State::Pending]);
}

#[test]
fn gpu_an_edit_to_the_scene_in_hand_is_saved_when_its_bake_is_cancelled() {
    let (root, mut editor, mut scene) = editor("editor_queue_edit_cancel");
    ask(&root, &mut scene, &["a.scene", "b.scene"]);
    frames(&mut editor, &mut scene, |e| e.dev.lightmap_bake.is_some());
    scene.add_entity("Edited".into());
    scene.mark_unsaved();
    editor.dev.lightmap_bake.as_ref().unwrap().cancel();
    frames(&mut editor, &mut scene, |e| {
        e.dev.bake_queue.running.is_none()
    });
    let mut a = Scene::new();
    rusty_edit::load_from_file(&mut a, &root.join("a.scene").to_string_lossy()).unwrap();
    assert!(
        a.find_entity_by_name("Edited").is_some(),
        "the edit was dropped"
    );
    assert!(editor
        .current_scene_path()
        .unwrap()
        .ends_with("start.scene"));
}

#[test]
fn gpu_the_frame_a_scene_bake_lands_on_asks_for_the_next() {
    let (root, mut editor, mut scene) = editor("editor_queue_wakes");
    ask(&root, &mut scene, &["a.scene", "b.scene"]);
    frames(&mut editor, &mut scene, |e| e.dev.lightmap_bake.is_some());
    let rang = std::cell::Cell::new(rusty_core::wake::rings());
    // The landing frame clears the job; the queue's wake must ring in that same frame.
    frames(&mut editor, &mut scene, |e| {
        let landed = e.dev.lightmap_bake.is_none();
        if !landed {
            rang.set(rusty_core::wake::rings());
        }
        landed
    });
    assert!(rusty_core::wake::rings() > rang.get(), "no frame asked for");
}
