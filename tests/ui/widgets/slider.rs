//! Slider and Scrollbar: pointer, drag and the arrows through `OnMove`.

use rusty::scene::authoring::ui_widgets::UiWidget;

use super::{part, widget, Ui};

fn value(ui: &Ui, id: u32, script: &str) -> f32 {
    let v = ui.eval(&format!("Scene.GetScript({id}, '{script}').value"));
    v.parse().expect("number")
}

#[test]
fn a_slider_follows_the_pointer_and_places_fill_and_handle() {
    let (mut sl, mut area, mut handle) = (0, 0, 0);
    let mut ui = Ui::new(|s| {
        sl = widget(s, UiWidget::Slider);
        area = part(s, sl, "Handle Slide Area");
        handle = part(s, sl, "Handle Slide Area/Handle");
    });
    let (x, y, w, h) = ui.rect(area);
    ui.pointer(x + w * 0.75, y + h * 0.5);
    ui.eval("Input.Press('Mouse0')");
    ui.tick(1);
    assert!(
        (value(&ui, sl, "slider") - 0.75).abs() < 0.01,
        "{:?}",
        ui.errors()
    );
    ui.pointer(x + w * 0.25, y + h * 0.5);
    ui.tick(1);
    ui.eval("Input.Release('Mouse0')");
    ui.tick(1);
    assert!(
        (value(&ui, sl, "slider") - 0.25).abs() < 0.01,
        "dragged back"
    );
    let (hx, _, hw, _) = ui.rect(handle);
    assert!(
        (hx + hw / 2.0 - (x + w * 0.25)).abs() < 1.0,
        "the handle sits on the value"
    );
}

#[test]
fn arrows_step_a_focused_slider_and_navigate_across_it() {
    let (mut sl, mut below) = (0, 0);
    let mut ui = Ui::new(|s| {
        sl = widget(s, UiWidget::Slider);
        below = widget(s, UiWidget::Button);
        let mut rt = s.world.rect_transform_mut(below).expect("rect");
        rt.anchored_position.y = -200.0;
    });
    ui.eval(&format!(
        "local s = Scene.GetScript({sl}, 'slider') s.max = 10 s.whole_numbers = true"
    ));
    ui.eval(&format!("UI.SetSelected({sl})"));
    ui.tick(1);
    ui.key("Right");
    ui.key("Right");
    assert_eq!(value(&ui, sl, "slider"), 2.0);
    assert_eq!(
        ui.eval("UI.GetSelected()"),
        sl.to_string(),
        "Left/Right keep the focus"
    );
    ui.key("Left");
    assert_eq!(value(&ui, sl, "slider"), 1.0);
    ui.key("Down");
    assert_eq!(
        ui.eval("UI.GetSelected()"),
        below.to_string(),
        "Down navigates"
    );
}

#[test]
fn a_scrollbar_drags_its_handle_and_pages_beside_it() {
    let (mut bar, mut area) = (0, 0);
    let mut ui = Ui::new(|s| {
        bar = widget(s, UiWidget::Scrollbar);
        area = part(s, bar, "Sliding Area");
    });
    let (x, y, w, h) = ui.rect(area);
    let mid = y + h / 2.0;
    // size 0.2 at value 0: the handle spans the first fifth.
    ui.drag((x + w * 0.1, mid), (x + w * 0.5, mid), 4);
    assert!(
        (value(&ui, bar, "scrollbar") - 0.5).abs() < 0.02,
        "{:?}",
        ui.errors()
    );
    ui.pointer(x + w * 0.02, mid);
    ui.eval("Input.Press('Mouse0') Input.Release('Mouse0')");
    ui.tick(1);
    assert!(
        (value(&ui, bar, "scrollbar") - 0.25).abs() < 0.02,
        "paged one size left"
    );
}
