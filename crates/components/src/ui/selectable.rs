use glam::Vec4;
use rusty_asset::MapRef;
use serde::{Deserialize, Serialize};

use crate::{EntityRefs, RefVisit};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum SelectableTransition {
        None = "None",
        #[default]
        ColorTint = "ColorTint",
        SpriteSwap = "SpriteSwap",
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum NavigationMode {
        None = "None",
        #[default]
        Automatic = "Automatic",
        Explicit = "Explicit",
    }
}

rusty_core::named_enum! {
    /// In the order `colors` and `sprites` hold a state's slot (`state as usize`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SelectionState {
        Normal = "Normal",
        Highlighted = "Highlighted",
        Pressed = "Pressed",
        Selected = "Selected",
        Disabled = "Disabled",
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectableComponent {
    pub interactable: bool,
    pub transition: SelectableTransition,
    pub target_graphic: Option<u32>,
    pub colors: [Vec4; 5],
    pub fade_duration: f32,
    pub sprites: [Option<MapRef>; 5],
    pub navigation: NavigationMode,
    pub select_on: [Option<u32>; 4],
}

impl Default for SelectableComponent {
    fn default() -> Self {
        let grey = |v: f32, a: f32| Vec4::new(v, v, v, a);
        Self {
            interactable: true,
            transition: SelectableTransition::ColorTint,
            target_graphic: None,
            // Unity's `ColorBlock.defaultColorBlock`.
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
    /// The id fields' JSON pointers, matching their serde paths: `Entity::is_ref_pointer` reads
    /// them.
    pub const REF_POINTERS: [&'static str; 5] = [
        "/target_graphic",
        "/select_on/0",
        "/select_on/1",
        "/select_on/2",
        "/select_on/3",
    ];

    pub fn color(&self, state: SelectionState) -> Vec4 {
        self.colors[state as usize]
    }

    pub fn sprite(&self, state: SelectionState) -> Option<&MapRef> {
        match state {
            SelectionState::Normal => None,
            _ => self.sprites[state as usize].as_ref(),
        }
    }

    pub fn target(&self, id: u32) -> u32 {
        self.target_graphic.unwrap_or(id)
    }
}

impl EntityRefs for SelectableComponent {
    fn visit_refs(&mut self, visit: &mut RefVisit<'_>) {
        let [graphic, up, down, left, right] = Self::REF_POINTERS;
        visit(graphic, &mut self.target_graphic);
        for (p, r) in [up, down, left, right].into_iter().zip(&mut self.select_on) {
            visit(p, r);
        }
    }

    fn each_ref(&self, visit: &mut dyn FnMut(&str, u32)) {
        let slots = std::iter::once(&self.target_graphic).chain(&self.select_on);
        for (p, r) in Self::REF_POINTERS.into_iter().zip(slots) {
            r.inspect(|&r| visit(p, r));
        }
    }

    /// Test hook: other crates' tests build it to reach every id slot.
    #[doc(hidden)]
    fn every_ref() -> Self {
        Self::default()
    }
}

impl crate::FirstClass for SelectableComponent {
    const NAME: &'static str = "Selectable";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_never_swaps() {
        let sel = SelectableComponent {
            sprites: std::array::from_fn(|i| Some(format!("s{i}.png").into())),
            ..Default::default()
        };
        assert_eq!(sel.sprite(SelectionState::Normal), None);
        assert_eq!(
            sel.sprite(SelectionState::Pressed),
            Some(&MapRef::from("s2.png"))
        );
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
