//! Widget-tree tests: the menu's canvas rule and each widget's named parts.

use super::*;

fn parts_of(scene: &Scene, id: u32) -> Vec<String> {
    let mut out = Vec::new();
    for c in scene.world.children(id) {
        out.push(scene.world.name(c).map(|n| n.clone()).unwrap_or_default());
        out.extend(parts_of(scene, c));
    }
    out
}

#[test]
fn a_widget_without_a_canvas_gets_one_and_reuses_it() {
    let mut scene = Scene::new();
    let b = create_ui(&mut scene, UiWidget::Button, None);
    let canvas = scene.world.parent_id(b).expect("under a canvas");
    assert!(scene.world.has_canvas(canvas));
    let s = create_ui(&mut scene, UiWidget::Slider, None);
    assert_eq!(
        scene.world.parent_id(s),
        Some(canvas),
        "the first root canvas is reused"
    );
    let inner = create_ui(&mut scene, UiWidget::Toggle, Some(b));
    assert_eq!(
        scene.world.parent_id(inner),
        Some(b),
        "a parent inside a canvas is kept"
    );
    let outside = scene.add_entity("Cube".into());
    let t = create_ui(&mut scene, UiWidget::Text, Some(outside));
    assert_eq!(
        scene.world.parent_id(t),
        Some(canvas),
        "a parent outside any canvas is not"
    );
}

#[test]
fn widgets_carry_their_script_and_named_parts() {
    let mut scene = Scene::new();
    for (kind, script, parts) in [
        (UiWidget::Button, "button", &["Label"][..]),
        (
            UiWidget::Toggle,
            "toggle",
            &["Background", "Checkmark", "Label"],
        ),
        (
            UiWidget::Slider,
            "slider",
            &["Fill Area", "Fill", "Handle Slide Area", "Handle"],
        ),
        (
            UiWidget::Scrollbar,
            "scrollbar",
            &["Sliding Area", "Handle"],
        ),
        (
            UiWidget::ScrollView,
            "scroll_view",
            &["Viewport", "Content", "Scrollbar Vertical"],
        ),
        (
            UiWidget::Dropdown,
            "dropdown",
            &["Label", "Arrow", "Template", "Viewport"],
        ),
        (
            UiWidget::InputField,
            "input_field",
            &["Text Area", "Placeholder", "Text", "Caret"],
        ),
    ] {
        let id = create_ui(&mut scene, kind, None);
        let scripts = scene.world.scripts(id).expect("scripts");
        assert!(
            scripts
                .iter()
                .any(|s| s.path.ends_with(&format!("ui/{script}.lua"))),
            "{kind:?} runs {script}.lua"
        );
        drop(scripts);
        let names = parts_of(&scene, id);
        for p in parts {
            assert!(
                names.iter().any(|n| n == p),
                "{kind:?} has a {p}: {names:?}"
            );
        }
    }
}

#[test]
fn labels_parse_loosely_and_prefabs_exist_for_all_but_canvas() {
    assert_eq!(UiWidget::parse("scrollview"), Some(UiWidget::ScrollView));
    assert_eq!(UiWidget::parse("Input Field"), Some(UiWidget::InputField));
    assert_eq!(UiWidget::parse("nope"), None);
    assert_eq!(UiWidget::Canvas.prefab_path(), None);
    assert_eq!(
        UiWidget::Dropdown.prefab_path().as_deref(),
        Some("project/prefabs/ui/Dropdown.prefab")
    );
}
