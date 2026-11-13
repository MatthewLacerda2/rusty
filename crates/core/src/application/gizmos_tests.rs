use super::*;

/// Unity's defaults: UI and the probes on, every collider and the navmesh off; the
/// navmesh's toggle shown (#1910).
#[test]
fn defaults_follow_unity() {
    let set = SceneGizmos::default();
    assert!(set.on(Gizmo::Ui));
    assert!(set.on(Gizmo::LightProbes));
    assert!(set.on(Gizmo::ReflectionProbes));
    assert!(!set.on(Gizmo::Colliders));
    assert!(Gizmo::Colliders.row().shown);
    assert!(!set.on(Gizmo::NavMesh));
    assert!(Gizmo::NavMesh.row().shown);
    assert!(
        !set.on(Gizmo::GameGizmos),
        "Unity's Game view starts without"
    );
    let game: Vec<_> = Gizmo::ALL.iter().filter(|g| g.row().game).collect();
    assert_eq!(game, [&Gizmo::GameGizmos], "the Game tab's one toggle");
}

/// Every row's name finds its own gizmo, and the names are unique.
#[test]
fn names_round_trip() {
    for &gizmo in Gizmo::ALL {
        assert_eq!(Gizmo::named(gizmo.row().name), Some(gizmo));
    }
    assert_eq!(Gizmo::named("navmesh"), None, "names are case-sensitive");
    assert_eq!(Gizmo::named("colliders"), None);
}

/// A script's request reads back at once, reaches the editor's set at its frame, and
/// the editor's own change (a toggle click) is mirrored back.
#[test]
fn requests_reach_the_editor_and_mirror_back() {
    let mut app = Application::new();
    let mut editor = SceneGizmos::default();
    app.request_gizmo(Gizmo::NavMesh, true);
    app.request_gizmo(Gizmo::LightProbes, false);
    assert!(
        app.scene_gizmos().on(Gizmo::NavMesh),
        "read back before the frame"
    );
    assert!(!editor.on(Gizmo::NavMesh));

    app.sync_gizmos(&mut editor);
    assert!(editor.on(Gizmo::NavMesh));
    assert!(!editor.on(Gizmo::LightProbes));

    *editor.flag_mut(Gizmo::Ui) = false; // a click on the UI toggle
    app.sync_gizmos(&mut editor);
    assert!(!app.scene_gizmos().on(Gizmo::Ui));
    assert!(
        app.scene_gizmos().on(Gizmo::NavMesh),
        "requests are taken once"
    );
}

/// Two requests in one frame land in order: the later wins.
#[test]
fn the_later_request_wins() {
    let mut app = Application::new();
    let mut editor = SceneGizmos::default();
    app.request_gizmo(Gizmo::ReflectionProbes, false);
    app.request_gizmo(Gizmo::ReflectionProbes, true);
    app.sync_gizmos(&mut editor);
    assert!(editor.on(Gizmo::ReflectionProbes));
}
