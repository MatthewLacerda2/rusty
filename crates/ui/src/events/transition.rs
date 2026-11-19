//! crates/ui/src/events/transition.rs — Selectable states → their graphics (#420).
//!
//! Each `LateUpdate`, after the layout pass, every Selectable's state
//! ([`EventSystem::state_of`]) is shown on its target graphic — the Selectable's
//! `target_graphic`, or its own entity:
//!
//! - `ColorTint` fades the target's `state_tint` (on its Image, Shape and Text) to the
//!   state's colour, linearly over `fade_duration` seconds of **unscaled** time, so a
//!   menu under `Time.SetTimeScale(0)` still animates. A Selectable seen for the first
//!   time starts at its colour, without a fade.
//! - `SpriteSwap` sets the target Image's `override_texture` to the state's sprite.
//! - `None` leaves the target untinted and unswapped.
//!
//! Both slots are runtime-only (never saved) and the renderer multiplies / prefers
//! them, like Unity's `CanvasRenderer` colour and `Image.overrideSprite`.

use glam::Vec4;

use super::EventSystem;
use rusty_asset::MapRef;
use rusty_components::SelectableTransition;
use rusty_ecs::World;

/// Put `tint` on `target`'s Image, Shape and Text, and `image`'s sprite on its
/// Image when [`EventSystem::target_state`] found it differs. Each is written only
/// when it shows something else: a write is a change on the World's change log,
/// which the renderer reads to keep a still canvas's mesh (#2416).
fn show(world: &mut World, target: u32, tint: Vec4, image: Option<Option<MapRef>>) {
    if let Some(sprite) = image {
        if let Some(mut image) = world.image_mut(target) {
            (image.state_tint, image.override_texture) = (tint, sprite);
        }
    }
    if world.shape(target).is_some_and(|s| s.state_tint != tint) {
        if let Some(mut shape) = world.shape_mut(target) {
            shape.state_tint = tint;
        }
    }
    if world.text(target).is_some_and(|t| t.state_tint != tint) {
        if let Some(mut text) = world.text_mut(target) {
            text.state_tint = tint;
        }
    }
}

/// A `ColorTint` fade in flight.
#[derive(Clone, Copy, Debug)]
pub(super) struct Fade {
    from: Vec4,
    to: Vec4,
    elapsed: f32,
    current: Vec4,
}

impl EventSystem {
    /// Show every Selectable's state on its target graphic; `dt` is this tick's
    /// unscaled delta time.
    pub fn apply_transitions(&mut self, world: &mut World, dt: f32) {
        let mut ids = world.ids_with_selectable();
        ids.sort_unstable();
        self.fades.retain(|id, _| ids.binary_search(id).is_ok());
        for id in ids {
            if let Some((target, tint, image)) = self.target_state(world, id, dt) {
                show(world, target, tint, image);
            }
        }
    }

    /// Selectable `id`'s target, the tint its state shows, and the sprite to put on
    /// the target's Image when that Image shows another tint or sprite (`None`: no
    /// Image write). The sprite is compared by reference and cloned only for a
    /// write, so a settled Selectable allocates nothing.
    fn target_state(
        &mut self,
        world: &World,
        id: u32,
        dt: f32,
    ) -> Option<(u32, Vec4, Option<Option<MapRef>>)> {
        let sel = world.selectable(id)?;
        let state = self.state_of(world, id);
        let (tint, sprite) = match sel.transition {
            SelectableTransition::ColorTint => {
                let to = sel.color(state);
                (self.fade(id, to, sel.fade_duration, dt), None)
            }
            SelectableTransition::SpriteSwap => (Vec4::ONE, sel.sprite(state)),
            SelectableTransition::None => (Vec4::ONE, None),
        };
        let target = sel.target(id);
        let image = world
            .image(target)
            .filter(|i| i.state_tint != tint || i.override_texture.as_ref() != sprite)
            .map(|_| sprite.cloned());
        Some((target, tint, image))
    }

    /// Advance `id`'s fade toward `to` by `dt`; returns the colour to show now.
    fn fade(&mut self, id: u32, to: Vec4, duration: f32, dt: f32) -> Vec4 {
        let fade = self.fades.entry(id).or_insert(Fade {
            from: to,
            to,
            elapsed: duration,
            current: to,
        });
        if fade.to != to {
            *fade = Fade {
                from: fade.current,
                to,
                elapsed: 0.0,
                current: fade.current,
            };
        }
        fade.elapsed += dt;
        let t = if duration > 0.0 {
            (fade.elapsed / duration).min(1.0)
        } else {
            1.0
        };
        fade.current = fade.from.lerp(fade.to, t);
        fade.current
    }
}
