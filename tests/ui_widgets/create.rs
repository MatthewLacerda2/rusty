//! `UI.Create` (the Create ▸ UI menu's API face) and the seeded widget prefabs.

use super::Ui;

#[test]
fn ui_create_builds_under_a_canvas_and_rejects_unknown_kinds() {
    let ui = Ui::new(|_| {});
    let b = ui.eval("UI.Create('Button')");
    let canvas = ui.eval(&format!("Scene.GetParent({b})"));
    assert_eq!(ui.eval(&format!("Canvas.GetSortOrder({canvas})")), "0");
    let t = ui.eval(&format!("UI.Create('input field', {b})"));
    assert_eq!(ui.eval(&format!("Scene.GetParent({t})")), b);
    let err = ui.game.script_manager().eval("UI.Create('Knob')");
    assert!(
        err.is_err_and(|e| e.contains("Scroll View")),
        "the error lists the kinds"
    );
}

#[test]
fn a_seeded_prefab_instantiates_into_a_working_widget() {
    let mut ui = Ui::new(|_| {});
    let canvas = ui.eval("UI.Create('Canvas')");
    let b = ui.eval(&format!(
        "Scene.Instantiate('project/prefabs/ui/Button.prefab', {canvas})"
    ));
    ui.tick(1);
    ui.eval(&format!(
        "Scene.GetScript({b}, 'button').on_click = function() print('[w] prefab click') end"
    ));
    ui.click(b.parse().expect("id"));
    assert_eq!(ui.log(), vec!["[w] prefab click"], "{:?}", ui.errors());
}
