//! Scroll View: wheel, drag with elastic edges and inertia, and its scrollbars.

use rusty::components::LayoutElementComponent;
use rusty::scene::authoring::ui_widgets::{create_ui, UiWidget};

use super::{part, widget, Ui};

/// A scroll view over ten 60-tall buttons (600 of content in a 380-tall viewport).
fn list() -> (Ui, u32, u32, u32) {
    let (mut sv, mut content, mut vbar) = (0, 0, 0);
    let ui = Ui::new(|s| {
        sv = widget(s, UiWidget::ScrollView);
        content = part(s, sv, "Viewport/Content");
        vbar = part(s, sv, "Scrollbar Vertical");
        for _ in 0..10 {
            let b = create_ui(s, UiWidget::Button, Some(content));
            let e = LayoutElementComponent {
                preferred_height: Some(60.0),
                ..Default::default()
            };
            s.world.set_layout_element(b, Some(e));
        }
    });
    (ui, sv, content, vbar)
}

fn content_y(ui: &Ui, content: u32) -> f32 {
    let v = ui.eval(&format!(
        "select(2, RectTransform.GetAnchoredPosition({content}))"
    ));
    v.parse().expect("number")
}

fn bar_value(ui: &Ui, bar: u32) -> f32 {
    let v = ui.eval(&format!("Scene.GetScript({bar}, 'scrollbar').value"));
    v.parse().expect("number")
}

#[test]
fn the_wheel_scrolls_within_bounds_and_moves_the_bar() {
    let (mut ui, sv, content, vbar) = list();
    let (x, y, w, h) = ui.rect(sv);
    ui.pointer(x + w / 2.0, y + h / 2.0);
    ui.eval("Input.Scroll(-1)");
    ui.tick(1);
    assert_eq!(content_y(&ui, content), 40.0, "{:?}", ui.errors());
    assert!((bar_value(&ui, vbar) - (1.0 - 40.0 / 220.0)).abs() < 0.01);
    ui.eval("Input.Scroll(-20)");
    ui.tick(1);
    assert_eq!(content_y(&ui, content), 220.0, "clamped at the bottom");
    assert_eq!(bar_value(&ui, vbar), 0.0);
}

#[test]
fn a_drag_past_the_edge_rubber_bands_then_springs_back() {
    let (mut ui, sv, content, _) = list();
    let (x, y, w, h) = ui.rect(sv);
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    ui.drag((cx, cy), (cx, cy - 150.0), 5);
    let pulled = content_y(&ui, content);
    assert!(
        pulled < 0.0 && pulled > -150.0,
        "rubber-banded past the top: {pulled}"
    );
    ui.tick(60);
    assert!(
        content_y(&ui, content).abs() < 0.5,
        "sprung back to the top"
    );
}

#[test]
fn a_flick_coasts_after_release() {
    let (mut ui, sv, content, _) = list();
    let (x, y, w, h) = ui.rect(sv);
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    ui.drag((cx, cy - 60.0), (cx, cy + 60.0), 3);
    let released = content_y(&ui, content);
    ui.tick(10);
    assert!(content_y(&ui, content) > released, "inertia carries it on");
}

#[test]
fn the_scrollbar_drives_the_content() {
    let (ui, _, content, vbar) = list();
    ui.eval(&format!(
        "Scene.GetScript({vbar}, 'scrollbar').set_value(0)"
    ));
    assert_eq!(content_y(&ui, content), 220.0);
}
