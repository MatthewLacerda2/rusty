//! Input Field: typing, editing keys, selection, validation and submit / cancel.

use rusty::scene::authoring::ui_widgets::UiWidget;

use super::{part, widget, Ui};

fn field() -> (Ui, u32, u32) {
    let (mut f, mut text) = (0, 0);
    let ui = Ui::new(|s| {
        f = widget(s, UiWidget::InputField);
        text = part(s, f, "Text Area/Text");
    });
    let script = format!("Scene.GetScript({f}, 'input_field')");
    ui.eval(&format!(
        "{script}.on_submit = function(id, t) print('[w] submit ' .. t) end \
         {script}.on_end_edit = function(id, t) print('[w] end ' .. t) end"
    ));
    (ui, f, text)
}

fn value(ui: &Ui, f: u32) -> String {
    ui.eval(&format!("Scene.GetScript({f}, 'input_field').text"))
}

fn type_text(ui: &mut Ui, s: &str) {
    ui.eval(&format!("Input.TypeText({s:?})"));
    ui.tick(1);
}

#[test]
fn typing_editing_and_submitting() {
    let (mut ui, f, text) = field();
    ui.click(f);
    type_text(&mut ui, "helo");
    ui.key("Left");
    type_text(&mut ui, "l");
    assert_eq!(value(&ui, f), "hello", "{:?}", ui.errors());
    ui.key("End");
    type_text(&mut ui, "!\u{8}\u{8}");
    assert_eq!(value(&ui, f), "hell");
    ui.key("Home");
    ui.key("Delete");
    assert_eq!(value(&ui, f), "ell");
    assert_eq!(ui.eval(&format!("Text.GetText({text})")), "ell");
    ui.key("Enter");
    assert_eq!(ui.log(), vec!["[w] submit ell"]);
}

#[test]
fn focus_selects_all_and_shift_extends_a_selection() {
    let (mut ui, f, _) = field();
    ui.eval(&format!(
        "Scene.GetScript({f}, 'input_field').set_text('abc')"
    ));
    ui.eval(&format!("UI.SetSelected({f})"));
    ui.tick(1);
    type_text(&mut ui, "x");
    assert_eq!(value(&ui, f), "x", "typing replaced the selected whole");
    type_text(&mut ui, "yz");
    ui.eval("Input.Press('LeftShift')");
    ui.key("Left");
    ui.key("Left");
    ui.eval("Input.Release('LeftShift')");
    type_text(&mut ui, "Q");
    assert_eq!(value(&ui, f), "xQ");
}

#[test]
fn content_types_and_char_limit_filter_input() {
    let (mut ui, f, text) = field();
    let script = format!("Scene.GetScript({f}, 'input_field')");
    ui.eval(&format!(
        "{script}.content_type = 'Integer' {script}.char_limit = 4"
    ));
    ui.click(f);
    type_text(&mut ui, "-1a2-.345");
    assert_eq!(value(&ui, f), "-123");
    ui.eval(&format!(
        "{script}.content_type = 'Password' {script}.set_text('')"
    ));
    type_text(&mut ui, "pw");
    assert_eq!(value(&ui, f), "pw");
    assert_eq!(ui.eval(&format!("Text.GetText({text})")), "**");
}

#[test]
fn escape_restores_the_text_and_lets_go() {
    let (mut ui, f, _) = field();
    ui.eval(&format!(
        "Scene.GetScript({f}, 'input_field').set_text('keep')"
    ));
    ui.click(f);
    type_text(&mut ui, "zz");
    ui.key("Escape");
    assert_eq!(value(&ui, f), "keep");
    assert_eq!(ui.eval("UI.GetSelected()"), "nil");
    assert_eq!(ui.log(), vec!["[w] end keep"]);
}
