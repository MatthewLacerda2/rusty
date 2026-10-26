use super::{check, RULE};

#[test]
fn flags_a_bare_range_on_a_drag_value() {
    let src = "let d = egui::DragValue::new(&mut v)\n    .range(0.0..=1.0);\n";
    let found: Vec<_> = check("src/editor/a.rs", src)
        .iter()
        .map(|f| (f.line, f.rule))
        .collect();
    assert_eq!(found, [(2, RULE)]);
}

#[test]
fn passes_the_helper_comments_other_types_and_other_dirs() {
    let ranged = "egui::DragValue::new(&mut v).ranged(0.0..=1.0);\n";
    assert!(check("src/editor/a.rs", ranged).is_empty());
    assert!(check("src/editor/a.rs", "// DragValue .range(0..=1)\n").is_empty());
    assert!(check("src/editor/a.rs", "let r = slider.range(0..=1);\n").is_empty());
    let bare = "DragValue::new(&mut v).range(0.0..=1.0);\n";
    assert!(check("src/render/a.rs", bare).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(drag-range) the helper itself\nDragValue::new(v).range(r);\n";
    let found = check("src/editor/a.rs", src);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}

#[test]
fn flags_a_bare_slider_and_passes_the_helper() {
    let src = "ui.add(egui::Slider::new(&mut v, 0.0..=1.0));\n";
    let found: Vec<_> = check("src/editor/a.rs", src)
        .iter()
        .map(|f| f.line)
        .collect();
    assert_eq!(found, [1]);
    let helper = "ui.add(widgets::slider(&mut v, 0.0..=1.0));\n";
    assert!(check("src/editor/a.rs", helper).is_empty());
    assert!(check("src/render/a.rs", src).is_empty());
}
