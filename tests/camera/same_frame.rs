//! Same-frame readers of the main view (#1619): moving the camera entity by
//! Transform, or its lens through `Camera.SetFov`, is seen by `Camera.WorldToScreen`
//! and a world-anchored `UI.GetRect` at once, with no tick in between; outside
//! Play, the UI still takes the entity's lens.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::{CanvasComponent, RectTransformComponent};
use rusty::scene::authoring::default_camera;
use rusty::scene::camera::set_view_fov;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

/// A 1280×720 session: a `MainCamera` at the origin looking down -Z and a HUD
/// marker pinned to the world point (1, 0, -10). Returns the world and the ids.
fn session() -> (GameWorld, [u32; 3]) {
    let mut s = Scene::new();
    let cam = s.add_entity("MainCamera".to_string());
    s.world.set_camera(cam, Some(default_camera()));
    let hud = s.add_entity("Hud".to_string());
    s.world.set_canvas(hud, Some(CanvasComponent::default()));
    let marker = s.add_entity("Marker".to_string());
    let rt = RectTransformComponent::default();
    s.world.set_rect_transform(marker, Some(rt));
    s.set_parent(marker, Some(hud)).expect("parent");
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut game = crate::play::world(Rc::new(RefCell::new(s)), 10.0, console);
    game.resources.screen.borrow_mut().set_game_view(1280, 720);
    game.set_playing(true);
    crate::play::run(&mut game, 1);
    let eval = |line: &str| game.script_manager().eval(line).expect("evaluates");
    eval(&format!(
        "RectTransform.SetWorldAnchor({marker}, nil, 1, 0, -10)"
    ));
    (game, [cam, hud, marker])
}

/// The marker's centre and where `Camera.WorldToScreen` puts its point, both in the
/// HUD's reference units.
fn read(game: &GameWorld, [_, hud, marker]: [u32; 3]) -> (String, String) {
    let eval = |line: &str| game.script_manager().eval(line).expect("evaluates");
    let rect = eval(&format!(
        "local r = UI.GetRect({marker}) \
         return string.format('%.1f %.1f', r.x + r.width / 2, r.y + r.height / 2)"
    ));
    let point = eval(&format!(
        "local x, y = Camera.WorldToScreen(1, 0, -10, {hud}) \
         return string.format('%.1f %.1f', x, y)"
    ));
    (rect, point)
}

fn agree(game: &GameWorld, ids: [u32; 3], what: &str) -> String {
    let (rect, point) = read(game, ids);
    assert_eq!(rect, point, "{what}: the marker and the projection agree");
    rect
}

#[test]
fn a_camera_moved_by_transform_moves_world_ui_in_the_same_frame() {
    let (game, ids) = session();
    let before = agree(&game, ids, "authored");
    let eval = |line: &str| game.script_manager().eval(line).expect("evaluates");
    eval(&format!("Transform.SetPosition({}, 0.5, 0, 0)", ids[0]));
    let moved = agree(&game, ids, "moved by Transform");
    assert_ne!(before, moved, "the marker follows the camera at once");
    let at = game.main_view().position;
    assert!(at.abs_diff_eq(Vec3::new(0.5, 0.0, 0.0), 1e-4), "{at:?}");
}

#[test]
fn set_fov_sets_the_camera_entity_lens() {
    let (game, ids) = session();
    let before = agree(&game, ids, "authored");
    let eval = |line: &str| game.script_manager().eval(line).expect("evaluates");
    eval("Camera.SetFov(30)");
    assert_eq!(eval("return Camera.GetFov()"), "30");
    let lens = format!("return Camera.GetFieldOfView({})", ids[0]);
    assert_eq!(eval(&lens), "30", "the entity's own field of view");
    let zoomed = agree(&game, ids, "zoomed");
    assert_ne!(before, zoomed, "a narrower lens spreads the marker out");
}

#[test]
fn edit_mode_readers_see_the_camera_entity_lens() {
    // Outside Play the per-frame sync leaves the resource's lens at the default;
    // the main view, and the UI built on it, still take the entity's.
    let mut s = Scene::new();
    let cam = s.add_entity("MainCamera".to_string());
    s.world.set_camera(cam, Some(default_camera()));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let scene = Rc::new(RefCell::new(s));
    let mut game = crate::play::world(Rc::clone(&scene), 10.0, console);
    crate::play::run(&mut game, 1);
    let mut resource = game.camera().borrow().clone();
    set_view_fov(&mut resource, &mut scene.borrow_mut(), 30.0);
    crate::play::run(&mut game, 1);
    assert_eq!(game.main_view().fov, 30.0);
    let view = game.resources.ui_view(&scene.borrow());
    assert_eq!(view.camera.expect("a camera").fov, 30.0);
}
