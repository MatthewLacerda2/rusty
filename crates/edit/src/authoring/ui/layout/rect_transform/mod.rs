//! Anchors stay in `[0, 1]` with `anchor_min <= anchor_max` per axis (moving one past the other
//! drags the other along, as Unity's inspector does); a pivot outside 0..1 is legal, as in Unity.

use glam::{Vec2, Vec3};

use rusty_components::{RectTransformComponent, WorldAnchor};
use rusty_scene::rules::sanitize::{finite2, finite2_or, finite3, non_negative};

mod preset;

pub use preset::{apply_anchor_preset, AnchorPreset, AxisPreset};

pub fn set_anchor_min(r: &mut RectTransformComponent, anchor: Vec2) {
    r.anchor_min = finite2(anchor).clamp(Vec2::ZERO, Vec2::ONE);
    r.anchor_max = r.anchor_max.max(r.anchor_min);
}

pub fn set_anchor_max(r: &mut RectTransformComponent, anchor: Vec2) {
    r.anchor_max = finite2(anchor).clamp(Vec2::ZERO, Vec2::ONE);
    r.anchor_min = r.anchor_min.min(r.anchor_max);
}

pub fn set_pivot(r: &mut RectTransformComponent, pivot: Vec2) {
    r.pivot = finite2_or(pivot, r.pivot);
}

pub fn set_anchored_position(r: &mut RectTransformComponent, position: Vec2) {
    r.anchored_position = finite2(position);
}

pub fn set_size_delta(r: &mut RectTransformComponent, size_delta: Vec2) {
    r.size_delta = finite2(size_delta);
}

/// One whose target was destroyed stays hidden until it is given a target.
pub fn set_world_anchor(r: &mut RectTransformComponent, target: Option<u32>, offset: Vec3) {
    let anchor = r.world_anchor.get_or_insert_with(WorldAnchor::default);
    if anchor.target != target {
        anchor.target_lost = false;
    }
    anchor.target = target;
    anchor.offset = finite3(offset);
}

pub fn clear_world_anchor(r: &mut RectTransformComponent) {
    r.world_anchor = None;
}

pub fn set_anchor_clamp(r: &mut RectTransformComponent, clamp: bool, padding: f32) {
    if let Some(a) = &mut r.world_anchor {
        a.clamp_to_screen_edge = clamp;
        a.edge_padding = non_negative(padding);
    }
}

pub fn set_anchor_rotate(r: &mut RectTransformComponent, rotate: bool) {
    if let Some(a) = &mut r.world_anchor {
        a.rotate_toward_target = rotate;
    }
}

pub fn set_anchor_hide_when_behind(r: &mut RectTransformComponent, hide: bool) {
    if let Some(a) = &mut r.world_anchor {
        a.hide_when_behind = hide;
    }
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

    #[test]
    fn world_anchor_is_set_whole_and_tuned_in_place() {
        let mut r = RectTransformComponent::default();
        set_anchor_clamp(&mut r, true, 5.0);
        assert_eq!(r.world_anchor, None, "options need a marker");
        set_world_anchor(&mut r, Some(4), Vec3::Y);
        set_anchor_clamp(&mut r, true, -5.0);
        set_anchor_rotate(&mut r, true);
        set_anchor_hide_when_behind(&mut r, false);
        set_world_anchor(&mut r, None, Vec3::X);
        let a = r.world_anchor.clone().expect("a marker");
        assert_eq!((a.target, a.offset), (None, Vec3::X));
        assert!(a.clamp_to_screen_edge && a.rotate_toward_target && !a.hide_when_behind);
        assert_eq!(a.edge_padding, 0.0);
        clear_world_anchor(&mut r);
        assert_eq!(r.world_anchor, None);
    }

    #[test]
    fn a_lost_marker_stays_lost_until_it_gets_a_target() {
        let mut r = RectTransformComponent::default();
        set_world_anchor(&mut r, None, Vec3::Y);
        r.world_anchor.as_mut().unwrap().target_lost = true;
        set_world_anchor(&mut r, None, Vec3::X);
        assert!(r.world_anchor.as_ref().unwrap().target_lost);
        set_world_anchor(&mut r, Some(7), Vec3::X);
        assert!(!r.world_anchor.as_ref().unwrap().target_lost);
    }
}
