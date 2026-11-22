use glam::Vec3;

use rusty_components::OffMeshLinkComponent as Link;
use rusty_scene::rules::sanitize::{finite3_or, finite_or};

/// Enable or disable the link (Unity's `activated`).
pub fn set_active(l: &mut Link, active: bool) {
    l.active = active;
}

pub fn set_start(l: &mut Link, start: Vec3) {
    l.start = finite3_or(start, l.start);
}

pub fn set_end(l: &mut Link, end: Vec3) {
    l.end = finite3_or(end, l.end);
}

/// Let agents cross end to start as well (Unity's `biDirectional`).
pub fn set_bidirectional(l: &mut Link, both_ways: bool) {
    l.bidirectional = both_ways;
}

/// Set the path cost of crossing the link, in world units. Negative or NaN means
/// "the link's length" (Unity's `costOverride = -1`); infinity is ignored.
pub fn set_cost_override(l: &mut Link, cost: f32) {
    l.cost_override = if cost >= 0.0 {
        finite_or(cost, l.cost_override)
    } else {
        -1.0
    };
}

pub fn set_area(l: &mut Link, area: i64) {
    if (0..rusty_scene::nav_settings::MAX_AREAS as i64).contains(&area) {
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
        set_start(&mut l, Vec3::new(5.0, f32::NAN, 9.0));
        assert_eq!(l.start, Vec3::Y, "a partly-NaN start is refused whole");
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
