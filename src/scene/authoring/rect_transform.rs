//! src/scene/authoring/rect_transform.rs — Shared RectTransform-authoring ops (#417).
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `RectTransformComponent` field by field. The editor's RectTransform card and the
//! Lua `RectTransform.*` namespace both route every write through these. Anchors are
//! kept in `[0, 1]` with `anchor_min <= anchor_max` per axis (the setter that moves
//! one anchor past the other drags the other along, as Unity's inspector does);
//! the pivot, position and size are free (a pivot outside 0..1 is legal in Unity).
//!
//! Allowed deps: components (the `RectTransformComponent` data). Pure.

use glam::Vec2;

use crate::components::RectTransformComponent;

/// Set the lower-left anchor, clamped to `[0, 1]`; `anchor_max` is raised to stay ≥ it.
pub fn set_anchor_min(r: &mut RectTransformComponent, anchor: Vec2) {
    r.anchor_min = anchor.clamp(Vec2::ZERO, Vec2::ONE);
    r.anchor_max = r.anchor_max.max(r.anchor_min);
}

/// Set the upper-right anchor, clamped to `[0, 1]`; `anchor_min` is lowered to stay ≤ it.
pub fn set_anchor_max(r: &mut RectTransformComponent, anchor: Vec2) {
    r.anchor_max = anchor.clamp(Vec2::ZERO, Vec2::ONE);
    r.anchor_min = r.anchor_min.min(r.anchor_max);
}

/// Set the pivot (a fraction of the element's own rect).
pub fn set_pivot(r: &mut RectTransformComponent, pivot: Vec2) {
    r.pivot = pivot;
}

/// Set the pivot's offset from the anchor reference point.
pub fn set_anchored_position(r: &mut RectTransformComponent, position: Vec2) {
    r.anchored_position = position;
}

/// Set the size relative to the anchor region.
pub fn set_size_delta(r: &mut RectTransformComponent, size_delta: Vec2) {
    r.size_delta = size_delta;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_clamp_and_stay_ordered() {
        let mut r = RectTransformComponent::default();
        set_anchor_min(&mut r, Vec2::new(0.8, -1.0));
        assert_eq!(r.anchor_min, Vec2::new(0.8, 0.0));
        assert_eq!(r.anchor_max, Vec2::new(0.8, 0.5));
        set_anchor_max(&mut r, Vec2::new(0.2, 2.0));
        assert_eq!(r.anchor_max, Vec2::new(0.2, 1.0));
        assert_eq!(r.anchor_min, Vec2::new(0.2, 0.0));
    }

    #[test]
    fn free_fields_write_through() {
        let mut r = RectTransformComponent::default();
        set_pivot(&mut r, Vec2::new(0.0, 1.5));
        set_anchored_position(&mut r, Vec2::new(-10.0, 20.0));
        set_size_delta(&mut r, Vec2::new(-40.0, 30.0));
        assert_eq!(r.pivot, Vec2::new(0.0, 1.5));
        assert_eq!(r.anchored_position, Vec2::new(-10.0, 20.0));
        assert_eq!(r.size_delta, Vec2::new(-40.0, 30.0));
    }
}
