//! The in-game UI's inspector cards (#417–#421): the Canvas root, the
//! RectTransform every UI element carries, the Image graphic, the Shape graphic
//! (#425), the Text label, the Canvas Group, the Rect Mask and Mask, the Backdrop
//! Filter, the Selectable, and the Layout Group and Layout
//! Element. Each is a thin client over its shared
//! `scene::authoring` ops, the same ones the matching Lua namespace calls.

pub mod anchor_presets;
pub mod backdrop;
pub mod canvas;
pub mod canvas_group;
pub mod image;
pub mod layout_element;
pub mod layout_group;
mod look;
pub mod rect_mask;
pub mod rect_transform;
pub mod selectable;
pub mod shape;
pub mod text;

/// Draw every UI card the entity carries, in the order Unity's inspector shows them.
/// `view` is the viewport image's size in points (the UI lays out on its pixels).
pub fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    view: egui::Vec2,
    is_dirty: &mut bool,
) {
    let px = view * ui.ctx().pixels_per_point();
    let screen = glam::Vec2::new(px.x, px.y).round();
    canvas::draw(ui, world, id, is_dirty);
    rect_transform::draw(ui, world, id, screen, is_dirty);
    canvas_group::draw(ui, world, id, is_dirty);
    rect_mask::draw(ui, world, id, is_dirty);
    rect_mask::draw_mask(ui, world, id, is_dirty);
    image::draw(ui, world, id, is_dirty);
    shape::draw(ui, world, id, is_dirty);
    backdrop::draw(ui, world, id, is_dirty);
    text::draw(ui, world, id, is_dirty);
    selectable::draw(ui, world, id, is_dirty);
    layout_group::draw(ui, world, id, is_dirty);
    layout_element::draw(ui, world, id, is_dirty);
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

/// An entity-id field: `0` reads and writes as `None` (shown as `empty`).
pub(super) fn entity_row(
    ui: &mut egui::Ui,
    label: &str,
    empty: &str,
    value: &mut Option<u32>,
) -> bool {
    let mut raw = value.unwrap_or(0);
    let changed = ui
        .horizontal(|ui| {
            ui.label(label);
            let drag = egui::DragValue::new(&mut raw).custom_formatter(|v, _| {
                if v == 0.0 {
                    empty.to_string()
                } else {
                    format!("#{v}")
                }
            });
            ui.add(drag).changed()
        })
        .inner;
    *value = (raw != 0).then_some(raw);
    changed
}
