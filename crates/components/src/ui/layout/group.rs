use glam::{Vec2, Vec4};
use serde::{Deserialize, Serialize};

use crate::ui::TextAlignment;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum LayoutKind {
        #[default]
        Horizontal = "Horizontal",
        Vertical = "Vertical",
        Grid = "Grid",
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum LayoutConstraint {
        #[default]
        Flexible = "Flexible",
        FixedColumnCount = "FixedColumnCount",
        FixedRowCount = "FixedRowCount",
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum LayoutCorner {
        #[default]
        UpperLeft = "UpperLeft",
        UpperRight = "UpperRight",
        LowerLeft = "LowerLeft",
        LowerRight = "LowerRight",
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutGroupComponent {
    pub kind: LayoutKind,
    pub padding: Vec4,
    pub spacing: Vec2,
    pub child_alignment: TextAlignment,
    pub control_child_width: bool,
    pub control_child_height: bool,
    pub child_force_expand_width: bool,
    pub child_force_expand_height: bool,
    pub cell_size: Vec2,
    pub constraint: LayoutConstraint,
    pub constraint_count: u32,
    pub start_corner: LayoutCorner,
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
    pub fn controls(&self, axis: usize) -> bool {
        [self.control_child_width, self.control_child_height][axis]
    }

    pub fn force_expands(&self, axis: usize) -> bool {
        [
            self.child_force_expand_width,
            self.child_force_expand_height,
        ][axis]
    }
}

impl crate::FirstClass for LayoutGroupComponent {
    const NAME: &'static str = "LayoutGroup";
}
