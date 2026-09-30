//! Button and Toggle (+ Toggle Group): owner callbacks wired with `Scene.GetScript`.

use rusty::scene::authoring::ui_widgets::UiWidget;

use super::{part, widget, Ui};

#[test]
fn a_button_raises_on_click_for_clicks_and_enter_but_not_when_disabled() {
    let mut b = 0;
    let mut ui = Ui::new(|s| b = widget(s, UiWidget::Button));
    ui.eval(&format!(
        "Scene.GetScript({b}, 'button').on_click = function(id) print('[w] click ' .. id) end"
    ));
    ui.click(b);
    ui.key("Enter");
    ui.eval(&format!("Selectable.SetInteractable({b}, false)"));
    ui.click(b);
    ui.key("Enter");
    assert_eq!(
        ui.log(),
        vec![format!("[w] click {b}"), format!("[w] click {b}")]
    );
}

#[test]
fn a_toggle_flips_and_fades_its_checkmark() {
    let (mut t, mut check) = (0, 0);
    let mut ui = Ui::new(|s| {
        t = widget(s, UiWidget::Toggle);
        check = part(s, t, "Background/Checkmark");
    });
    let script = format!("Scene.GetScript({t}, 'toggle')");
    ui.eval(&format!(
        "{script}.on_value_changed = function(id, on) print('[w] ' .. tostring(on)) end"
    ));
    assert_eq!(ui.eval(&format!("{script}.is_on")), "true");
    ui.click(t);
    assert_eq!(ui.eval(&format!("{script}.is_on")), "false");
    ui.tick(12);
    let alpha = format!("select(4, Image.GetColor({check}))");
    assert_eq!(ui.eval(&alpha), "0", "faded out on unscaled time");
    ui.eval("Time.SetTimeScale(0)");
    ui.key("Enter");
    ui.tick(12);
    assert_eq!(ui.eval(&alpha), "1", "fades in under a paused game");
    assert_eq!(ui.log(), vec!["[w] false", "[w] true"]);
}

#[test]
fn a_toggle_group_keeps_exactly_one_on() {
    let mut g = 0;
    let mut ui = Ui::new(|s| g = widget(s, UiWidget::ToggleGroup));
    let (one, two) = {
        let scene = ui.game.scene().borrow();
        (part(&scene, g, "Option 1"), part(&scene, g, "Option 2"))
    };
    let on = |ui: &Ui, id: u32| ui.eval(&format!("Scene.GetScript({id}, 'toggle').is_on"));
    assert_eq!(
        (on(&ui, one), on(&ui, two)),
        ("true".into(), "false".into()),
        "{:?}",
        ui.errors()
    );
    ui.click(two);
    assert_eq!(
        (on(&ui, one), on(&ui, two)),
        ("false".into(), "true".into())
    );
    ui.click(two);
    assert_eq!(on(&ui, two), "true", "the on one cannot switch off");
    ui.eval(&format!(
        "Scene.GetScript({g}, 'toggle_group').allow_switch_off = true"
    ));
    ui.click(two);
    assert_eq!(
        (on(&ui, one), on(&ui, two)),
        ("false".into(), "false".into())
    );
}
