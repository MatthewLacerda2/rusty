//! Input Field clipboard (#612): Ctrl/Cmd + C, X, V through `Input.SetClipboard`,
//! the same call a bot uses to stage a paste.

use rusty::scene::authoring::ui_widgets::UiWidget;

use super::{widget, Ui};

fn focused_field(text: &str) -> (Ui, String) {
    let mut f = 0;
    let mut ui = Ui::new(|s| f = widget(s, UiWidget::InputField));
    let script = format!("Scene.GetScript({f}, 'input_field')");
    ui.eval(&format!("{script}.set_text({text:?})"));
    ui.eval(&format!("UI.SetSelected({f})")); // focus selects everything
    ui.tick(1);
    (ui, script)
}

fn chord(ui: &mut Ui, modifier: &str, key: &str) {
    ui.eval(&format!("Input.Press('{modifier}')"));
    ui.key(key);
    ui.eval(&format!("Input.Release('{modifier}')"));
    ui.tick(1);
}

fn value(ui: &Ui, script: &str) -> String {
    ui.eval(&format!("{script}.text"))
}

#[test]
fn copy_and_cut_put_the_selection_on_the_clipboard() {
    let (mut ui, script) = focused_field("hello");
    chord(&mut ui, "LeftControl", "C");
    assert_eq!(
        ui.eval("Input.GetClipboard()"),
        "hello",
        "{:?}",
        ui.errors()
    );
    assert_eq!(value(&ui, &script), "hello", "copy leaves the text");
    chord(&mut ui, "LeftControl", "A");
    chord(&mut ui, "LeftSuper", "X");
    assert_eq!(ui.eval("Input.GetClipboard()"), "hello");
    assert_eq!(value(&ui, &script), "", "Cmd+X cuts");
    chord(&mut ui, "LeftControl", "V");
    assert_eq!(value(&ui, &script), "hello", "and pastes back");
}

#[test]
fn paste_goes_through_the_fields_filters() {
    let (mut ui, script) = focused_field("");
    ui.eval(&format!(
        "{script}.content_type = 'Integer' {script}.char_limit = 4"
    ));
    ui.eval("Input.SetClipboard('-1a2\\n345')");
    chord(&mut ui, "LeftControl", "V");
    assert_eq!(value(&ui, &script), "-123");

    let (mut ui, script) = focused_field("x");
    ui.eval("Input.SetClipboard('10.0\\r\\n.0.7')");
    chord(&mut ui, "LeftControl", "V");
    assert_eq!(
        value(&ui, &script),
        "10.0.0.7",
        "SingleLine drops line breaks"
    );
}

#[test]
fn a_password_field_never_copies() {
    let (mut ui, script) = focused_field("secret");
    ui.eval(&format!("{script}.content_type = 'Password'"));
    ui.eval("Input.SetClipboard('before')");
    chord(&mut ui, "LeftControl", "C");
    chord(&mut ui, "LeftControl", "X");
    assert_eq!(ui.eval("Input.GetClipboard()"), "before");
    assert_eq!(value(&ui, &script), "secret");
}
