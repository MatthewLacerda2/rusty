//! The widget kit (#422) end to end: widgets built by `create_ui` (the Create ▸ UI
//! path), run as their engine Lua scripts in a real Play session, and driven only
//! through the player's input path — `UI.Click`, injected keys, mouse and text.

mod button_toggle;
mod create;
mod dropdown;
mod input_field;
mod reads;
mod scroll;
mod slider;
mod world;

use std::cell::RefCell;
use std::rc::Rc;

use rusty::app::GameWorld;
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::authoring::ui_widgets::{self, create_ui, UiWidget};
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;
const SCREEN_H: f32 = 1080.0;

/// A 1920×1080 Play session over a scene `build` populated.
pub struct Ui {
    pub game: GameWorld,
    console: Rc<RefCell<ConsoleLogs>>,
}

impl Ui {
    /// Seed the widget scripts, build the scene, enter Play with a free cursor, and
    /// run one tick so every widget has woken.
    pub fn new(build: impl FnOnce(&mut Scene)) -> Self {
        ui_widgets::seed();
        let mut scene = Scene::new();
        build(&mut scene);
        let console = Rc::new(RefCell::new(ConsoleLogs::new()));
        let nav = NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
        let mut game = GameWorld::new(
            Rc::new(RefCell::new(scene)),
            Rc::new(RefCell::new(InputState::new())),
            Rc::new(RefCell::new(nav)),
            Rc::clone(&console),
        );
        game.resources.screen.borrow_mut().set_game_view(1920, 1080);
        game.set_playing(true);
        let mut ui = Self { game, console };
        ui.tick(1);
        ui.eval("Input.SetCursorLocked(false)");
        ui
    }

    pub fn eval(&self, line: &str) -> String {
        self.game
            .script_manager()
            .eval(line)
            .unwrap_or_else(|e| panic!("`{line}` failed: {e}"))
    }

    pub fn tick(&mut self, n: usize) {
        for _ in 0..n {
            self.game.tick(DT);
        }
    }

    /// `UI.Click(id)`, then a tick for it to land.
    pub fn click(&mut self, id: u32) {
        assert_eq!(
            self.eval(&format!("UI.Click({id})")),
            "true",
            "click lands on {id}"
        );
        self.tick(1);
    }

    /// Press a key for one tick, then release it.
    pub fn key(&mut self, key: &str) {
        self.eval(&format!("Input.Press('{key}')"));
        self.tick(1);
        self.eval(&format!("Input.Release('{key}')"));
        self.tick(1);
    }

    /// Move the pointer to UI screen pixels `(x, y)` (bottom-left, y-up).
    pub fn pointer(&mut self, x: f32, y: f32) {
        self.eval(&format!("Input.MoveMouse({x}, {})", SCREEN_H - y));
    }

    /// Drag with the left button from `from` to `to` in `steps` ticks.
    pub fn drag(&mut self, from: (f32, f32), to: (f32, f32), steps: usize) {
        self.pointer(from.0, from.1);
        self.tick(1);
        self.eval("Input.Press('Mouse0')");
        self.tick(1);
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            self.pointer(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
            self.tick(1);
        }
        self.eval("Input.Release('Mouse0')");
        self.tick(1);
    }

    /// `id`'s screen rect as `(x, y, width, height)`.
    pub fn rect(&self, id: u32) -> (f32, f32, f32, f32) {
        let s = self.eval(&format!(
            "local s = UI.GetRect({id}).screen return s.x, s.y, s.width, s.height"
        ));
        let v: Vec<f32> = s.split(", ").map(|n| n.parse().expect("number")).collect();
        (v[0], v[1], v[2], v[3])
    }

    /// Lines the widgets' owner scripts printed, prefixed `[w]`.
    pub fn log(&self) -> Vec<String> {
        let c = self.console.borrow();
        let lines = c.messages.iter().map(|m| m.0.clone());
        lines.filter(|m| m.starts_with("[w]")).collect()
    }

    /// Console errors, for a failing assertion's message.
    pub fn errors(&self) -> Vec<String> {
        let c = self.console.borrow();
        let lines = c.messages.iter().map(|m| m.0.clone());
        lines.filter(|m| m.contains("Error")).collect()
    }
}

/// Create `kind` under the scene's canvas (made on first use).
pub fn widget(scene: &mut Scene, kind: UiWidget) -> u32 {
    create_ui(scene, kind, None)
}

/// The child of `id` at name path `path`.
pub fn part(scene: &Scene, id: u32, path: &str) -> u32 {
    let mut at = id;
    for seg in path.split('/') {
        let kids = scene.world.children(at);
        at = kids
            .into_iter()
            .find(|&c| scene.world.name(c).is_some_and(|n| *n == seg))
            .unwrap_or_else(|| panic!("no {path} under {id}"));
    }
    at
}
