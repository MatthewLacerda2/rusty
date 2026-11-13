//! The UI's texture-size reader inside the tick (#1241, #1256): a bare tick fills the table, an idle
//! one leaves it alone, and a Play or Stop measures the scene it swapped in.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec2;

use rusty::app::GameWorld;
use rusty::components::{
    CanvasComponent, ImageComponent, LayoutAxisFit, LayoutElementComponent, RectTransformComponent,
};
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::procgen::{bake::bake_to_png, Image, Slot};
use rusty::scene::Scene;

/// A scene of a canvas holding one Image of `texture`, fitted to its preferred size
/// (the crate's own `size_reader` tests build the same).
fn fitted_image(texture: &str) -> Scene {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    scene
        .world
        .set_canvas(root, Some(CanvasComponent::default()));
    let id = scene.add_entity("Icon".to_string());
    scene
        .world
        .set_rect_transform(id, Some(RectTransformComponent::default()));
    let fit = LayoutElementComponent {
        horizontal_fit: LayoutAxisFit::PreferredSize,
        vertical_fit: LayoutAxisFit::PreferredSize,
        ..LayoutElementComponent::default()
    };
    scene.world.set_layout_element(id, Some(fit));
    let img = ImageComponent {
        texture: Some(texture.into()),
        ..ImageComponent::default()
    };
    scene.world.set_image(id, Some(img));
    scene.set_parent(id, Some(root)).expect("parent exists");
    scene
}

/// A fresh path for a baked PNG named `name`.
fn temp_png(name: &str) -> String {
    let dir = crate::temp::dir().join("rusty_1221");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("{name}.png"));
    std::fs::remove_file(&path).ok();
    path.to_string_lossy().into_owned()
}

/// A game of `scene`, not ticked yet.
fn game(scene: Scene) -> GameWorld {
    let nav = NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
    GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(rusty::scripting::ConsoleLogs::new())),
    )
}

/// A game of one Image of a baked 4×4 PNG, and that PNG's path.
fn game_of_baked(name: &str) -> (GameWorld, String) {
    let path = temp_png(name);
    bake_to_png(&Image::new(4), Slot::BaseColor, &path).expect("bake");
    (game(fitted_image(&path)), path)
}

#[test]
fn a_bare_tick_reads_the_sizes_and_an_idle_one_copies_nothing() {
    let (mut game, path) = game_of_baked("ticked");
    // No refresh call beside the tick: the tick does it (#1241).
    game.tick(rusty::time::FIXED_DELTA_TIME);
    // Held as the renderer and the editor hold it, so a write would copy it.
    let held = Rc::clone(game.resources.screen.borrow().textures());
    assert_eq!(held.get(&path), Some(Vec2::splat(4.0)));
    game.tick(rusty::time::FIXED_DELTA_TIME);
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
