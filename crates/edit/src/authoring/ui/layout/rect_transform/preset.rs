//! By default the element stays where it is: the anchored position and size delta are re-derived so
//! the rect does not move (Unity's plain click). `set_pivot` is Unity's Shift, `set_position` its
//! Alt.

use glam::Vec2;

use rusty_components::RectTransformComponent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisPreset {
    Min,
    Center,
    Max,
    Stretch,
}

impl AxisPreset {
    pub const ALL: [Self; 4] = [Self::Min, Self::Center, Self::Max, Self::Stretch];

    fn anchors(self) -> (f32, f32, f32) {
        match self {
            Self::Min => (0.0, 0.0, 0.0),
            Self::Center => (0.5, 0.5, 0.5),
            Self::Max => (1.0, 1.0, 1.0),
            Self::Stretch => (0.0, 1.0, 0.5),
        }
    }

    pub fn of(min: f32, max: f32) -> Option<Self> {
        Self::ALL.into_iter().find(|p| {
            let (lo, hi, _) = p.anchors();
            lo == min && hi == max
        })
    }

    pub fn name(self, axis: usize) -> &'static str {
        let names = match axis {
            0 => ["left", "center", "right", "stretch"],
            _ => ["bottom", "middle", "top", "stretch"],
        };
        names[self as usize]
    }

    pub fn from_name(name: &str, axis: usize) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name(axis) == name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnchorPreset {
    pub x: AxisPreset,
    pub y: AxisPreset,
    pub set_pivot: bool,
    pub set_position: bool,
}

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
