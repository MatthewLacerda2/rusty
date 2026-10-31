//! Which focus takes Ctrl+Z from the scene (#1631): only a text field. The
//! viewport and a slider keep focus after their gesture, and the console after
//! Enter; none of them may leave Undo dead or a step open.

use super::{repl_input_id, shortcuts, typing_holds_step};
use crate::editor::EditorUi;
use crate::scene::authoring::create_entity;
use crate::scene::Scene;

/// What widget has the keyboard when Ctrl+Z is pressed.
#[derive(Clone, Copy)]
enum Focus {
    /// A plain widget (the viewport, a slider) kept focus after its gesture.
    Widget,
    /// A text field in the Inspector.
    TextField,
    /// The console's line, after Enter.
    Console,
}

/// One pass drawing a button and a text field (the console's id when `Focus::
/// Console`), focusing the one `focus` names.
fn pass(ctx: &egui::Context, focus: Focus, input: egui::RawInput, then: impl FnOnce()) {
    ctx.begin_pass(input);
    egui::Area::new(egui::Id::new("undo_focus")).show(ctx, |ui| {
        let button = ui.button("slider");
        let mut text = String::new();
        let mut field = egui::TextEdit::singleline(&mut text);
        if let Focus::Console = focus {
            field = field.id(repl_input_id());
        }
        let field = ui.add(field);
        match focus {
            Focus::Widget => button.request_focus(),
            Focus::TextField | Focus::Console => field.request_focus(),
        }
    });
    then();
    ctx.end_pass().textures_delta.clear();
}

/// Press Ctrl+Z with `focus` held; whether the scene's step was undone, and
/// whether the focus would hold an editor step open.
fn press_undo(focus: Focus) -> (bool, bool) {
    let mut scene = Scene::new();
    scene.begin_edits();
    create_entity(&mut scene, "Crate", None);
    scene.commit_edits(None);
    let mut editor = EditorUi::new(Default::default());
    let ctx = egui::Context::default();
    pass(&ctx, focus, Default::default(), || {});
    let mut input = egui::RawInput::default();
    input.events.push(egui::Event::Key {
        key: egui::Key::Z,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    });
    let mut holds = false;
    pass(&ctx, focus, input, || {
        holds = typing_holds_step(&ctx);
        shortcuts(&mut editor, &ctx, &mut scene);
    });
    (scene.undo_name().is_none(), holds)
}

#[test]
fn a_widget_keeping_focus_after_its_gesture_neither_blocks_undo_nor_holds_a_step() {
    assert_eq!(press_undo(Focus::Widget), (true, false));
}

#[test]
fn a_text_field_takes_ctrl_z_and_holds_the_step_while_typed_into() {
    assert_eq!(press_undo(Focus::TextField), (false, true));
}

#[test]
fn the_console_edits_its_own_text_but_holds_no_step_open() {
    assert_eq!(press_undo(Focus::Console), (false, false));
}
