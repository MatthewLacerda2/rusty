//! The API the widgets stand on (#422): hierarchy reads, `SetActive`,
//! `GetScript`, `Text.MeasureString` and `UI.FindSelectable`.

use rusty::scene::authoring::ui_widgets::UiWidget;

use super::{part, widget, Ui};

#[test]
fn hierarchy_reads_and_set_active() {
    let (mut sl, mut handle) = (0, 0);
    let mut ui = Ui::new(|s| {
        sl = widget(s, UiWidget::Slider);
        handle = part(s, sl, "Handle Slide Area/Handle");
    });
    let area = ui.eval(&format!("Scene.GetParent({handle})"));
    assert_eq!(
        ui.eval(&format!(
            "Scene.FindChild({sl}, 'Handle Slide Area/Handle')"
        )),
        handle.to_string()
    );
    assert_eq!(
        ui.eval(&format!("Scene.FindChild({sl}, 'Nope/Handle')")),
        "nil"
    );
    assert_eq!(ui.eval(&format!("#Scene.GetChildren({sl})")), "3");
    assert_eq!(
        ui.eval(&format!("Scene.GetChildren({area})[1]")),
        handle.to_string()
    );
    assert_eq!(ui.eval(&format!("Scene.SetActive({sl}, false)")), "true");
    assert_eq!(ui.eval(&format!("Scene.IsActive({sl})")), "false");
    ui.tick(1);
    assert_eq!(ui.eval(&format!("Scene.SetActive({sl}, true)")), "true");
    assert_eq!(ui.eval(&format!("Scene.IsActive({sl})")), "true");
    assert_eq!(ui.eval("Scene.IsActive(99999)"), "false");
}

#[test]
fn get_script_hands_out_the_live_instance() {
    let mut b = 0;
    let ui = Ui::new(|s| b = widget(s, UiWidget::Button));
    let same = format!("Scene.GetScript({b}, 'button') == Scene.GetScript({b}, 'button')");
    assert_eq!(ui.eval(&same), "true");
    assert_eq!(ui.eval(&format!("Scene.GetScript({b}, 'slider')")), "nil");
    assert_eq!(ui.eval("Scene.GetScript(99999, 'button')"), "nil");
}

#[test]
fn measure_string_counts_trailing_spaces() {
    let mut label = 0;
    let ui = Ui::new(|s| {
        let b = widget(s, UiWidget::Button);
        label = part(s, b, "Label");
    });
    let w = |s: &str| -> f32 {
        let out = ui.eval(&format!("(Text.MeasureString({label}, '{s}'))"));
        out.parse().expect("number")
    };
    assert_eq!(w(""), 0.0);
    assert!(w("ab ") > w("ab"), "a caret after a space sits past it");
    let two = ui.eval(&format!("select(2, Text.MeasureString({label}, 'a\\nb'))"));
    let one = ui.eval(&format!("select(2, Text.MeasureString({label}, 'a'))"));
    assert!(two.parse::<f32>().expect("h") > one.parse::<f32>().expect("h"));
}

#[test]
fn find_selectable_follows_navigation() {
    let (mut top, mut bottom) = (0, 0);
    let ui = Ui::new(|s| {
        top = widget(s, UiWidget::Button);
        bottom = widget(s, UiWidget::Button);
        s.world
            .rect_transform_mut(bottom)
            .expect("rect")
            .anchored_position
            .y = -200.0;
    });
    assert_eq!(
        ui.eval(&format!("UI.FindSelectable({top}, 'down')")),
        bottom.to_string()
    );
    assert_eq!(ui.eval(&format!("UI.FindSelectable({top}, 'Up')")), "nil");
    let err = ui
        .game
        .script_manager()
        .eval(&format!("UI.FindSelectable({top}, 'sideways')"));
    assert!(err.is_err());
}
