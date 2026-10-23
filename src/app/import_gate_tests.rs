//! Play never converts a model (#1253): a source changed during Play is served
//! from its last import until Stop, when the first edit frame converts it and
//! every edit-scene instance follows (#1242). A standalone game never converts.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use glam::Vec3;

use crate::app::{GameWorld, PlayTransition};
use crate::asset::convert::{cache_location, conversion_allowed};
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::asset_instance::instantiate_asset;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;

/// A quad (6 indices) under the object name `Quad`.
const QUAD: &str = include_str!("../../tests/fixtures/models/embedded_textures.fbx");

/// One triangle (3 indices) under the same object name.
fn triangle() -> String {
    include_str!("../../tests/fixtures/models/missing_texture.fbx").replace("Tri", "Quad")
}

fn world() -> GameWorld {
    GameWorld::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    )
}

fn source() -> PathBuf {
    let path = crate::test_temp::dir().join("model.fbx");
    std::fs::write(&path, QUAD).unwrap();
    path
}

fn reference(path: &Path) -> String {
    format!("{}::Quad", path.display())
}

fn stamp(path: &Path) -> String {
    let cache = cache_location(path).unwrap();
    std::fs::read_to_string(format!("{}.source", cache.display())).unwrap()
}

fn spawn(gw: &GameWorld, path: &Path) -> Result<u32, String> {
    let mut scene = gw.world.scene.borrow_mut();
    instantiate_asset(&mut scene, &reference(path), None, Vec3::ZERO)
}

fn indices(gw: &GameWorld, id: u32) -> usize {
    gw.world
        .scene
        .borrow()
        .world
        .mesh(id)
        .unwrap()
        .indices
        .len()
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
fn the_standalone_boot_never_converts() {
    let mut gw = world();
    gw.boot_standalone();
    assert!(!conversion_allowed(), "off from boot, before Play enters");
    gw.tick(DT);
    gw.set_playing(false);
    gw.tick(DT);
    assert!(!conversion_allowed(), "and stays off");
}
