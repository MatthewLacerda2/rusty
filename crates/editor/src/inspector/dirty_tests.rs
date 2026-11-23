//! Every widget of every card, poked: the scene reads dirty exactly when its saved document
//! changed. A new card is covered without a new test.

use std::collections::BTreeSet;

use rusty_edit::authoring::ComponentKind;
use rusty_edit::to_scene_data;
use rusty_scene::Scene;

#[path = "dirty_rig_tests.rs"]
mod rig;
use rig::{Key, Rig, Subject};

/// Less the selection, which the editor copies in each frame.
fn saved(scene: &Scene) -> String {
    let mut data = to_scene_data(scene);
    data.selected_entity_id = None;
    serde_json::to_string(&data).expect("a scene serializes")
}

/// How many widgets it poked, and how many changed the saved scene.
fn sweep(subject: Subject, skip: &BTreeSet<Key>, mut after: impl FnMut(&Rig)) -> (usize, usize) {
    let mut rig = Rig::new(subject);
    let targets: Vec<_> = rig
        .targets()
        .into_iter()
        .filter(|t| !skip.contains(&t.0))
        .collect();
    let mut changed = 0;
    for &(_, at, drags) in &targets {
        let before = saved(&rig.scene);
        rig.poke(at, drags);
        let edited = saved(&rig.scene) != before;
        let dirty = rig.editor.is_dirty;
        assert_eq!(
            dirty, edited,
            "{subject:?}: the widget at {at:?}, dirty iff saved"
        );
        changed += usize::from(edited);
        after(&rig);
        rig.reset();
    }
    (targets.len(), changed)
}

#[test]
fn scene_settings_edits_mark_the_scene() {
    let (poked, changed) = sweep(Subject::Settings, &BTreeSet::new(), |_| {});
    assert!(
        poked > 0 && changed > 0,
        "a sweep that changes nothing proves nothing"
    );
}

#[test]
fn the_box_cards_edits_mark_the_scene_the_mesh_culling_rows_too() {
    let mesh = |rig: &Rig| {
        let mesh = rig.scene.world.mesh(rig.id);
        mesh.map(|m| (m.max_draw_distance, m.min_screen_size))
    };
    let built = mesh(&Rig::new(Subject::Box)).expect("a box has a Mesh");
    let (mut distance, mut size) = (false, false);
    let (_, changed) = sweep(Subject::Box, &BTreeSet::new(), |rig| {
        let now = mesh(rig).unwrap_or(built);
        distance |= now.0 != built.0;
        size |= now.1 != built.1;
    });
    assert!(
        changed > 0 && distance && size,
        "the sweep reached both culling rows"
    );
}

/// A third of the kinds, so three tests sweep side by side.
fn added_cards(third: usize) {
    let boxed = Rig::new(Subject::Box).targets();
    let skip: BTreeSet<Key> = boxed.into_iter().map(|t| t.0).collect();
    let part = ComponentKind::ALL
        .chunks(ComponentKind::ALL.len().div_ceil(3))
        .nth(third);
    for &kind in part.unwrap_or_default() {
        let (poked, changed) = sweep(Subject::With(kind), &skip, |_| {});
        assert!(
            poked == 0 || changed > 0,
            "{kind:?}: its card's widgets changed nothing"
        );
    }
}

#[test]
fn the_first_third_of_the_added_cards_edits_mark_the_scene() {
    added_cards(0);
}

#[test]
fn the_second_third_of_the_added_cards_edits_mark_the_scene() {
    added_cards(1);
}

#[test]
fn the_last_third_of_the_added_cards_edits_mark_the_scene() {
    added_cards(2);
}

#[test]
fn a_write_of_the_value_already_there_leaves_the_scene_clean() {
    use rusty_edit::authoring::rendering::mesh as mesh_ops;
    let mut rig = Rig::new(Subject::Box);
    rig.scene.begin_edits();
    if let Some(mut mesh) = rig.scene.world.mesh_mut(rig.id) {
        let distance = mesh.max_draw_distance;
        mesh_ops::set_max_draw_distance(&mut mesh, distance);
    }
    rig.scene.commit_edits(None);
    assert!(!rig.editor.follow_edits(&mut rig.scene));
    assert!(!rig.editor.is_dirty);
}
