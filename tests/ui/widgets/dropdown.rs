//! Dropdown: opens on a popup canvas, picks by click or keys, closes on Escape and
//! on a click outside.

use rusty::scene::authoring::ui_widgets::UiWidget;

use super::{part, widget, Ui};

fn dropdown() -> (Ui, u32, u32) {
    let (mut d, mut label) = (0, 0);
    let ui = Ui::new(|s| {
        d = widget(s, UiWidget::Dropdown);
        label = part(s, d, "Label");
    });
    ui.eval(&format!(
        "Scene.GetScript({d}, 'dropdown').on_value_changed = \
         function(id, i, text) print('[w] ' .. i .. ' ' .. text) end"
    ));
    (ui, d, label)
}

fn find(ui: &Ui, name: &str) -> u32 {
    // `0` when absent (`nil` renders as a parse failure).
    let id = ui.eval(&format!("Scene.FindEntityByName('{name}')"));
    id.parse().unwrap_or(0)
}

fn is_open(ui: &Ui, d: u32) -> bool {
    ui.eval(&format!("Scene.GetScript({d}, 'dropdown').is_expanded()")) == "true"
}

#[test]
fn clicking_an_item_picks_it_and_closes_the_list() {
    let (mut ui, d, label) = dropdown();
    ui.click(d);
    assert!(is_open(&ui, d), "{:?}", ui.errors());
    let popup = find(&ui, "Dropdown List");
    assert_eq!(ui.eval(&format!("Canvas.GetSortOrder({popup})")), "30000");
    let item = find(&ui, "Item 1: Option B");
    ui.click(item);
    assert!(!is_open(&ui, d));
    assert_eq!(ui.eval(&format!("Text.GetText({label})")), "Option B");
    assert_eq!(ui.log(), vec!["[w] 1 Option B"]);
    assert_eq!(find(&ui, "Dropdown List"), 0, "the popup is gone");
    assert_eq!(ui.eval("UI.GetSelected()"), d.to_string(), "focus returns");
}

#[test]
fn the_keyboard_opens_moves_and_picks() {
    let (mut ui, d, _) = dropdown();
    ui.eval(&format!("UI.SetSelected({d})"));
    ui.tick(1);
    ui.key("Enter");
    assert!(is_open(&ui, d));
    ui.key("Down");
    ui.key("Down");
    ui.key("Enter");
    assert!(!is_open(&ui, d));
    assert_eq!(ui.log(), vec!["[w] 2 Option C"]);
}

#[test]
fn escape_and_an_outside_click_close_without_picking() {
    let (mut ui, d, _) = dropdown();
    ui.click(d);
    ui.key("Escape");
    assert!(!is_open(&ui, d));
    ui.click(d);
    ui.tick(1);
    ui.pointer(5.0, 5.0);
    ui.eval("Input.Press('Mouse0') Input.Release('Mouse0')");
    ui.tick(1);
    assert!(!is_open(&ui, d));
    assert!(ui.log().is_empty());
    ui.eval(&format!(
        "Scene.GetScript({d}, 'dropdown').set_options({{ 'Low', 'High' }})"
    ));
    ui.click(d);
    ui.tick(1);
    assert_ne!(
        find(&ui, "Item 1: High"),
        0,
        "set_options rebuilds the list"
    );
}
