//! Unit tests for `GameWorld` (game.rs): the shared handles, play/stop
//! transitions, the scaled-dt threading, and the snapshot capture/restore boundary.
//! All drive the real `tick`, honouring the determinism contract (fixed dt, no
//! wall-clock, no RNG).

use super::*;
use crate::scene::Scene;
use std::cell::RefCell;
use std::rc::Rc;

/// A bare play-mode world over `scene`; nav grid spans the demo bounds.
fn world_with(scene: Scene) -> GameWorld {
    let scene = Rc::new(RefCell::new(scene));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -20.0, 20.0, -20.0, 20.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    GameWorld::new(scene, input, nav, console)
}

fn empty_world() -> GameWorld {
    world_with(Scene::new())
}

#[test]
fn accessors_expose_shared_handles() {
    let gw = empty_world();
    // Each accessor returns the very cell the resources hold (same allocation).
    assert!(Rc::ptr_eq(gw.scene(), &gw.world.scene));
    assert!(Rc::ptr_eq(gw.input(), &gw.resources.input));
    assert!(Rc::ptr_eq(gw.nav(), &gw.resources.nav));
    assert!(Rc::ptr_eq(gw.console(), &gw.resources.console));
    assert!(Rc::ptr_eq(gw.camera(), &gw.resources.camera));
    assert!(Rc::ptr_eq(gw.time(), &gw.resources.time));
    assert!(!gw.is_playing());
    assert_eq!(gw.play_frame(), 0);
    let _ = gw.script_manager();
}

#[test]
fn set_playing_flips_state() {
    let mut gw = empty_world();
    assert!(!gw.is_playing());
    gw.set_playing(true);
    assert!(gw.is_playing());
    gw.set_playing(false);
    assert!(!gw.is_playing());
}
