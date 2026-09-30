//! The in-game UI's inspector cards (#417): the Canvas root and the RectTransform
//! every UI element carries. Both are thin clients over the shared
//! `scene::authoring::{canvas, rect_transform}` ops, the same ones the `Canvas` and
//! `RectTransform` Lua namespaces call.

pub mod canvas;
pub mod rect_transform;

/// A labelled `x, y` drag row. Returns the edited pair when either axis changed.
fn vec2_row(ui: &mut egui::Ui, label: &str, value: glam::Vec2, speed: f32) -> Option<glam::Vec2> {
    let (mut x, mut y) = (value.x, value.y);
    let changed = ui
        .horizontal(|ui| {
            ui.label(label);
            let cx = ui.add(egui::DragValue::new(&mut x).speed(speed).prefix("x "));
            let cy = ui.add(egui::DragValue::new(&mut y).speed(speed).prefix("y "));
            cx.changed() || cy.changed()
        })
        .inner;
    changed.then(|| glam::Vec2::new(x, y))
}
