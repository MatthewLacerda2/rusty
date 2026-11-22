use glam::Vec4;

use rusty_asset::MapRef;
use rusty_components::{NavigationMode, SelectableComponent, SelectableTransition, SelectionState};
use rusty_scene::rules::sanitize::{non_negative, unit_color};

/// `select_on`'s index order, shared with `ui::events::focus` and
/// `SelectableComponent::REF_POINTERS`.
pub const DIRECTIONS: [&str; 4] = ["Up", "Down", "Left", "Right"];

pub fn direction_index(name: &str) -> Option<usize> {
    DIRECTIONS.iter().position(|d| d.eq_ignore_ascii_case(name))
}

pub fn set_interactable(s: &mut SelectableComponent, interactable: bool) {
    s.interactable = interactable;
}

pub fn set_transition(s: &mut SelectableComponent, transition: SelectableTransition) {
    s.transition = transition;
}

pub fn set_target_graphic(s: &mut SelectableComponent, target: Option<u32>) {
    s.target_graphic = target;
}

pub fn set_color(s: &mut SelectableComponent, state: SelectionState, color: Vec4) {
    s.colors[state as usize] = unit_color(color);
}

pub fn set_fade_duration(s: &mut SelectableComponent, seconds: f32) {
    s.fade_duration = non_negative(seconds);
}

/// `Normal` has no slot of its own, so setting it is a no-op.
pub fn set_sprite(s: &mut SelectableComponent, state: SelectionState, sprite: Option<String>) {
    if state != SelectionState::Normal {
        s.sprites[state as usize] = sprite.filter(|p| !p.is_empty()).map(MapRef::from);
    }
}

pub fn set_navigation(s: &mut SelectableComponent, mode: NavigationMode) {
    s.navigation = mode;
}

pub fn set_select_on(s: &mut SelectableComponent, dir: usize, target: Option<u32>) {
    if let Some(slot) = s.select_on.get_mut(dir) {
        *slot = target;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_write_through_and_clamp() {
        let mut s = SelectableComponent::default();
        set_color(
            &mut s,
            SelectionState::Pressed,
            Vec4::new(2.0, -1.0, 0.5, 1.0),
        );
        assert_eq!(s.colors[2], Vec4::new(1.0, 0.0, 0.5, 1.0));
        set_fade_duration(&mut s, -3.0);
        assert_eq!(s.fade_duration, 0.0);
        set_sprite(&mut s, SelectionState::Normal, Some("n.png".into()));
        set_sprite(&mut s, SelectionState::Disabled, Some("d.png".into()));
        assert_eq!(s.sprites[0], None);
        assert_eq!(s.sprites[4], Some("d.png".into()));
        set_select_on(
            &mut s,
            direction_index("right").expect("a direction"),
            Some(7),
        );
        set_select_on(&mut s, 9, Some(8));
        assert_eq!(s.select_on, [None, None, None, Some(7)]);
    }
}
