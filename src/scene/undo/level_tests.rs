//! The scene level's comparison (#1631). Which fields it is made of is the scene
//! state inventory's authored list (`scene::state`, #1649).

#[test]
fn a_nan_probe_position_is_no_edit_on_an_idle_frame() {
    let mut scene = crate::scene::Scene::new();
    scene.probes.add_probe(glam::Vec3::NAN);
    scene.begin_edits();
    scene.pause_edits();
    assert!(!scene.commit_edits(None), "NaN never equals itself");
}

/// Marking a light probe manual is an edit Undo takes back (#1737).
#[test]
fn marking_a_probe_manual_undoes() {
    let mut scene = crate::scene::Scene::new();
    scene.probes.add_probe(glam::Vec3::ONE);
    scene.begin_edits();
    scene.probes.set_manual(0, true);
    assert!(scene.commit_edits(None), "the flag is an edit");
    scene.undo().expect("a step to undo");
    assert!(!scene.probes.probes[0].manual);
}
