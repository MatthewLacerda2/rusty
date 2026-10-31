//! Undo by hand (#1631): what the inspector, the gizmo and a card do, as World
//! writes between `begin_edits` and a commit, the way the editor's frame runs them.

use glam::Vec3;

use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{to_scene_data, Scene};

fn document(scene: &Scene) -> String {
    serde_json::to_string(&to_scene_data(scene)).expect("a scene serializes")
}

fn scene_with_crate() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Crate", Some(Primitive::Box));
    (scene, id)
}

/// One hand edit: `edit` between an opened and a committed step.
fn edit(scene: &mut Scene, edit: impl FnOnce(&mut Scene)) {
    scene.begin_edits();
    edit(scene);
    scene.commit_edits(None);
}

fn move_to(scene: &mut Scene, id: u32, x: f32) {
    scene.world.transform_mut(id).expect("live").position = Vec3::new(x, 0.0, 0.0);
}

#[test]
fn an_inspector_edit_undoes_and_redoes_to_identical_documents() {
    let (mut scene, id) = scene_with_crate();
    let before = document(&scene);
    edit(&mut scene, |s| move_to(s, id, 4.0));
    let after = document(&scene);
    assert_eq!(scene.undo_name(), Some("Edit Crate"));
    assert_eq!(scene.undo().as_deref(), Some("Edit Crate"));
    assert_eq!(document(&scene), before);
    assert_eq!(scene.redo().as_deref(), Some("Edit Crate"));
    assert_eq!(document(&scene), after);
}

#[test]
fn a_drag_over_several_frames_is_one_step() {
    let (mut scene, id) = scene_with_crate();
    let before = document(&scene);
    for x in [1.0, 2.0, 3.0] {
        scene.begin_edits(); // each frame of the drag resumes the open edit
        move_to(&mut scene, id, x);
        scene.pause_edits();
    }
    scene.commit_edits(None); // the pointer is released
    scene.undo();
    assert_eq!(
        document(&scene),
        before,
        "one Undo takes the whole drag back"
    );
    assert_eq!(scene.undo(), None);
}

#[test]
fn a_follow_up_frame_joins_the_last_step() {
    let (mut scene, id) = scene_with_crate();
    let before = document(&scene);
    edit(&mut scene, |s| move_to(s, id, 1.0));
    // An idle frame that writes nothing records nothing, however the level moved.
    scene.skybox_path = "baked.hdr".into();
    scene.begin_edits();
    scene.commit_follow_up();
    scene.skybox_path.clear();
    // The next frame folds the gesture's leftovers in (a card's library insert).
    scene.begin_edits();
    scene.materials.insert("Folded".into(), Default::default());
    scene.world.set_name(id, "Renamed".into());
    scene.commit_follow_up();
    scene.undo();
    assert_eq!(
        document(&scene),
        before,
        "the follow-up went with its gesture"
    );
    assert_eq!(scene.undo(), None);
}

#[test]
fn scene_level_edits_are_recorded_as_one_more_object() {
    let (mut scene, _) = scene_with_crate();
    let before = document(&scene);
    edit(&mut scene, |s| {
        s.ambient_intensity = 0.1;
        s.fog.density = 0.5;
    });
    assert_eq!(scene.undo_name(), Some("Scene Settings"));
    scene.undo();
    assert_eq!(document(&scene), before);
    // A change made outside an edit (a bake landing) is no step of the next one.
    scene.skybox_path = "sky.hdr".into();
    edit(&mut scene, |_| {});
    assert_eq!(scene.undo_name(), None, "Redo is kept: nothing was edited");
    assert_eq!(scene.redo_name(), Some("Scene Settings"));
}

#[test]
fn an_edit_that_changes_nothing_saved_records_nothing() {
    let (mut scene, id) = scene_with_crate();
    edit(&mut scene, |s| {
        drop(s.world.mesh_mut(id)); // borrowed mutably, nothing written
        s.world.mesh_mut(id).expect("mesh").vertices.clear(); // runtime-only
        move_to(s, id, 0.0); // written, but to the value it had
    });
    assert_eq!(scene.undo_name(), None);
}

#[test]
fn geometry_is_kept_for_the_same_source_and_rebuilt_for_another() {
    let (mut scene, id) = scene_with_crate();
    let box_vertices = scene.world.mesh(id).expect("mesh").vertices.len();
    let revision = scene.world.mesh(id).expect("mesh").revision;
    edit(&mut scene, |s| move_to(s, id, 2.0));
    scene.undo();
    let mesh = scene.world.mesh(id).expect("mesh");
    assert_eq!(
        mesh.revision, revision,
        "the live geometry was kept, not re-uploaded"
    );
    drop(mesh);
    edit(&mut scene, |s| {
        let sphere = crate::scene::authoring::primitive_mesh_component(Primitive::Sphere);
        s.world.set_mesh(id, sphere);
    });
    scene.undo();
    let mesh = scene.world.mesh(id).expect("mesh");
    assert_eq!(
        (mesh.primitive_type.as_str(), mesh.vertices.len()),
        ("Box", box_vertices)
    );
}

#[test]
fn the_history_keeps_its_last_steps_and_play_clears_it() {
    let (mut scene, id) = scene_with_crate();
    for i in 0..=super::DEPTH {
        edit(&mut scene, |s| move_to(s, id, i as f32 + 1.0));
    }
    let undone = std::iter::from_fn(|| scene.undo()).count();
    assert_eq!(undone, super::DEPTH);
    scene.redo();
    scene.set_playing(true);
    assert_eq!((scene.undo_name(), scene.redo_name()), (None, None));
    edit(&mut scene, |s| move_to(s, id, 9.0));
    assert_eq!(scene.undo(), None, "nothing is recorded in Play");
}
