//! The window title (#1914): `<project> - <scene>`, `*` while unsaved, and the mark
//! kept to the edit scene across Play and a scene swap.

use super::{text, Title};
use crate::scene::Scene;

fn title() -> Title {
    Title {
        project: "Neon".to_string(),
        shown: String::new(),
        scene: None,
        dirty_at_play: None,
    }
}

#[test]
fn names_the_project_and_scene_and_marks_unsaved() {
    assert_eq!(text("Neon", "Dust", false), "Neon - Dust");
    assert_eq!(text("Neon", "Untitled", true), "Neon - Untitled*");
}

#[test]
fn play_edits_leave_the_edit_scene_mark_alone() {
    let (mut t, id) = (title(), Scene::new().id());
    for before in [false, true] {
        let mut dirty = before;
        t.track(&mut dirty, false, id);
        t.track(&mut dirty, true, id);
        dirty = !before; // an edit in Play, or a save refused there
        t.track(&mut dirty, true, id);
        t.track(&mut dirty, false, id);
        assert_eq!(dirty, before, "Stop puts back the mark Play found");
    }
}

#[test]
fn a_scene_swapped_in_outside_play_starts_clean() {
    let mut t = title();
    let mut dirty = true;
    t.track(&mut dirty, false, Scene::new().id());
    assert!(dirty, "the first frame only notes the scene");
    t.track(&mut dirty, false, Scene::new().id());
    assert!(!dirty, "another scene: nothing of it is unsaved yet");
}
