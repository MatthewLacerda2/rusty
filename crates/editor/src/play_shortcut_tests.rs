//! Ctrl+P (Cmd+P on macOS) toggles Play from wherever the keyboard is, as Unity's does:
//! through the whole editor frame, not the header alone, so no panel drawn before or
//! after it takes the key. During Play the Game view is showing and clicked into, as
//! when the game holds the cursor: the shell hands egui every window event whatever the
//! capture (`EditorFrontend::on_window_event`).

use egui::{Event, Key, Modifiers, PointerButton, Pos2};

use super::{EditorUi, ViewportTab};
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

struct Rig {
    ctx: egui::Context,
    editor: EditorUi,
    scene: Scene,
    console: ConsoleLogs,
    playing: bool,
}

impl Rig {
    fn new() -> Self {
        let mut rig = Rig {
            ctx: egui::Context::default(),
            editor: EditorUi::new(Default::default()),
            scene: Scene::new(),
            console: ConsoleLogs::new(),
            playing: false,
        };
        rig.frame(Vec::new());
        rig.frame(Vec::new());
        rig
    }

    fn frame(&mut self, events: Vec<Event>) {
        let size = egui::vec2(1600.0, 900.0);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        let mut nav = NavigationGraph::from_scene(&self.scene);
        self.ctx.begin_pass(input);
        let (scene, console) = (&mut self.scene, &mut self.console);
        self.editor
            .draw(&self.ctx, scene, console, &mut nav, &mut self.playing, None);
        self.ctx.end_pass().textures_delta.clear();
    }

    /// Ctrl+P (Cmd+P on macOS) pressed and released, as egui-winit hands it on.
    fn shortcut(&mut self) {
        let key = |pressed| Event::Key {
            key: Key::P,
            physical_key: Some(Key::P),
            pressed,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        };
        self.frame(vec![key(true)]);
        self.frame(vec![key(false)]);
    }

    /// A click in the middle of the window: the viewport, the Game view in Play.
    fn click_viewport(&mut self) {
        let at = Pos2::new(800.0, 400.0);
        let press = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(at), press(true)]);
        self.frame(vec![press(false)]);
    }
}

#[test]
fn ctrl_p_plays_from_a_focused_text_field_and_stops_from_the_game_view() {
    let mut rig = Rig::new();
    let field = egui::Id::new("a focused text field");
    rig.ctx.memory_mut(|m| m.request_focus(field));
    rig.shortcut();
    assert!(rig.playing, "Ctrl+P enters Play with a text field focused");

    rig.editor.viewport_tab = ViewportTab::Game;
    rig.click_viewport();
    assert!(rig.playing, "a click in the Game view stays in Play");
    rig.shortcut();
    assert!(!rig.playing, "Ctrl+P leaves Play from the Game view");
    rig.shortcut();
    assert!(rig.playing, "and enters it again");
}

/// The Play-mode Developer Console's command line focused takes no shortcut either.
#[cfg(feature = "dev")]
#[test]
fn ctrl_p_stops_play_with_the_play_console_typed_into() {
    let mut rig = Rig::new();
    rig.shortcut();
    assert!(rig.playing);
    rig.editor.dev.show_play_console = true;
    rig.frame(Vec::new());
    let repl = super::header::repl_input_id();
    rig.ctx.memory_mut(|m| m.request_focus(repl));
    rig.frame(vec![Event::Text("print".into())]);
    rig.shortcut();
    assert!(!rig.playing, "Ctrl+P leaves Play from the console's field");
}
