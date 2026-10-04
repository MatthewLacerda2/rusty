//! The Layout Group inspector card (#421): the kind, padding, spacing and child
//! alignment; the row/column control and force-expand flags; the grid's cell
//! size, constraint, start corner and axis. Widgets edit a snapshot; every write
//! routes through `scene::authoring::layout_group`.

use egui_phosphor::regular as icon;

use super::{combo, vec2_row, vec4_row};
use crate::components::{LayoutGroupComponent, LayoutKind, TextAlignment};
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::layout_group::{self as ops, name_of, CONSTRAINTS, CORNERS, KINDS};
use crate::scene::authoring::text::alignment_name;

/// Layout Group card. A THIN client (#287): widgets edit a snapshot and the
/// changed snapshot is written back through the shared ops; remove detaches.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(mut edit) = world.layout_group(id).map(|g| g.clone()) else {
        return;
    };
    let mut remove = false;
    let mut changed = false;
    component_card(ui, icon::LAYOUT, "Layout Group", Some(&mut remove), |ui| {
        let kinds = KINDS.map(|(k, _)| k);
        changed |= combo(ui, "Kind", &mut edit.kind, &kinds, |k| name_of(&KINDS, k));
        changed |= draw_common(ui, &mut edit);
        if edit.kind == LayoutKind::Grid {
            changed |= draw_grid(ui, &mut edit);
        } else {
            changed |= draw_linear(ui, &mut edit);
        }
    });
    if changed {
        if let Some(mut g) = world.layout_group_mut(id) {
            write_back(&mut g, &edit);
        }
    }
    if remove {
        world.set_layout_group(id, None);
    }
    *is_dirty |= remove || changed;
}

/// Padding, spacing and child alignment.
fn draw_common(ui: &mut egui::Ui, g: &mut LayoutGroupComponent) -> bool {
    let mut changed = false;
    if let Some(p) = vec4_row(ui, "Padding:", g.padding) {
        g.padding = p;
        changed = true;
    }
    if let Some(s) = vec2_row(ui, "Spacing:", g.spacing, 1.0) {
        g.spacing = s;
        changed = true;
    }
    let all = TextAlignment::ALL;
    changed
        | combo(
            ui,
            "Child Alignment",
            &mut g.child_alignment,
            &all,
            alignment_name,
        )
}

/// The row/column flags.
fn draw_linear(ui: &mut egui::Ui, g: &mut LayoutGroupComponent) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Control Child Size:");
        changed |= ui.checkbox(&mut g.control_child_width, "Width").changed();
        changed |= ui.checkbox(&mut g.control_child_height, "Height").changed();
    });
    ui.horizontal(|ui| {
        ui.label("Child Force Expand:");
        changed |= ui
            .checkbox(&mut g.child_force_expand_width, "Width")
            .changed();
        changed |= ui
            .checkbox(&mut g.child_force_expand_height, "Height")
            .changed();
    });
    changed
}

/// The grid's cell size, constraint and fill order.
fn draw_grid(ui: &mut egui::Ui, g: &mut LayoutGroupComponent) -> bool {
    let mut changed = false;
    if let Some(s) = vec2_row(ui, "Cell Size:", g.cell_size, 1.0) {
        g.cell_size = s;
        changed = true;
    }
    let constraints = CONSTRAINTS.map(|(c, _)| c);
    let name = |c| name_of(&CONSTRAINTS, c);
    changed |= combo(ui, "Constraint", &mut g.constraint, &constraints, name);
    ui.horizontal(|ui| {
        ui.label("Constraint Count:");
        let count = egui::DragValue::new(&mut g.constraint_count).range(1..=64);
        changed |= ui.add(count).changed();
    });
    let corners = CORNERS.map(|(c, _)| c);
    changed |= combo(ui, "Start Corner", &mut g.start_corner, &corners, |c| {
        name_of(&CORNERS, c)
    });
    changed
        | ui.checkbox(&mut g.start_vertical, "Fill Columns First")
            .changed()
}

/// Write the edited snapshot back through the shared ops.
fn write_back(g: &mut LayoutGroupComponent, e: &LayoutGroupComponent) {
    ops::set_kind(g, e.kind);
    ops::set_padding(g, e.padding);
    ops::set_spacing(g, e.spacing);
    ops::set_child_alignment(g, e.child_alignment);
    ops::set_control_child_size(g, e.control_child_width, e.control_child_height);
    ops::set_child_force_expand(g, e.child_force_expand_width, e.child_force_expand_height);
    ops::set_cell_size(g, e.cell_size);
    ops::set_constraint(g, e.constraint);
    ops::set_constraint_count(g, e.constraint_count);
    ops::set_start_corner(g, e.start_corner);
    ops::set_start_vertical(g, e.start_vertical);
}
