use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::{EntityRefs, RefVisit};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RectTransformComponent {
    pub anchor_min: Vec2,
    pub anchor_max: Vec2,
    pub pivot: Vec2,
    pub anchored_position: Vec2,
    pub size_delta: Vec2,
    pub world_anchor: Option<WorldAnchor>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldAnchor {
    pub target: Option<u32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub target_lost: bool,
    pub offset: Vec3,
    pub clamp_to_screen_edge: bool,
    pub edge_padding: f32,
    pub rotate_toward_target: bool,
    pub hide_when_behind: bool,
}

impl Default for WorldAnchor {
    fn default() -> Self {
        Self {
            target: None,
            target_lost: false,
            offset: Vec3::ZERO,
            clamp_to_screen_edge: false,
            edge_padding: 0.0,
            rotate_toward_target: false,
            hide_when_behind: true,
        }
    }
}

impl Default for RectTransformComponent {
    fn default() -> Self {
        Self {
            anchor_min: Vec2::splat(0.5),
            anchor_max: Vec2::splat(0.5),
            pivot: Vec2::splat(0.5),
            anchored_position: Vec2::ZERO,
            size_delta: Vec2::splat(100.0),
            world_anchor: None,
        }
    }
}

impl RectTransformComponent {
    pub fn layout_in(&self, parent_min: Vec2, parent_size: Vec2) -> (Vec2, Vec2) {
        let lo = parent_min + self.anchor_min * parent_size;
        let hi = parent_min + self.anchor_max * parent_size;
        let size = (hi - lo) + self.size_delta;
        let pivot_pos = lo + (hi - lo) * self.pivot + self.anchored_position;
        (pivot_pos - size * self.pivot, size)
    }

    pub const REF_POINTERS: [&'static str; 1] = ["/world_anchor/target"];
}

impl EntityRefs for RectTransformComponent {
    fn visit_refs(&mut self, visit: &mut RefVisit<'_>) {
        if let Some(a) = &mut self.world_anchor {
            visit(Self::REF_POINTERS[0], &mut a.target);
        }
    }

    fn forget(&mut self, dead: &dyn Fn(u32) -> bool) {
        if let Some(a) = &mut self.world_anchor {
            if a.target.is_some_and(dead) {
                a.target = None;
                a.target_lost = true;
            }
        }
    }

    fn each_ref(&self, visit: &mut dyn FnMut(&str, u32)) {
        if let Some(r) = self.world_anchor.as_ref().and_then(|a| a.target) {
            visit(Self::REF_POINTERS[0], r);
        }
    }

    #[doc(hidden)]
    fn every_ref() -> Self {
        Self {
            world_anchor: Some(WorldAnchor::default()),
            ..Self::default()
        }
    }
}

impl crate::FirstClass for RectTransformComponent {
    const NAME: &'static str = "RectTransform";
}
