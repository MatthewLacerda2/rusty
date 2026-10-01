//! src/scene/authoring/line.rs — Shared Line-authoring ops (#441).
//!
//! The ONE place a `LineComponent`'s own fields are written; its look goes
//! through `authoring::ribbon`. The editor's Line card and the Lua `Line.*`
//! namespace both call these. A non-finite point is dropped (a NaN would poison
//! the whole strip's geometry).

use glam::Vec3;

use crate::components::LineComponent;

/// The most points a line holds — a guard against a runaway script loop.
pub const MAX_POSITIONS: usize = 4096;

/// Replace every point (non-finite ones dropped, capped at [`MAX_POSITIONS`]).
pub fn set_positions(l: &mut LineComponent, positions: &[Vec3]) {
    let finite = positions.iter().copied().filter(|p| p.is_finite());
    l.positions = finite.take(MAX_POSITIONS).collect();
}

/// Move point `index` (0-based); out of range or non-finite is ignored.
pub fn set_position(l: &mut LineComponent, index: usize, position: Vec3) {
    if let (Some(p), true) = (l.positions.get_mut(index), position.is_finite()) {
        *p = position;
    }
}

/// Grow (new points repeat the last one, or the origin) or shrink to `count`.
pub fn set_position_count(l: &mut LineComponent, count: usize) {
    let fill = l.positions.last().copied().unwrap_or(Vec3::ZERO);
    l.positions.resize(count.min(MAX_POSITIONS), fill);
}

/// Whether the points are world space (else the entity's local space).
pub fn set_use_world_space(l: &mut LineComponent, world: bool) {
    l.use_world_space = world;
}

/// Whether the last point joins back to the first.
pub fn set_looping(l: &mut LineComponent, looping: bool) {
    l.looping = looping;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_ops_validate() {
        let mut l = LineComponent::default();
        set_positions(&mut l, &[Vec3::X, Vec3::NAN, Vec3::Y]);
        assert_eq!(l.positions, vec![Vec3::X, Vec3::Y]);
        set_position(&mut l, 1, Vec3::Z);
        set_position(&mut l, 5, Vec3::Z);
        set_position(&mut l, 0, Vec3::INFINITY);
        assert_eq!(l.positions, vec![Vec3::X, Vec3::Z]);
        set_position_count(&mut l, 3);
        assert_eq!(l.positions[2], Vec3::Z);
        set_position_count(&mut l, usize::MAX);
        assert_eq!(l.positions.len(), MAX_POSITIONS);
        set_use_world_space(&mut l, false);
        set_looping(&mut l, true);
        assert!(!l.use_world_space && l.looping);
    }
}
