//! src/ui/events/transition.rs — Selectable states → their graphics (#420).
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
use crate::components::SelectableTransition;
use crate::ecs::World;

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
            let Some(sel) = world.selectable(id).map(|s| s.clone()) else {
                continue;
            };
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
            let sprite = sprite.map(str::to_string);
            if let Some(mut image) = world.image_mut(target) {
                image.state_tint = tint;
                if image.override_texture != sprite {
                    image.override_texture = sprite;
                }
            }
            if let Some(mut shape) = world.shape_mut(target) {
                shape.state_tint = tint;
            }
            if let Some(mut text) = world.text_mut(target) {
                text.state_tint = tint;
            }
        }
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
