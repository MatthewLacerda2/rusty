use glam::Vec3;

use rusty_components::LineComponent;
use rusty_scene::rules::sanitize::finite3_or;

/// Points: a guard against a runaway script loop.
pub const MAX_POSITIONS: usize = 4096;

// lint: allow(sanitize) the `is_finite` filter drops every non-finite point
pub fn set_positions(l: &mut LineComponent, positions: &[Vec3]) {
    let finite = positions.iter().copied().filter(|p| p.is_finite());
    l.positions = finite.take(MAX_POSITIONS).collect();
}

pub fn set_position(l: &mut LineComponent, index: usize, position: Vec3) {
    if let Some(p) = l.positions.get_mut(index) {
        *p = finite3_or(position, *p);
    }
}

pub fn set_position_count(l: &mut LineComponent, count: usize) {
    let fill = l.positions.last().copied().unwrap_or(Vec3::ZERO);
    l.positions.resize(count.min(MAX_POSITIONS), fill);
}

pub fn set_use_world_space(l: &mut LineComponent, world: bool) {
    l.use_world_space = world;
}

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
