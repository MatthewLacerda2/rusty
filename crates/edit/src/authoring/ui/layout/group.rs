//! Padding may be negative (it pushes children out); a cell size never is.

use glam::{Vec2, Vec4};

use rusty_components::{
    LayoutConstraint, LayoutCorner, LayoutGroupComponent, LayoutKind, TextAlignment,
};
use rusty_scene::rules::sanitize::{finite2, finite4};

pub fn set_kind(g: &mut LayoutGroupComponent, kind: LayoutKind) {
    g.kind = kind;
}

pub fn set_padding(g: &mut LayoutGroupComponent, padding: Vec4) {
    g.padding = finite4(padding);
}

pub fn set_spacing(g: &mut LayoutGroupComponent, spacing: Vec2) {
    g.spacing = finite2(spacing);
}

pub fn set_child_alignment(g: &mut LayoutGroupComponent, alignment: TextAlignment) {
    g.child_alignment = alignment;
}

pub fn set_control_child_size(g: &mut LayoutGroupComponent, width: bool, height: bool) {
    g.control_child_width = width;
    g.control_child_height = height;
}

pub fn set_child_force_expand(g: &mut LayoutGroupComponent, width: bool, height: bool) {
    g.child_force_expand_width = width;
    g.child_force_expand_height = height;
}

pub fn set_cell_size(g: &mut LayoutGroupComponent, size: Vec2) {
    g.cell_size = finite2(size).max(Vec2::ZERO);
}

pub fn set_constraint(g: &mut LayoutGroupComponent, constraint: LayoutConstraint) {
    g.constraint = constraint;
}

pub fn set_constraint_count(g: &mut LayoutGroupComponent, count: u32) {
    g.constraint_count = count.max(1);
}

pub fn set_start_corner(g: &mut LayoutGroupComponent, corner: LayoutCorner) {
    g.start_corner = corner;
}

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
