//! The reader inside the tick (#1241, #1256): a bare tick fills the table, an idle
//! one leaves it alone, and a Play or Stop measures the scene it swapped in.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec2;

use super::tests::{fitted_image, temp_png};
use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::procgen::{bake::bake_to_png, Image, Slot};
use crate::scene::Scene;

/// A game of `scene`, not ticked yet.
fn game(scene: Scene) -> GameWorld {
    let nav = NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
    GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(crate::scripting::ConsoleLogs::new())),
    )
}

/// A game of one Image of a baked 4×4 PNG, and that PNG's path.
fn game_of_baked(name: &str) -> (GameWorld, String) {
    let path = temp_png(name);
    bake_to_png(&Image::new(4), Slot::BaseColor, &path).expect("bake");
    (game(fitted_image(&path).0), path)
}

#[test]
fn a_bare_tick_reads_the_sizes_and_an_idle_one_copies_nothing() {
    let (mut game, path) = game_of_baked("ticked");
    // No refresh call beside the tick: the tick does it (#1241).
    game.tick(crate::time::FIXED_DELTA_TIME);
    // Held as the renderer and the editor hold it, so a write would copy it.
    let held = Rc::clone(game.resources.screen.borrow().textures());
    assert_eq!(held.get(&path), Some(Vec2::splat(4.0)));
    game.tick(crate::time::FIXED_DELTA_TIME);
    let now = Rc::clone(game.resources.screen.borrow().textures());
    assert!(
        Rc::ptr_eq(&held, &now),
        "nothing changed: the table is not copied"
    );
}

#[test]
fn a_paused_play_measures_the_scene_it_entered() {
    let (mut game, path) = game_of_baked("paused_play");
    game.set_playing(true);
    // A paused frame: the transition runs, nothing steps.
    game.poll_transition();
    let sizes = Rc::clone(game.resources.screen.borrow().textures());
    assert_eq!(sizes.get(&path), Some(Vec2::splat(4.0)));
}
