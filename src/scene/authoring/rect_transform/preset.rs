//! src/scene/authoring/rect_transform/preset.rs — anchor presets (#423).
//!
//! Unity's 4×4 anchor-preset grid: each axis is pinned to its parent's min edge,
//! centre or max edge, or stretched across it. By default the element **stays
//! where it is** — the anchored position and size delta are re-derived so the rect
//! does not move (Unity's plain click). `set_pivot` also moves the pivot to the
//! preset's point (Shift); `set_position` also snaps the element onto its anchors
//! (Alt): a point anchor takes the pivot, a stretch fills the parent span. The
//! RectTransform card and `RectTransform.SetAnchorPreset` both call
//! [`apply_anchor_preset`].

use glam::Vec2;

use crate::components::RectTransformComponent;

/// Where one axis's anchors sit in the parent rect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisPreset {
    /// The left (x) or bottom (y) edge.
    Min,
    /// The centre.
    Center,
    /// The right (x) or top (y) edge.
    Max,
    /// From edge to edge.
    Stretch,
}

impl AxisPreset {
    /// Every preset, in the grid's order.
    pub const ALL: [Self; 4] = [Self::Min, Self::Center, Self::Max, Self::Stretch];

    /// `(anchor_min, anchor_max, pivot)` along the axis.
    fn anchors(self) -> (f32, f32, f32) {
        match self {
            Self::Min => (0.0, 0.0, 0.0),
            Self::Center => (0.5, 0.5, 0.5),
            Self::Max => (1.0, 1.0, 1.0),
            Self::Stretch => (0.0, 1.0, 0.5),
        }
    }

    /// The preset an axis's `anchor_min`/`anchor_max` already form, if any.
    pub fn of(min: f32, max: f32) -> Option<Self> {
        Self::ALL.into_iter().find(|p| {
            let (lo, hi, _) = p.anchors();
            lo == min && hi == max
        })
    }

    /// The script name along `axis` (0 = x, 1 = y): `left`/`center`/`right`, or
    /// `bottom`/`middle`/`top`; `stretch` on both.
    pub fn name(self, axis: usize) -> &'static str {
        let names = match axis {
            0 => ["left", "center", "right", "stretch"],
            _ => ["bottom", "middle", "top", "stretch"],
        };
        names[self as usize]
    }

    /// The preset named `name` along `axis` (see [`AxisPreset::name`]).
    pub fn from_name(name: &str, axis: usize) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name(axis) == name)
    }
}

/// One cell of the preset grid plus its modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnchorPreset {
    pub x: AxisPreset,
    pub y: AxisPreset,
    /// Also move the pivot to the preset's point (Unity's Shift).
    pub set_pivot: bool,
    /// Also snap the element onto its new anchors (Unity's Alt).
    pub set_position: bool,
}

/// Anchor `r` by `preset` inside a parent rect `parent_size` reference units big
/// (see the module docs). Without `set_position` the laid-out rect is unchanged.
pub fn apply_anchor_preset(
    r: &mut RectTransformComponent,
    preset: AnchorPreset,
    parent_size: Vec2,
) {
    let (min, size) = r.layout_in(Vec2::ZERO, parent_size);
    for (axis, p) in [preset.x, preset.y].into_iter().enumerate() {
        let (lo, hi, pivot) = p.anchors();
        r.anchor_min[axis] = lo;
        r.anchor_max[axis] = hi;
        if preset.set_pivot {
            r.pivot[axis] = pivot;
        }
        let span = (hi - lo) * parent_size[axis];
        if preset.set_position {
            r.anchored_position[axis] = 0.0;
            r.size_delta[axis] = if p == AxisPreset::Stretch {
                0.0
            } else {
                size[axis]
            };
        } else {
            let reference = lo * parent_size[axis] + span * r.pivot[axis];
            r.size_delta[axis] = size[axis] - span;
            r.anchored_position[axis] = min[axis] + size[axis] * r.pivot[axis] - reference;
        }
    }
}

#[cfg(test)]
#[path = "preset_tests.rs"]
mod preset_tests;
