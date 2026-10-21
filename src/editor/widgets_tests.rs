use super::DragRange;

/// One headless egui pass over a ranged drag of `value`; return what it left.
fn shown(value: f32, range: std::ops::RangeInclusive<f32>) -> f32 {
    let ctx = egui::Context::default();
    let mut v = value;
    let mut out = ctx.run_ui(Default::default(), |ui| {
        ui.add(egui::DragValue::new(&mut v).ranged(range.clone()));
    });
    out.textures_delta.clear();
    v
}

/// Selecting an entity must not rewrite a value a scene or script set out of range.
#[test]
fn ranged_leaves_an_out_of_range_value_unchanged() {
    assert_eq!(shown(200.0, 0.01..=100.0), 200.0);
    assert_eq!(shown(-3.0, 0.0..=1.0), -3.0);
}

#[test]
fn ranged_leaves_an_in_range_value_unchanged() {
    assert_eq!(shown(0.5, 0.0..=1.0), 0.5);
}

/// The reason for the helper: egui's own `range` rewrites the shown value.
#[test]
fn egui_range_alone_clamps_the_shown_value() {
    let ctx = egui::Context::default();
    let mut v = 200.0_f32;
    for _ in 0..2 {
        let mut out = ctx.run_ui(Default::default(), |ui| {
            // lint: allow(drag-range) pins the egui default the helper exists to turn off
            ui.add(egui::DragValue::new(&mut v).range(0.01..=100.0));
        });
        out.textures_delta.clear();
    }
    assert_eq!(v, 100.0);
}
