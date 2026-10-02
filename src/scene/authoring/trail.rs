//! src/scene/authoring/trail.rs — Shared Trail-authoring ops (#441).
//!
//! The ONE place a `TrailComponent`'s own fields are written; its look goes
//! through `authoring::ribbon`. The editor's Trail card and the Lua `Trail.*`
//! namespace both call these.
//!
//! Pure.

use super::ribbon::non_negative;
use crate::components::TrailComponent;

/// Start or stop recording new points (the existing trail still ages out).
pub fn set_emitting(t: &mut TrailComponent, emitting: bool) {
    t.emitting = emitting;
}

/// Set how long (seconds) a point lives, floored at 0.
pub fn set_time(t: &mut TrailComponent, seconds: f32) {
    t.time = non_negative(seconds);
}

/// Set the distance the head travels before a point is committed, floored at 0.
pub fn set_min_vertex_distance(t: &mut TrailComponent, distance: f32) {
    t.min_vertex_distance = non_negative(distance);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trail_ops_clamp() {
        let mut t = TrailComponent::default();
        set_emitting(&mut t, false);
        set_time(&mut t, -1.0);
        set_min_vertex_distance(&mut t, f32::NAN);
        assert!(!t.emitting);
        assert_eq!((t.time, t.min_vertex_distance), (0.0, 0.0));
    }
}
