//! src/scene/authoring/layout_group.rs — Shared LayoutGroup-authoring ops (#421).
//!
//! The ONE place the engine mutates an entity's first-class `LayoutGroupComponent`.
//! The editor's Layout Group card and the Lua `LayoutGroup.*` namespace both route
//! every write through these: padding may be negative (it pushes children out),
//! a cell size never is, and a fixed column / row count is at least 1.
//!
//! Pure.

use glam::{Vec2, Vec4};

use super::sanitize::{finite2, finite4};
use crate::components::{
    LayoutConstraint, LayoutCorner, LayoutGroupComponent, LayoutKind, TextAlignment,
};

/// Set the arrangement.
pub fn set_kind(g: &mut LayoutGroupComponent, kind: LayoutKind) {
    g.kind = kind;
}

/// Set the inset from the rect (left, bottom, right, top), reference units.
pub fn set_padding(g: &mut LayoutGroupComponent, padding: Vec4) {
    g.padding = finite4(padding);
}

/// Set the gap between columns (`x`) and rows (`y`).
pub fn set_spacing(g: &mut LayoutGroupComponent, spacing: Vec2) {
    g.spacing = finite2(spacing);
}

/// Set where the children sit when they do not fill the rect.
pub fn set_child_alignment(g: &mut LayoutGroupComponent, alignment: TextAlignment) {
    g.child_alignment = alignment;
}

/// Set whether the group sizes its children's width / height.
pub fn set_control_child_size(g: &mut LayoutGroupComponent, width: bool, height: bool) {
    g.control_child_width = width;
    g.control_child_height = height;
}

/// Set whether the children expand to share spare width / height.
pub fn set_child_force_expand(g: &mut LayoutGroupComponent, width: bool, height: bool) {
    g.child_force_expand_width = width;
    g.child_force_expand_height = height;
}

/// Set the grid's cell size, each axis floored at 0.
pub fn set_cell_size(g: &mut LayoutGroupComponent, size: Vec2) {
    g.cell_size = finite2(size).max(Vec2::ZERO);
}

/// Set how the grid picks its column / row count.
pub fn set_constraint(g: &mut LayoutGroupComponent, constraint: LayoutConstraint) {
    g.constraint = constraint;
}

/// Set the grid's fixed column / row count, at least 1.
pub fn set_constraint_count(g: &mut LayoutGroupComponent, count: u32) {
    g.constraint_count = count.max(1);
}

/// Set the corner the grid's first child sits in.
pub fn set_start_corner(g: &mut LayoutGroupComponent, corner: LayoutCorner) {
    g.start_corner = corner;
}

/// Set whether the grid fills columns first.
pub fn set_start_vertical(g: &mut LayoutGroupComponent, vertical: bool) {
    g.start_vertical = vertical;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_writes_are_clamped() {
        let mut g = LayoutGroupComponent::default();
        set_cell_size(&mut g, Vec2::new(-5.0, 20.0));
        set_constraint_count(&mut g, 0);
        assert_eq!(g.cell_size, Vec2::new(0.0, 20.0));
        assert_eq!(g.constraint_count, 1);
    }
}
