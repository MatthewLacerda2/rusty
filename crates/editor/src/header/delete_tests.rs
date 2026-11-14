//! Deleting by hand (#1915): Edit ▸ Delete and the Delete key each remove the
//! selection with its subtree and clear the selection, as one Undo step, and
//! the key does nothing while a text field has the keyboard.

use egui::{Event, Key, Modifiers, Pos2};

use super::{edit_menu, shortcuts};
use crate::EditorUi;
use rusty_scene::authoring::create_entity;
use rusty_scene::Scene;

/// An editor over a scene whose selected "Crate" holds a "Lid".
struct Rig {
    ctx: egui::Context,
    editor: EditorUi,
    scene: Scene,
    ids: [u32; 2],
}

impl Rig {
    fn new() -> Self {
        let mut scene = Scene::new();
        let ids = [create_entity(&mut scene, "Crate", None), 0];
        let ids = [ids[0], create_entity(&mut scene, "Lid", None)];
        scene.set_parent(ids[1], Some(ids[0])).expect("a child");
        let mut editor = EditorUi::new(Default::default());
        editor.selected_entity_id = Some(ids[0]);
        let ctx = egui::Context::default();
        Self {
            ctx,
            editor,
            scene,
            ids,
        }
    }

    /// One recorded frame: `events` in, the shortcuts and the Edit menu drawn
    /// (and a focused text field when `typing`), the frame's step committed.
    fn frame(&mut self, events: Vec<Event>, typing: bool) {
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        let (ctx, editor, scene) = (&self.ctx, &mut self.editor, &mut self.scene);
        scene.begin_edits();
        ctx.begin_pass(input);
        shortcuts(editor, ctx, scene);
        egui::Area::new(egui::Id::new("menu"))
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| edit_menu(editor, ui, scene));
                if typing {
                    let mut text = String::new();
                    ui.add(egui::TextEdit::singleline(&mut text))
                        .request_focus();
                }
            });
        ctx.end_pass().textures_delta.clear();
        scene.commit_edits(None);
    }

    /// Press and release the primary button at `pos`, a frame each.
    fn click(&mut self, pos: Pos2) {
        let button = |pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(pos), button(true)], false);
        self.frame(vec![button(false)], false);
    }

    /// The last clickable widget outside the menu bar's own layer: the open
    /// Edit menu's last item, Delete.
    fn delete_item(&self) -> Pos2 {
        let bar = egui::LayerId::new(egui::Order::Middle, egui::Id::new("menu"));
        self.ctx
            .viewport(|v| {
                let mut layers = v.this_pass.widgets.layers();
                let (_, widgets) = layers.find(|(l, _)| **l != bar)?;
                let item = widgets.iter().rev().find(|w| w.sense.senses_click())?;
                Some(item.rect.center())
            })
            .expect("the Edit menu is open")
    }

    /// The crate and its lid are gone, the selection cleared, and Undo brings
    /// both back.
    fn assert_deleted_then_undone(mut self) {
        let alive = |s: &Scene| self.ids.map(|id| s.world.contains(id));
        assert_eq!(alive(&self.scene), [false, false], "the subtree goes");
        assert_eq!(self.editor.selected_entity_id, None);
        assert!(self.editor.is_dirty);
        self.scene.undo();
        assert_eq!(alive(&self.scene), [true, true], "one Undo step");
    }
}

fn key(key: Key, modifiers: Modifiers) -> Vec<Event> {
    let event = |pressed| Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    };
    vec![event(true), event(false)]
}

#[test]
fn edit_delete_removes_the_selection_and_undo_brings_it_back() {
    let mut rig = Rig::new();
    rig.frame(vec![], false); // the Area's sizing pass
    rig.frame(vec![], false);
    rig.click(egui::pos2(12.0, 8.0));
    rig.frame(vec![], false); // the menu draws the frame after it opens
    let item = rig.delete_item();
    rig.click(item);
    rig.assert_deleted_then_undone();
}

#[test]
fn the_delete_key_removes_the_selection_unless_a_text_field_has_it() {
    let mut rig = Rig::new();
    rig.frame(vec![], true);
    rig.frame(key(Key::Delete, Modifiers::NONE), true);
    assert!(rig.scene.world.contains(rig.ids[0]), "typing keeps Delete");
    rig.ctx = egui::Context::default(); // the field loses the keyboard
    rig.frame(key(Key::Delete, Modifiers::NONE), false);
    rig.assert_deleted_then_undone();
}

#[test]
fn cmd_backspace_deletes_on_macos_and_ctrl_backspace_does_not() {
    let mut rig = Rig::new();
    let ctrl = Modifiers::CTRL.plus(Modifiers::COMMAND);
    rig.frame(key(Key::Backspace, ctrl), false);
    assert!(
        rig.scene.world.contains(rig.ids[0]),
        "Ctrl+Backspace off a Mac"
    );
    rig.frame(
        key(Key::Backspace, Modifiers::MAC_CMD.plus(Modifiers::COMMAND)),
        false,
    );
    rig.assert_deleted_then_undone();
}
