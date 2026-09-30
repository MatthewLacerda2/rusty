//! src/components/ui/rect_transform.rs — RectTransform: 2D placement inside a parent rect (#417).
//!
//! Unity's `RectTransform`, beside (never instead of) the mandatory Transform. It
//! places a UI element relative to its parent's rect with Unity's exact semantics:
//! the anchors mark a sub-rect of the parent (a point when `anchor_min ==
//! anchor_max`, a stretch region otherwise); `size_delta` is the element's size
//! *minus* that anchor region; `anchored_position` is the pivot's offset from the
//! anchor reference point (the anchors lerped by the pivot). The entity's Transform
//! keeps **rotation and scale**, applied around the pivot; its position is ignored
//! for rect-laid-out entities.
//!
//! Units are the owning canvas's reference units, y-up, origin at the bottom-left —
//! Unity's convention. Pure authoring data; the computed rect lives in the
//! `ui::UiLayout` resource.
//!
//! **World anchors (#429).** An element with a [`WorldAnchor`] is a *marker*: the
//! layout pins its pivot to where a world point (an entity, plus an offset) lands on
//! screen each tick, optionally clamped to the screen edge and turned toward the
//! target — enemy health bars, waypoints, damage numbers. The anchor holds an
//! entity reference, so it follows prefab save / stamp ([`RectTransformComponent::remap_refs`]).

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

/// A UI element's placement relative to its parent rect. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RectTransformComponent {
    /// Lower-left anchor, as a fraction of the parent rect (0..1 per axis).
    pub anchor_min: Vec2,
    /// Upper-right anchor, as a fraction of the parent rect (0..1 per axis).
    pub anchor_max: Vec2,
    /// The point the element rotates and scales around, and that
    /// `anchored_position` places — a fraction of the element's own rect.
    pub pivot: Vec2,
    /// The pivot's offset from the anchor reference point, in reference units.
    pub anchored_position: Vec2,
    /// The element's size minus the anchor region's size, in reference units. With
    /// point anchors this is simply the size; with stretch anchors, a negative value
    /// insets the element from the anchor edges.
    pub size_delta: Vec2,
    /// Pins the element to a world point instead of its anchors (a marker); see
    /// the module docs. `None` for ordinary UI.
    pub world_anchor: Option<WorldAnchor>,
}

/// Where a marker points and how it behaves off-screen (#429). Only screen-space
/// canvases place markers; on a `WorldSpace` canvas the anchor is ignored.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldAnchor {
    /// The entity followed (its world position); `None` makes `offset` a fixed
    /// world point. A target that no longer exists hides the marker.
    pub target: Option<u32>,
    /// Added to the target's world position, in metres (a bar above a head).
    pub offset: Vec3,
    /// Keep the marker on screen: an off-screen or behind-the-camera target pins it
    /// to the screen edge, in the target's direction.
    pub clamp_to_screen_edge: bool,
    /// How far inside the screen edge a clamped marker stays, in reference units.
    pub edge_padding: f32,
    /// While clamped, rotate the element so its up (+Y) points at the target — an
    /// off-screen arrow. On screen it stays upright.
    pub rotate_toward_target: bool,
    /// Hide the marker (and its children) while the target is behind the camera.
    /// Takes precedence over clamping.
    pub hide_when_behind: bool,
}

impl Default for WorldAnchor {
    fn default() -> Self {
        Self {
            target: None,
            offset: Vec3::ZERO,
            clamp_to_screen_edge: false,
            edge_padding: 0.0,
            rotate_toward_target: false,
            hide_when_behind: true,
        }
    }
}

impl Default for RectTransformComponent {
    /// Unity's fresh UI element: centred point anchors and pivot, 100×100.
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
    /// Lay this element out inside `parent` (`min`, `size`), all in reference units.
    /// Returns the element's own axis-aligned rect as `(min, size)`, before any
    /// rotation or scale — Unity's semantics: the anchor region is
    /// `parent.min + anchor * parent.size`, the size is that region plus
    /// `size_delta`, and the pivot sits at the anchors lerped by the pivot plus
    /// `anchored_position`.
    pub fn layout_in(&self, parent_min: Vec2, parent_size: Vec2) -> (Vec2, Vec2) {
        let lo = parent_min + self.anchor_min * parent_size;
        let hi = parent_min + self.anchor_max * parent_size;
        let size = (hi - lo) + self.size_delta;
        let pivot_pos = lo + (hi - lo) * self.pivot + self.anchored_position;
        (pivot_pos - size * self.pivot, size)
    }

    /// The entity references, as JSON pointers under the component (how prefab
    /// overrides recognise them).
    pub const REF_POINTERS: [&'static str; 1] = ["/world_anchor/target"];

    /// Rewrite the world anchor's target through `map` (a reference `map` drops
    /// becomes `None`) — how the reference follows a prefab save / stamp.
    pub fn remap_refs(&mut self, map: &dyn Fn(u32) -> Option<u32>) {
        if let Some(a) = &mut self.world_anchor {
            a.target = a.target.and_then(map);
        }
    }
}
