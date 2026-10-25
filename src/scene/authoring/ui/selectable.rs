//! src/scene/authoring/ui/selectable.rs — Shared Selectable-authoring ops (#420).
//!
//! The ONE place the engine mutates an entity's first-class `SelectableComponent`.
//! The editor's Selectable card and the Lua `Selectable.*` namespace both route every
//! write through these; colours are kept in `[0, 1]` and the fade non-negative.
//!
//! Pure.

use glam::Vec4;

use crate::asset::MapRef;
use crate::components::{
    NavigationMode, SelectableComponent, SelectableTransition, SelectionState,
};
use crate::scene::authoring::sanitize::{non_negative, unit_color};

/// The navigation directions, in `select_on` order.
pub const DIRECTIONS: [&str; 4] = ["Up", "Down", "Left", "Right"];

/// Parse a direction name (case-insensitive) into its `select_on` index.
pub fn direction_index(name: &str) -> Option<usize> {
    DIRECTIONS.iter().position(|d| d.eq_ignore_ascii_case(name))
}

/// Set whether it accepts input.
pub fn set_interactable(s: &mut SelectableComponent, interactable: bool) {
    s.interactable = interactable;
}

/// Set how its state shows.
pub fn set_transition(s: &mut SelectableComponent, transition: SelectableTransition) {
    s.transition = transition;
}

/// Set the entity the transition drives (`None`: this entity).
pub fn set_target_graphic(s: &mut SelectableComponent, target: Option<u32>) {
    s.target_graphic = target;
}

/// Set `state`'s tint colour, each channel clamped to `[0, 1]`.
pub fn set_color(s: &mut SelectableComponent, state: SelectionState, color: Vec4) {
    s.colors[state as usize] = unit_color(color);
}

/// Set the fade duration in seconds, floored at 0 (instant).
pub fn set_fade_duration(s: &mut SelectableComponent, seconds: f32) {
    s.fade_duration = non_negative(seconds);
}

/// Set `state`'s swap sprite (`None`: the Image's own texture). `Normal` has no slot
/// of its own — it always shows the Image's texture — so setting it is a no-op.
pub fn set_sprite(s: &mut SelectableComponent, state: SelectionState, sprite: Option<String>) {
    if state != SelectionState::Normal {
        s.sprites[state as usize] = sprite.filter(|p| !p.is_empty()).map(MapRef::from);
    }
}

/// Set how keyboard focus leaves it.
pub fn set_navigation(s: &mut SelectableComponent, mode: NavigationMode) {
    s.navigation = mode;
}

/// Set the `Explicit` target for direction `dir` (a [`DIRECTIONS`] index).
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
