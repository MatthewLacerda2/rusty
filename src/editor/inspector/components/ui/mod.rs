//! The in-game UI's inspector cards (#417, #418, #419): the Canvas root, the
//! RectTransform every UI element carries, the Image graphic, the Text label, the
//! Canvas Group and the Rect Mask. Each is a thin client over its shared
//! `scene::authoring` ops, the same ones the matching Lua namespace calls.

pub mod canvas;
pub mod canvas_group;
pub mod image;
pub mod rect_mask;
pub mod rect_transform;
pub mod text;

/// Draw every UI card the entity carries, in the order Unity's inspector shows them.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    canvas::draw(ui, world, id, is_dirty);
    rect_transform::draw(ui, world, id, is_dirty);
    canvas_group::draw(ui, world, id, is_dirty);
    rect_mask::draw(ui, world, id, is_dirty);
    image::draw(ui, world, id, is_dirty);
    text::draw(ui, world, id, is_dirty);
}

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

/// A labelled `x, y, z, w` drag row (left, bottom, right, top for borders and
/// padding). Returns the edited value when any component changed.
fn vec4_row(ui: &mut egui::Ui, label: &str, value: glam::Vec4) -> Option<glam::Vec4> {
    let mut v = value.to_array();
    let changed = ui
        .horizontal(|ui| {
            ui.label(label);
            let mut any = false;
            for (c, prefix) in v.iter_mut().zip(["l ", "b ", "r ", "t "]) {
                any |= ui.add(egui::DragValue::new(c).prefix(prefix)).changed();
            }
            any
        })
        .inner;
    changed.then(|| glam::Vec4::from_array(v))
}

/// A labelled combo box over `options`, named by `name`.
fn combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    options: &[T],
    name: fn(T) -> &'static str,
) -> bool {
    let before = *value;
    egui::ComboBox::from_label(label)
        .selected_text(name(*value))
        .show_ui(ui, |ui| {
            for &o in options {
                ui.selectable_value(value, o, name(o));
            }
        });
    *value != before
}
