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

/// One headless egui pass over a [`super::slider`] of `value`; what it left, and
/// whether it reported a change.
fn slid(value: f32, range: std::ops::RangeInclusive<f32>) -> (f32, bool) {
    let ctx = egui::Context::default();
    let (mut v, mut changed) = (value, false);
    let mut out = ctx.run_ui(Default::default(), |ui| {
        changed = ui.add(super::slider(&mut v, range.clone())).changed();
    });
    out.textures_delta.clear();
    (v, changed)
}

#[test]
fn slider_leaves_an_out_of_range_value_unchanged() {
    assert_eq!(slid(6.0, -4.0..=4.0), (6.0, false));
    assert_eq!(slid(-9.0, 0.0..=1.0), (-9.0, false));
    assert_eq!(slid(0.5, 0.0..=1.0), (0.5, false));
}

/// The reason for the helper: egui's default slider rewrites the shown value.
#[test]
fn egui_slider_alone_clamps_the_shown_value() {
    let ctx = egui::Context::default();
    let mut v = 6.0_f32;
    let mut out = ctx.run_ui(Default::default(), |ui| {
        // lint: allow(drag-range) pins the egui default the helper exists to turn off
        ui.add(egui::Slider::new(&mut v, -4.0..=4.0));
    });
    out.textures_delta.clear();
    assert_eq!(v, 4.0);
}
