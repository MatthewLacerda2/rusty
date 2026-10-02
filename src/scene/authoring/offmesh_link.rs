//! src/scene/authoring/offmesh_link.rs — Shared OffMeshLink authoring ops (#462).
//!
//! The ONE place the engine mutates an entity's `OffMeshLinkComponent`. The
//! editor's card and the Lua `OffMeshLink.*` setters both route every write
//! through these, so the rules live once: the ends stay finite, and a cost below
//! zero (or NaN) means "use the link's length", stored as `-1`.
//!
//! Allowed deps: components (the component data), navigation (the area limit). Pure.

use glam::Vec3;

use crate::components::OffMeshLinkComponent as Link;

/// Enable or disable the link (Unity's `activated`).
pub fn set_active(l: &mut Link, active: bool) {
    l.active = active;
}

/// Set the start point, local to the entity. A non-finite point is ignored.
pub fn set_start(l: &mut Link, start: Vec3) {
    if start.is_finite() {
        l.start = start;
    }
}

/// Set the end point, local to the entity. A non-finite point is ignored.
pub fn set_end(l: &mut Link, end: Vec3) {
    if end.is_finite() {
        l.end = end;
    }
}

/// Let agents cross end to start as well (Unity's `biDirectional`).
pub fn set_bidirectional(l: &mut Link, both_ways: bool) {
    l.bidirectional = both_ways;
}

/// Set the path cost of crossing the link, in world units. Negative or NaN means
/// "the link's length" (Unity's `costOverride = -1`); infinity is ignored.
pub fn set_cost_override(l: &mut Link, cost: f32) {
    if cost.is_nan() || cost < 0.0 {
        l.cost_override = -1.0;
    } else if cost.is_finite() {
        l.cost_override = cost;
    }
}

/// Set the link's navigation area (#460). An id past the area table's limit is
/// ignored.
pub fn set_area(l: &mut Link, area: i64) {
    if (0..crate::navigation::MAX_AREAS as i64).contains(&area) {
        l.area = area as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setters_keep_the_link_well_formed() {
        let mut l = Link::default();
        set_start(&mut l, Vec3::Y);
        set_start(&mut l, Vec3::NAN);
        assert_eq!(l.start, Vec3::Y, "a non-finite start is ignored");
        set_end(&mut l, Vec3::new(0.0, 4.0, 1.0));
        set_end(&mut l, Vec3::INFINITY);
        assert_eq!(l.end, Vec3::new(0.0, 4.0, 1.0));
        set_cost_override(&mut l, 7.5);
        assert_eq!(l.cost_override, 7.5);
        set_cost_override(&mut l, f32::INFINITY);
        assert_eq!(l.cost_override, 7.5, "infinity is ignored");
        set_cost_override(&mut l, -3.0);
        assert_eq!(l.cost_override, -1.0, "negative means the length");
        set_cost_override(&mut l, f32::NAN);
        assert_eq!(l.cost_override, -1.0);
        set_area(&mut l, 4);
        set_area(&mut l, 40);
        assert_eq!(l.area, 4, "an out-of-range area is ignored");
        set_bidirectional(&mut l, false);
        set_active(&mut l, false);
        assert!(!l.bidirectional && !l.active);
    }
}
