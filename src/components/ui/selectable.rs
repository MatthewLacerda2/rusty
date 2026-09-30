//! src/components/ui/selectable.rs — Selectable: an interactive UI element (#420).
//!
//! Unity's `Selectable`, the base of every widget (Button, Toggle, Slider, … ship as
//! Lua scripts over it, #422). It marks its entity as something the pointer and the
//! keyboard focus can interact with, and gives it visual **states** — `Normal`,
//! `Highlighted` (hovered), `Pressed`, `Selected` (focused) and `Disabled` — shown
//! through a `transition` on its `target_graphic`:
//!
//! - `ColorTint` multiplies the target's colour by the state's colour, fading over
//!   `fade_duration` seconds of unscaled time (so it animates under a paused game);
//! - `SpriteSwap` shows the state's sprite instead of the target Image's texture.
//!
//! `navigation` says how keyboard focus leaves it: `Automatic` picks the nearest
//! selectable in the pressed direction by geometry, `Explicit` follows the
//! `select_on_*` entities, `None` never focuses it. The runtime state (hover, press,
//! the fade) lives in the `ui::EventSystem` resource, never here — this is pure
//! authoring data. The entity references (`target_graphic`, `select_on_*`) are
//! remapped with the entity when a prefab is saved or stamped
//! ([`SelectableComponent::remap_refs`]).

use glam::Vec4;
use serde::{Deserialize, Serialize};

/// How a Selectable shows its state. See the module docs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectableTransition {
    /// No visual change.
    None,
    /// Multiply the target graphic's colour by the state's colour.
    #[default]
    ColorTint,
    /// Swap the target Image's texture for the state's sprite.
    SpriteSwap,
}

/// How keyboard focus moves away from a Selectable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavigationMode {
    /// Never focused by navigation (the pointer can still use it).
    None,
    /// The nearest selectable in the pressed direction, by geometry.
    #[default]
    Automatic,
    /// The `select_on_*` entity for the pressed direction.
    Explicit,
}

/// A visual state of a Selectable, in the order its colours and sprites are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionState {
    Normal,
    Highlighted,
    Pressed,
    Selected,
    Disabled,
}

impl SelectionState {
    /// Every state, in storage order.
    pub const ALL: [SelectionState; 5] = [
        Self::Normal,
        Self::Highlighted,
        Self::Pressed,
        Self::Selected,
        Self::Disabled,
    ];

    /// The state's name as scripts and the snapshot spell it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Highlighted => "Highlighted",
            Self::Pressed => "Pressed",
            Self::Selected => "Selected",
            Self::Disabled => "Disabled",
        }
    }

    /// Parse a state name (case-insensitive).
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|s| s.name().eq_ignore_ascii_case(name))
    }
}

/// An interactive UI element. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectableComponent {
    /// Whether it accepts input. A `CanvasGroup` with `interactable = false` above it
    /// disables it too.
    pub interactable: bool,
    /// How the state shows.
    pub transition: SelectableTransition,
    /// The entity whose Image / Text the transition drives; `None` is this entity.
    pub target_graphic: Option<u32>,
    /// `ColorTint` colours per state (display-space RGBA), in [`SelectionState`]
    /// order: normal, highlighted, pressed, selected, disabled.
    pub colors: [Vec4; 5],
    /// `ColorTint`: seconds (unscaled) a change of state fades over.
    pub fade_duration: f32,
    /// `SpriteSwap` textures per state, in [`SelectionState`] order; `None` (and the
    /// `Normal` slot, always) shows the Image's own texture.
    pub sprites: [Option<String>; 5],
    /// How keyboard focus leaves it.
    pub navigation: NavigationMode,
    /// `Explicit` targets: up, down, left, right.
    pub select_on: [Option<u32>; 4],
}

impl Default for SelectableComponent {
    fn default() -> Self {
        let grey = |v: f32, a: f32| Vec4::new(v, v, v, a);
        Self {
            interactable: true,
            transition: SelectableTransition::ColorTint,
            target_graphic: None,
            // Unity's ColorBlock.defaultColorBlock.
            colors: [
                Vec4::ONE,
                grey(0.961, 1.0),
                grey(0.784, 1.0),
                grey(0.961, 1.0),
                grey(0.784, 0.502),
            ],
            fade_duration: 0.1,
            sprites: Default::default(),
            navigation: NavigationMode::Automatic,
            select_on: [None; 4],
        }
    }
}

impl SelectableComponent {
    /// The JSON pointers (inside the component) of its entity references — how
    /// prefab write-back finds the override leaves holding entity ids.
    pub const REF_POINTERS: [&'static str; 5] = [
        "/target_graphic",
        "/select_on/0",
        "/select_on/1",
        "/select_on/2",
        "/select_on/3",
    ];

    /// The colour for `state`.
    pub fn color(&self, state: SelectionState) -> Vec4 {
        self.colors[state as usize]
    }

    /// The sprite for `state` (`None` for `Normal`: the Image's own texture).
    pub fn sprite(&self, state: SelectionState) -> Option<&str> {
        match state {
            SelectionState::Normal => None,
            _ => self.sprites[state as usize].as_deref(),
        }
    }

    /// The entity the transition drives, given this Selectable's own `id`.
    pub fn target(&self, id: u32) -> u32 {
        self.target_graphic.unwrap_or(id)
    }

    /// Rewrite every entity reference through `map` (a reference `map` drops
    /// becomes `None`) — how the references follow a prefab save / stamp.
    pub fn remap_refs(&mut self, map: &dyn Fn(u32) -> Option<u32>) {
        self.target_graphic = self.target_graphic.and_then(map);
        for r in &mut self.select_on {
            *r = r.and_then(map);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_round_trip_and_normal_never_swaps() {
        for s in SelectionState::ALL {
            assert_eq!(SelectionState::parse(&s.name().to_lowercase()), Some(s));
        }
        let sel = SelectableComponent {
            sprites: std::array::from_fn(|i| Some(format!("s{i}.png"))),
            ..Default::default()
        };
        assert_eq!(sel.sprite(SelectionState::Normal), None);
        assert_eq!(sel.sprite(SelectionState::Pressed), Some("s2.png"));
        assert_eq!(sel.color(SelectionState::Normal), Vec4::ONE);
    }

    #[test]
    fn remap_rewrites_and_drops_references() {
        let mut sel = SelectableComponent {
            target_graphic: Some(3),
            select_on: [Some(4), None, Some(9), None],
            ..Default::default()
        };
        sel.remap_refs(&|id| (id != 9).then_some(id + 10));
        assert_eq!(sel.target_graphic, Some(13));
        assert_eq!(sel.select_on, [Some(14), None, None, None]);
        assert_eq!(sel.target(1), 13);
    }
}
