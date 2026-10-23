//! Play never converts a model (#1253): a source changed during Play is served
//! from its last import until Stop, when the first edit frame converts it and
//! every edit-scene instance follows (#1242). The standalone game holds the same.

use super::{assets, indices, spawn, stamp, triangle, world, DT, QUAD};
use crate::app::PlayTransition;
use crate::asset::convert::conversion_allowed;
use std::path::PathBuf;

fn source() -> PathBuf {
    let path = crate::test_temp::dir().join("model.fbx");
    std::fs::write(&path, QUAD).unwrap();
    path
}

#[test]
fn a_source_changed_in_play_converts_only_after_stop() {
    let path = source();
    let mut gw = world();
    let edit = spawn(&gw, &path).unwrap();
    let before = stamp(&path);

    gw.set_playing(true);
    assert!(gw.tick(DT) == PlayTransition::Entered);
    assert!(!conversion_allowed(), "Play turns conversion off");
    std::fs::write(&path, triangle()).unwrap();
    let first = spawn(&gw, &path).unwrap();
    let second = spawn(&gw, &path).unwrap();
    gw.tick(DT);
    assert_eq!(stamp(&path), before, "no conversion ran in Play");
    for id in [edit, first, second] {
        assert_eq!(indices(&gw, id), 6, "every instance has the last import");
    }

    gw.set_playing(false);
    assert!(gw.tick(DT) == PlayTransition::Exited);
    assert!(conversion_allowed(), "Stop turns it back on");
    assert_ne!(stamp(&path), before, "the first edit frame converts");
    gw.tick(DT);
    assert_eq!(
        indices(&gw, edit),
        3,
        "and the edit scene's instance follows"
    );
}

#[test]
fn a_source_never_imported_fails_in_play_with_a_console_line() {
    crate::logging::init();
    let path = source();
    let mut gw = world();
    gw.set_playing(true);
    gw.tick(DT);

    let err = spawn(&gw, &path).unwrap_err();
    assert!(err.contains("import it in edit mode first"), "{err}");
    let lines = crate::logging::forward(gw.console());
    assert!(
        lines
            .iter()
            .any(|l| l.contains("import it in edit mode first")),
        "{lines:?}"
    );
}

#[test]
fn the_standalone_boot_refreshes_then_holds_like_the_editor() {
    let (root, enemy) = assets();
    let mut gw = world();
    gw.resources.sources_root = root;
    gw.boot_standalone();
    assert!(conversion_allowed(), "nothing held before Play enters");
    assert!(gw.tick(DT) == PlayTransition::Entered);
    assert!(!stamp(&enemy).is_empty(), "the refresh imported the source");
    assert!(!conversion_allowed(), "and Play holds conversion off");
    std::fs::write(&enemy, triangle()).unwrap();
    let id = spawn(&gw, &enemy).unwrap();
    assert_eq!(indices(&gw, id), 6, "a running game plays the last import");
}
