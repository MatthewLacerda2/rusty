//! The Game view's own size through the editor's real draw and the input
//! path: the dropdown's size letterboxes the image, and a pointer on the image maps
//! into the game's pixels where it is drawn, as `Input.GetMousePosition` reads them.

use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::input::write_cursor_moved;
use rusty_core::application::{GameViewSetting, GameViewSize};
use rusty_core::input::InputState;
use rusty_editor::viewport::game_size;
use rusty_editor::EditorUi;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

/// The editor drawn on a 1600x900 window at 1x, Game tab open, with `size`.
fn drawn(size: GameViewSize) -> ViewportInteraction {
    drawn_on(ViewportTab::Game, size)
}

/// The editor drawn on a 1600x900 window at 1x, `tab` open, with `size`.
fn drawn_on(tab: ViewportTab, size: GameViewSize) -> ViewportInteraction {
    let ctx = egui::Context::default();
    let mut ui = EditorUi::new(Rc::new(RefCell::new(None)));
    ui.viewport_tab = tab;
    ui.game_view = GameViewSetting {
        size,
        low_resolution: false,
    };
    let (mut scene, mut console) = (Scene::new(), ConsoleLogs::new());
    let mut nav = NavigationGraph::from_scene(&scene);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 900.0));
    let mut last = None;
    for frame in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            time: Some(f64::from(frame)),
            ..Default::default()
        };
        ctx.begin_pass(input);
        let texture = Some(egui::TextureId::User(1));
        let playing = &mut false;
        last = Some(ui.draw(&ctx, &mut scene, &mut console, &mut nav, playing, texture));
        ctx.end_pass().textures_delta.clear();
    }
    last.expect("drawn")
}

#[test]
fn a_fixed_size_maps_the_pointer_where_it_is_drawn() {
    let interaction = drawn(GameViewSize::Fixed(640, 480));
    assert_eq!(interaction.render, Some((640, 480)));
    let (w, h) = (interaction.size.x, interaction.size.y);
    assert!(
        (w / h - 4.0 / 3.0).abs() < 0.01,
        "letterboxed at 4:3: {w}x{h}"
    );
    let rect = game_view(&interaction, 1.0).expect("the Game tab");
    assert_eq!(rect.render, (640, 480));

    let game = rusty_app::GameWorld::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::from_scene(&Scene::new()))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    let (x, y) = rect.origin;
    let centre = (x + rect.size.0 / 2.0, y + rect.size.1 / 2.0);
    write_cursor_moved(centre, &rect, &game);
    let (mx, my) = game.input().borrow().mouse_position();
    assert!(
        (mx - 320.0).abs() < 0.5 && (my - 240.0).abs() < 0.5,
        "{mx}, {my}"
    );
    write_cursor_moved((x, y), &rect, &game);
    assert_eq!(game.input().borrow().mouse_position(), (0.0, 0.0));
    write_cursor_moved((x - 10.0, centre.1), &rect, &game);
    assert!(
        game.input().borrow().mouse_position().0 < 0.0,
        "a bar is off screen"
    );
}

#[test]
fn an_aspect_fills_the_panel_and_free_is_the_panel() {
    let aspect = drawn(GameViewSize::Aspect(1, 1));
    let (w, h) = aspect.render.expect("a size of its own");
    assert_eq!(w, h, "square");
    assert_eq!(
        (w as f32, h as f32),
        (aspect.size.x.round(), aspect.size.y.round())
    );

    let free = drawn(GameViewSize::Free);
    assert_eq!(free.render, None);
    let rect = game_view(&free, 2.0).expect("the Game tab");
    let image = (free.size.x * 2.0).round() as u32;
    assert_eq!(rect.render.0, image, "the image's physical pixels");
    assert!(free.size.x > free.size.y, "the whole panel, unboxed");
}

/// Behind the Scene tab the same draw carries the Game view's size (what the UI
/// lays out at), renders nothing boxed, and boxes the UI overlay at its aspect.
#[test]
fn the_scene_tab_carries_the_game_views_size_from_its_draw() {
    let scene = drawn_on(ViewportTab::Scene, GameViewSize::Fixed(640, 480));
    assert_eq!((scene.render, scene.screen), (None, Some((640, 480))));
    let panel = (scene.size.x.round() as u32, scene.size.y.round() as u32);
    let (x, y, w, h) = game_size::overlay_box(&scene, panel).expect("boxed");
    assert!((w as f32 / h as f32 - 4.0 / 3.0).abs() < 0.01, "{w}x{h}");
    assert!(
        y == 0 && (x * 2 + w).abs_diff(panel.0) <= 1,
        "centred: {x}, {y}"
    );
    let free = drawn_on(ViewportTab::Scene, GameViewSize::Free);
    assert_eq!(free.screen, None);
    assert_eq!(game_size::overlay_box(&free, panel), None);
}
