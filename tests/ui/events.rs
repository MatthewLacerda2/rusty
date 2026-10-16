//! End-to-end UI interaction (#420) through a real Play session: `UI.Click` drives
//! the real input path, a Lua button receives enter → select → click in order and
//! submits on Enter, gameplay sees the pointer consumed, `UI.List` and
//! `Selectable.GetState` report the state, the ColorTint reaches the Image, and two
//! identical runs log identically.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec2;
use rusty::app::GameWorld;
use rusty::components::{
    CanvasComponent, ImageComponent, RectTransformComponent, SelectableComponent,
};
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::{Scene, ScriptComponent};
use rusty::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;

const BUTTON: &str = r#"
local B = {}
function B.OnPointerEnter(id, e) print("[ui] enter") end
function B.OnSelect(id) print("[ui] select") end
function B.OnPointerClick(id, e)
    print(string.format("[ui] click %s %d at %.0f,%.0f", e.button, e.target, e.position.x, e.position.y))
end
function B.OnSubmit(id) print("[ui] submit") end
function B.Update(id, dt) if UI.IsPointerConsumed() then print("[ui] consumed") end end
return B
"#;

/// A 1920×1080 Play session with a 200×80 button centred on screen, carrying the
/// script above and a child label. Returns the world, its console and the ids.
fn session(tag: &str) -> (GameWorld, Rc<RefCell<ConsoleLogs>>, u32, u32) {
    let script = crate::temp::dir().join(format!("rusty_420_button_{tag}.lua"));
    std::fs::write(&script, BUTTON).expect("write script");
    let mut s = Scene::new();
    let canvas = s.add_entity("Canvas".to_string());
    s.world.set_canvas(canvas, Some(CanvasComponent::default()));
    let button = s.add_entity("Start Game".to_string());
    let rt = RectTransformComponent {
        size_delta: Vec2::new(200.0, 80.0),
        ..RectTransformComponent::default()
    };
    s.world.set_rect_transform(button, Some(rt));
    s.world.set_image(button, Some(ImageComponent::default()));
    s.world
        .set_selectable(button, Some(SelectableComponent::default()));
    *s.world.scripts_mut(button).expect("scripts") = vec![ScriptComponent {
        path: script.to_string_lossy().replace('\\', "/"),
        ..Default::default()
    }];
    s.set_parent(button, Some(canvas)).expect("parent");
    let label = s.add_entity("Label".to_string());
    s.world
        .set_rect_transform(label, Some(RectTransformComponent::default()));
    s.world.set_image(label, Some(ImageComponent::default()));
    s.set_parent(label, Some(button)).expect("parent");
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let nav = NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(s)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::clone(&console),
    );
    game.resources.screen.borrow_mut().set_game_view(1920, 1080);
    game.set_playing(true);
    game.tick(DT);
    (game, console, button, label)
}

fn eval(game: &GameWorld, line: &str) -> String {
    game.script_manager().eval(line).expect("evaluates")
}

fn ui_log(console: &Rc<RefCell<ConsoleLogs>>) -> Vec<String> {
    let c = console.borrow();
    let lines = c.messages.iter().map(|m| m.0.clone());
    lines.filter(|m| m.starts_with("[ui]")).collect()
}

/// Click, then submit; returns the UI log.
fn click_and_submit(tag: &str) -> Vec<String> {
    let (mut game, console, button, label) = session(tag);
    assert_eq!(
        eval(&game, &format!("UI.Click({button})")),
        "false",
        "cursor locked"
    );
    game.tick(DT);
    assert!(
        ui_log(&console).is_empty(),
        "a locked cursor never reaches the UI"
    );
    eval(&game, "Input.SetCursorLocked(false)");
    assert_eq!(eval(&game, "UI.Raycast(960, 540)"), label.to_string());
    assert_eq!(eval(&game, &format!("UI.Click({button})")), "true");
    game.tick(DT);
    assert_eq!(eval(&game, "UI.GetSelected()"), button.to_string());
    let state = format!("Selectable.GetState({button})");
    assert_eq!(eval(&game, &state), "Selected");
    let listed = "local l = UI.List() return #l, l[1].name, l[1].state, l[1].rect.width";
    assert_eq!(eval(&game, listed), "1, Start Game, Selected, 200");
    let tint = game
        .scene()
        .borrow()
        .world
        .image(button)
        .expect("image")
        .state_tint;
    assert!(tint.x < 1.0, "the Selected tint reached the Image: {tint}");
    eval(&game, "Input.Press('Enter')");
    game.tick(DT);
    ui_log(&console)
}

#[test]
fn a_lua_button_is_clicked_focused_and_submitted_deterministically() {
    let log = click_and_submit("a");
    let expected = [
        "[ui] enter",
        "[ui] select",
        "[ui] click Left 3 at 960,540",
        "[ui] consumed",
        "[ui] submit",
        "[ui] consumed",
    ];
    assert_eq!(log, expected);
    assert_eq!(log, click_and_submit("b"), "same inputs, same callbacks");
}
