//! The one transport button (#1914): Play outside Play, Stop in its place during
//! it, the shortcut toggling both ways; and the console choice kept across Play.

use egui::{Event, Key, Modifiers, Pos2};
use egui_phosphor::regular as icon;

use crate::editor::EditorUi;
use crate::scene::Scene;

pub(super) struct Rig {
    ctx: egui::Context,
    editor: EditorUi,
    pub(super) scene: Scene,
    pub(super) playing: bool,
    /// The buttons last drawn: label and centre.
    buttons: Vec<(String, Pos2)>,
}

impl Rig {
    pub(super) fn new() -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let (editor, scene) = (EditorUi::new(Default::default()), Scene::new());
        let mut rig = Self {
            ctx,
            editor,
            scene,
            playing: false,
            buttons: Vec::new(),
        };
        rig.frame(vec![]);
        rig
    }

    /// One frame of the menu bar's tail with `events`, as the header lays it out.
    pub(super) fn frame(&mut self, events: Vec<Event>) {
        let (editor, scene, playing) = (&mut self.editor, &mut self.scene, &mut self.playing);
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        let mut output = self.ctx.run_ui(input, |ui| {
            let tail = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(tail, |ui| super::draw(editor, ui, scene, playing));
        });
        output.textures_delta.clear(); // no painter here to take the font atlas
        let tree = output
            .platform_output
            .accesskit_update
            .expect("accesskit on");
        self.buttons = (tree.nodes.iter())
            .filter(|(_, n)| n.role() == egui::accesskit::Role::Button)
            .filter_map(|(_, n)| {
                let b = n.bounds()?;
                let centre = Pos2::new((b.x0 + b.x1) as f32 / 2.0, (b.y0 + b.y1) as f32 / 2.0);
                Some((n.label()?.to_string(), centre))
            })
            .collect();
    }

    /// The transport glyphs drawn, Play and Stop only.
    fn transport(&self) -> Vec<&str> {
        let ours = [icon::PLAY, icon::STOP];
        (self.buttons.iter())
            .map(|(label, _)| label.as_str())
            .filter(|label| ours.contains(label))
            .collect()
    }

    pub(super) fn click(&mut self, glyph: &str) {
        let (_, at) = *self
            .buttons
            .iter()
            .find(|(l, _)| l == glyph)
            .expect("drawn");
        self.frame(vec![Event::PointerMoved(at)]);
        let press = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![press(true), press(false)]);
        self.frame(vec![]);
    }

    pub(super) fn shortcut(&mut self) {
        self.frame(vec![Event::Key {
            key: Key::P,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        }]);
        self.frame(vec![]);
    }
}

#[test]
fn one_button_plays_and_stops_in_place() {
    let mut rig = Rig::new();
    assert_eq!(rig.transport(), [icon::PLAY], "outside Play: Play only");
    let play_at = rig.buttons.iter().find(|(l, _)| l == icon::PLAY).unwrap().1;
    rig.click(icon::PLAY);
    assert!(rig.playing);
    assert_eq!(rig.transport(), [icon::STOP], "during Play: Stop only");
    let stop_at = rig.buttons.iter().find(|(l, _)| l == icon::STOP).unwrap().1;
    assert_eq!(play_at, stop_at, "Stop sits where Play was");

    rig.editor.selected_entity_id = Some(1);
    rig.click(icon::STOP);
    assert!(!rig.playing);
    assert_eq!(rig.transport(), [icon::PLAY]);
    assert_eq!(
        rig.editor.selected_entity_id, None,
        "Stop drops the selection"
    );
}

#[test]
fn the_shortcut_toggles_both_ways() {
    let mut rig = Rig::new();
    rig.shortcut();
    assert!(rig.playing);
    assert_eq!(rig.transport(), [icon::STOP]);
    rig.shortcut();
    assert!(!rig.playing);
    assert_eq!(rig.transport(), [icon::PLAY]);
}

/// Off when the editor starts, then kept in the editor's memory across Play and
/// Stop: nothing of it is saved (no `Storage` reaches the header at all).
#[cfg(feature = "dev")]
#[test]
fn the_console_choice_survives_play_and_stop() {
    let mut rig = Rig::new();
    assert!(
        !rig.editor.dev.show_play_console,
        "off when the editor starts"
    );
    rig.click(icon::TERMINAL_WINDOW);
    assert!(rig.editor.dev.show_play_console);
    for _ in 0..3 {
        rig.click(rig.transport()[0].to_string().as_str());
        assert!(rig.editor.dev.show_play_console);
    }
}
