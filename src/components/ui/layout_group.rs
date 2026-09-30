//! src/components/ui/layout_group.rs — LayoutGroup: arrange UI children (#421).
//!
//! Unity's `HorizontalLayoutGroup`, `VerticalLayoutGroup` and `GridLayoutGroup` in
//! one component, picked by `kind`. The group places every active child that has
//! a `RectTransform` (and no `LayoutElement.ignore_layout`) inside its own rect,
//! overriding the child's anchors: a row, a column, or a grid of equal cells. Its
//! own preferred size (from its children) is what a parent group or a content
//! fitter reads. The layout pass computes all of it (`ui::layout`); this is pure
//! authoring data, and the child RectTransforms are never rewritten.

use glam::{Vec2, Vec4};
use serde::{Deserialize, Serialize};

use super::TextAlignment;

/// Which arrangement a group makes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutKind {
    /// One row, left to right.
    #[default]
    Horizontal,
    /// One column, top to bottom.
    Vertical,
    /// Equal `cell_size` cells in rows and columns.
    Grid,
}

/// How a grid picks its column / row count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutConstraint {
    /// As many columns (or rows) as fit the rect.
    #[default]
    Flexible,
    /// Exactly `constraint_count` columns.
    FixedColumnCount,
    /// Exactly `constraint_count` rows.
    FixedRowCount,
}

/// The grid corner the first child sits in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutCorner {
    #[default]
    UpperLeft,
    UpperRight,
    LowerLeft,
    LowerRight,
}

/// A layout group. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutGroupComponent {
    /// Row, column or grid.
    pub kind: LayoutKind,
    /// Inset of the children from the rect in reference units — `x` left, `y`
    /// bottom, `z` right, `w` top (the `RectMask` order).
    pub padding: Vec4,
    /// Gap between columns (`x`) and between rows (`y`): a row uses `x`, a column
    /// `y`, a grid both.
    pub spacing: Vec2,
    /// Where the children sit when they do not fill the rect (Unity's
    /// `childAlignment`).
    pub child_alignment: TextAlignment,
    /// Row / column: size each child's width from its layout sizes (else its own
    /// `size_delta.x` is kept).
    pub control_child_width: bool,
    /// Row / column: size each child's height from its layout sizes.
    pub control_child_height: bool,
    /// Row / column: every child is at least flexible 1 along the width, so spare
    /// width is shared out.
    pub child_force_expand_width: bool,
    /// Row / column: every child is at least flexible 1 along the height.
    pub child_force_expand_height: bool,
    /// Grid: every cell's size, reference units.
    pub cell_size: Vec2,
    /// Grid: how the column / row count is chosen.
    pub constraint: LayoutConstraint,
    /// Grid: the fixed column or row count (at least 1).
    pub constraint_count: u32,
    /// Grid: the corner the first child sits in.
    pub start_corner: LayoutCorner,
    /// Grid: fill columns first (down, then across) instead of rows.
    pub start_vertical: bool,
}

impl Default for LayoutGroupComponent {
    fn default() -> Self {
        Self {
            kind: LayoutKind::Horizontal,
            padding: Vec4::ZERO,
            spacing: Vec2::ZERO,
            child_alignment: TextAlignment::TopLeft,
            control_child_width: true,
            control_child_height: true,
            child_force_expand_width: true,
            child_force_expand_height: true,
            cell_size: Vec2::splat(100.0),
            constraint: LayoutConstraint::Flexible,
            constraint_count: 2,
            start_corner: LayoutCorner::UpperLeft,
            start_vertical: false,
        }
    }
}

impl LayoutGroupComponent {
    /// Whether the group sizes its children along `axis` (0 width, 1 height).
    pub fn controls(&self, axis: usize) -> bool {
        [self.control_child_width, self.control_child_height][axis]
    }

    /// Whether the group force-expands its children along `axis`.
    pub fn force_expands(&self, axis: usize) -> bool {
        [
            self.child_force_expand_width,
            self.child_force_expand_height,
        ][axis]
    }
}
