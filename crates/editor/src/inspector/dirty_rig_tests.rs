//! One scene and one Inspector, poked through real egui input.

use egui::{Event, Pos2, Rect, Vec2};

use crate::EditorUi;
use rusty_edit::authoring::{add_component, create_entity, ComponentKind, Primitive};
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

/// Id and place: an automatic id can repeat across cards.
pub(super) type Key = (u64, u32, u32);

fn key(w: &egui::WidgetRect) -> Key {
    (w.id.value(), w.rect.min.x.to_bits(), w.rect.min.y.to_bits())
}

pub(super) struct Rig {
    ctx: egui::Context,
    pub(super) editor: EditorUi,
    pub(super) scene: Scene,
    nav: NavigationGraph,
    pub(super) id: u32,
}

impl Rig {
    pub(super) fn new(subject: Subject) -> Self {
        let mut scene = Scene::new();
        let id = create_entity(&mut scene, "Subject", Some(Primitive::Box));
        let mut editor = EditorUi::new(Default::default());
        if let Subject::With(kind) = subject {
            add_component(&mut scene, id, kind);
        }
        if subject != Subject::Settings {
            editor.selected_entity_id = Some(id);
        }
        scene.clear_undo();
        let nav = NavigationGraph::from_scene(&scene);
        let ctx = egui::Context::default();
        let mut rig = Self {
            ctx,
            editor,
            scene,
            nav,
            id,
        };
        rig.reset();
        rig
    }

    /// Back to the scene as built, a fresh Inspector, unmarked.
    pub(super) fn reset(&mut self) {
        while self.scene.undo().is_some() {}
        self.scene.take_edited();
        self.ctx.memory_mut(|m| *m = Default::default());
        let selected = self.editor.selected_entity_id;
        self.editor = EditorUi::new(Default::default());
        self.editor.selected_entity_id = selected;
        for _ in 0..3 {
            self.frame(vec![]);
        }
        self.editor.is_dirty = false;
    }

    /// Recorded as the shell records it: a step lands on release.
    fn frame(&mut self, events: Vec<Event>) {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 4000.0));
        let input = egui::RawInput {
            events,
            screen_rect: Some(screen),
            // A click: under 0.8 s; a double-click: two presses under 0.3 s.
            time: Some(self.ctx.input(|i| i.time) + 0.1),
            ..Default::default()
        };
        let mut console = ConsoleLogs::new();
        self.scene.begin_edits();
        self.ctx.begin_pass(input);
        let (editor, scene) = (&mut self.editor, &mut self.scene);
        let ui = &mut crate::theme::chrome::root_ui(&self.ctx);
        super::super::draw(editor, ui, scene, &mut console, &mut self.nav);
        self.ctx.end_pass().textures_delta.clear();
        self.scene.pause_edits();
        if !self.ctx.input(|i| i.pointer.any_down()) {
            self.scene.commit_edits(None);
        }
        self.editor.follow_edits(&mut self.scene);
    }

    pub(super) fn targets(&self) -> Vec<(Key, Pos2, bool)> {
        self.ctx.viewport(|v| {
            let layers = v.this_pass.widgets.layers();
            let widgets = layers.flat_map(|(_, w)| w.iter());
            let live =
                widgets.filter(|w| w.enabled && (w.sense.senses_click() || w.sense.senses_drag()));
            live.map(|w| (key(w), w.rect.center(), w.sense.senses_drag()))
                .collect()
        })
    }

    /// Press, drag when `drags`, release; then press what that opened (a combo's list).
    pub(super) fn poke(&mut self, at: Pos2, drags: bool) {
        let press = |pos, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let to = if drags { at + Vec2::new(40.0, 0.0) } else { at };
        self.frame(vec![Event::PointerMoved(at), press(at, true)]);
        self.frame(vec![Event::PointerMoved(to)]);
        self.frame(vec![press(to, false)]);
        let before = self.targets().len();
        self.frame(vec![]);
        if let Some(&(_, entry, _)) = self.targets().get(before) {
            self.frame(vec![Event::PointerMoved(entry), press(entry, true)]);
            self.frame(vec![press(entry, false)]);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Subject {
    Settings,
    Box,
    With(ComponentKind),
}
