//! World-space UI through the Lua surface in a real Play session (#429):
//! `Camera.WorldToScreen` / `ScreenToWorldRay`, a marker pinned with
//! `RectTransform.SetWorldAnchor`, and a `WorldSpace` terminal clicked through the
//! screen centre while the cursor is locked.

use std::cell::RefCell;
use std::rc::Rc;

use glam::{Vec2, Vec3};
use rusty::app::GameWorld;
use rusty::components::{
    CanvasComponent, CanvasRenderMode, ImageComponent, RectTransformComponent,
};
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::{Scene, ScriptComponent};
use rusty::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;

const TERMINAL: &str = r#"
return { OnPointerClick = function(id, e) print("[ui] terminal " .. e.button) end }
"#;

/// A 1280×720 session: a HUD with a marker, an enemy 10 m ahead, and a 4×2 m
/// terminal canvas 5 m ahead whose screen carries the script above. The camera
/// sits at the origin looking down -Z. Returns the world, console and ids.
fn session() -> (GameWorld, Rc<RefCell<ConsoleLogs>>, [u32; 4]) {
    let script = std::env::temp_dir().join("rusty_429_terminal.lua");
    std::fs::write(&script, TERMINAL).expect("write script");
    let mut s = Scene::new();
    let hud = s.add_entity("Hud".to_string());
    s.world.set_canvas(hud, Some(CanvasComponent::default()));
    let marker = s.add_entity("Marker".to_string());
    s.world
        .set_rect_transform(marker, Some(RectTransformComponent::default()));
    s.set_parent(marker, Some(hud)).expect("parent");
    let enemy = s.add_entity("Enemy".to_string());
    s.world.transform_mut(enemy).expect("t").position = Vec3::new(0.0, 0.0, -10.0);
    let terminal = s.add_entity("Terminal".to_string());
    let canvas = CanvasComponent {
        render_mode: CanvasRenderMode::WorldSpace,
        reference_resolution: Vec2::new(400.0, 200.0),
        ..Default::default()
    };
    s.world.set_canvas(terminal, Some(canvas));
    s.world.transform_mut(terminal).expect("t").position = Vec3::new(0.0, 0.0, -5.0);
    let screen = s.add_entity("Screen".to_string());
    let rt = RectTransformComponent {
        size_delta: Vec2::new(200.0, 100.0),
        ..Default::default()
    };
    s.world.set_rect_transform(screen, Some(rt));
    s.world.set_image(screen, Some(ImageComponent::default()));
    *s.world.scripts_mut(screen).expect("scripts") = vec![ScriptComponent {
        path: script.to_string_lossy().replace('\\', "/"),
        ..Default::default()
    }];
    s.set_parent(screen, Some(terminal)).expect("parent");
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let nav = NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(s)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::clone(&console),
    );
    game.resources.screen.borrow_mut().set_game_view(1280, 720);
    game.set_playing(true);
    game.tick(DT);
    eval(
        &game,
        "Camera.SetPosition(0, 0, 0) Camera.SetYaw(-90) Camera.SetPitch(0)",
    );
    game.tick(DT);
    (game, console, [hud, marker, enemy, screen])
}

fn eval(game: &GameWorld, line: &str) -> String {
    game.script_manager().eval(line).expect("evaluates")
}

#[test]
fn camera_projects_world_points_in_pixels_and_canvas_units() {
    let (game, _, [hud, ..]) = session();
    let fmt = "string.format('%.1f %.1f %.1f %s', Camera.WorldToScreen(0, 0, -5{}))";
    assert_eq!(
        eval(&game, &format!("return {}", fmt.replace("{}", ""))),
        "640.0 360.0 5.0 true"
    );
    let canvas = format!("return {}", fmt.replace("{}", &format!(", {hud}")));
    assert_eq!(
        eval(&game, &canvas),
        "960.0 540.0 5.0 true",
        "reference units"
    );
    let behind = "local x, y, d, on = Camera.WorldToScreen(0, 0, 5) return d < 0, on";
    assert_eq!(eval(&game, behind), "true, false");
    let ray = "local _, _, _, x, y, z = Camera.ScreenToWorldRay(640, 360) \
               return math.abs(x) < 1e-4 and math.abs(y) < 1e-4 and math.abs(z + 1) < 1e-4";
    assert_eq!(
        eval(&game, ray),
        "true",
        "the centre ray is the view direction"
    );
}

#[test]
fn a_marker_follows_its_target_and_a_terminal_takes_a_locked_click() {
    let (mut game, console, [_, marker, enemy, screen]) = session();
    eval(
        &game,
        &format!("RectTransform.SetWorldAnchor({marker}, {enemy}, 0, 0, 0)"),
    );
    let anchor = format!("return RectTransform.GetWorldAnchor({marker}).target");
    assert_eq!(eval(&game, &anchor), enemy.to_string());
    let centre = format!(
        "local r = UI.GetRect({marker}) return string.format('%.1f %.1f', r.x + r.width / 2, r.y + r.height / 2)"
    );
    assert_eq!(
        eval(&game, &centre),
        "960.0 540.0",
        "on the enemy, screen centre"
    );
    eval(&game, "Camera.SetYaw(90)");
    assert_eq!(
        eval(&game, &format!("return UI.GetRect({marker})")),
        "nil",
        "behind: hidden"
    );
    eval(&game, "Camera.SetYaw(-90)");
    // The cursor starts locked: the click goes through the screen centre.
    assert_eq!(eval(&game, &format!("return UI.Click({screen})")), "true");
    game.tick(DT);
    let log = console
        .borrow()
        .messages
        .iter()
        .any(|m| m.0 == "[ui] terminal Left");
    assert!(log, "the terminal took the click");
    let world = format!("return UI.GetRect({screen}).world");
    assert_eq!(eval(&game, &world), "true");
}
