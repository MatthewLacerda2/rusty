//! The Shape inspector card (#425): the primitive and its per-kind fields, the
//! fill (colour or gradient), border, shadow, glow, blend mode and raycast target.
//! Widgets edit a snapshot; every write routes through `scene::authoring::shape`.

use egui_phosphor::regular as icon;

use super::look::{blend_row, color_row, drag_row, gradient_editor, vec2_edit};
use super::{combo, vec4_row};
use crate::components::{ShapeComponent, ShapeCorner, ShapeKind};
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::shape as ops;

/// Shape card. A THIN client (#287): widgets edit a snapshot and the changed
/// snapshot is written back field by field through the shared ops; remove detaches.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(mut edit) = world.shape(id).map(|s| s.clone()) else {
        return;
    };
    let mut remove = false;
    let mut changed = false;
    component_card(ui, icon::SHAPES, "Shape", Some(&mut remove), |ui| {
        changed |= draw_kind(ui, &mut edit);
        ui.separator();
        if edit.gradient.is_none() {
            changed |= color_row(ui, "Color:", &mut edit.color);
        }
        changed |= gradient_editor(ui, &mut edit.gradient);
        changed |= drag_row(ui, "Border:", &mut edit.border_width, 0.1);
        changed |= color_row(ui, "Border Color:", &mut edit.border_color);
        ui.separator();
        changed |= draw_effects(ui, &mut edit);
        changed |= blend_row(ui, &mut edit.blend);
        changed |= ui
            .checkbox(&mut edit.raycast_target, "Raycast Target")
            .changed();
        changed |= super::shader::row(ui, &mut edit.shader);
    });
    if changed {
        if let Some(mut s) = world.shape_mut(id) {
            write_back(&mut s, edit);
        }
    }
    if remove {
        world.set_shape(id, None);
    }
    *is_dirty |= remove || changed;
}

/// The kind and the fields it uses.
fn draw_kind(ui: &mut egui::Ui, s: &mut ShapeComponent) -> bool {
    use ShapeKind::{Ellipse, Line, Rect, Ring};
    let mut changed = combo(
        ui,
        "Kind",
        &mut s.kind,
        &[Rect, Ellipse, Ring, Line],
        ops::kind_name,
    );
    match s.kind {
        Rect => {
            let corners = [ShapeCorner::Round, ShapeCorner::Chamfer];
            changed |= combo(ui, "Corners", &mut s.corner, &corners, ops::corner_name);
            if let Some(r) = vec4_row(ui, "Radius (tl tr br bl):", s.radius) {
                s.radius = r;
                changed = true;
            }
        }
        Ellipse => {}
        Ring => {
            changed |= drag_row(ui, "Inner Radius:", &mut s.inner_radius, 0.5);
            changed |= drag_row(ui, "Arc Start°:", &mut s.arc_start, 1.0);
            changed |= drag_row(ui, "Arc End°:", &mut s.arc_end, 1.0);
        }
        Line => {
            changed |= drag_row(ui, "Thickness:", &mut s.thickness, 0.1);
            changed |= drag_row(ui, "Dash:", &mut s.dash, 0.5);
            changed |= drag_row(ui, "Gap:", &mut s.gap, 0.5);
        }
    }
    changed
}

/// Shadow and glow.
fn draw_effects(ui: &mut egui::Ui, s: &mut ShapeComponent) -> bool {
    let mut changed = vec2_edit(ui, "Shadow Offset:", &mut s.shadow.offset, 0.5);
    changed |= drag_row(ui, "Shadow Blur:", &mut s.shadow.blur, 0.1);
    changed |= color_row(ui, "Shadow Color:", &mut s.shadow.color);
    changed |= drag_row(ui, "Glow Size:", &mut s.glow.size, 0.1);
    changed |= drag_row(ui, "Glow Intensity:", &mut s.glow.intensity, 0.05);
    changed |= color_row(ui, "Glow Color:", &mut s.glow.color);
    changed
}

/// Write the edited snapshot back through the shared ops.
fn write_back(s: &mut ShapeComponent, e: ShapeComponent) {
    ops::set_kind(s, e.kind);
    ops::set_corner(s, e.corner);
    ops::set_radius(s, e.radius);
    ops::set_inner_radius(s, e.inner_radius);
    ops::set_arc(s, e.arc_start, e.arc_end);
    ops::set_thickness(s, e.thickness);
    ops::set_dash(s, e.dash, e.gap);
    ops::set_color(s, e.color);
    ops::set_gradient(s, e.gradient);
    ops::set_border(s, e.border_width, e.border_color);
    ops::set_shadow(s, e.shadow.offset, e.shadow.blur, e.shadow.color);
    ops::set_glow(s, e.glow.size, e.glow.intensity, e.glow.color);
    ops::set_blend(s, e.blend);
    ops::set_raycast_target(s, e.raycast_target);
    super::shader::write_back(&mut s.shader, e.shader);
}
