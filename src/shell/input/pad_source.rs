//! src/shell/input/pad_source.rs — the real [`PadSource`]: gilrs (#471).
//!
//! The only file that names gilrs. It drains gilrs's event queue (which keeps its
//! cached per-pad state current), reads each connected pad into a [`PadSnapshot`],
//! and plays rumble through gilrs's force feedback. gilrs maps every brand to the
//! Xbox layout, so its `South` button is our `A`.

use std::collections::HashMap;

use gilrs::ff::{BaseEffect, BaseEffectType, Effect, EffectBuilder, Repeat, Replay, Ticks};
use gilrs::{Axis, Button, Gamepad, GamepadId, Gilrs};

use super::pads::{PadSnapshot, PadSource};
use crate::core::gamepad::Rumble;

/// gilrs buttons in `core::gamepad::BUTTONS` order.
const BUTTONS: [Button; 17] = [
    Button::South,
    Button::East,
    Button::West,
    Button::North,
    Button::LeftTrigger,
    Button::RightTrigger,
    Button::LeftTrigger2,
    Button::RightTrigger2,
    Button::Select,
    Button::Start,
    Button::Mode,
    Button::LeftThumb,
    Button::RightThumb,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
];

/// gilrs stick axes in `core::gamepad::AXES` order (gilrs sticks are already Y-up).
const STICKS: [Axis; 4] = [
    Axis::LeftStickX,
    Axis::LeftStickY,
    Axis::RightStickX,
    Axis::RightStickY,
];

/// gilrs and the rumble effects currently playing (dropping one stops it).
pub struct GilrsSource {
    gilrs: Gilrs,
    effects: HashMap<usize, Effect>,
}

impl GilrsSource {
    /// Open the OS gamepad backend; `None` (logged) where there is none, e.g. no udev.
    pub fn open() -> Option<Self> {
        match Gilrs::new() {
            Ok(gilrs) => Some(Self {
                gilrs,
                effects: HashMap::new(),
            }),
            Err(err) => {
                log::warn!("gamepads unavailable: {err}");
                None
            }
        }
    }

    fn id_of(&self, device: usize) -> Option<GamepadId> {
        let mut pads = self.gilrs.gamepads();
        pads.find(|(id, _)| usize::from(*id) == device)
            .map(|(id, _)| id)
    }
}

fn snapshot(pad: &Gamepad<'_>) -> PadSnapshot {
    let mut snap = PadSnapshot::default();
    for (slot, button) in snap.buttons.iter_mut().zip(BUTTONS) {
        *slot = pad.is_pressed(button);
    }
    for (slot, axis) in snap.axes.iter_mut().zip(STICKS) {
        *slot = pad.value(axis);
    }
    // Triggers are analogue buttons in gilrs.
    let trigger = |b| pad.button_data(b).map_or(0.0, |d| d.value());
    snap.axes[4] = trigger(Button::LeftTrigger2);
    snap.axes[5] = trigger(Button::RightTrigger2);
    snap
}

/// A 0..1 strength as a motor magnitude.
fn magnitude(strength: f32) -> u16 {
    (strength * f32::from(u16::MAX)) as u16
}

impl PadSource for GilrsSource {
    fn poll(&mut self) -> Vec<(usize, PadSnapshot)> {
        while self.gilrs.next_event().is_some() {}
        self.gilrs
            .gamepads()
            .map(|(id, pad)| (usize::from(id), snapshot(&pad)))
            .collect()
    }

    fn rumble(&mut self, device: usize, rumble: Rumble) {
        // A new request replaces the old one; an all-zero request just stops it.
        self.effects.remove(&device);
        let ms = (rumble.seconds * 1000.0) as u32;
        let Some(id) = self.id_of(device) else { return };
        if ms == 0 || (rumble.low == 0.0 && rumble.high == 0.0) {
            return;
        }
        if !self.gilrs.gamepad(id).is_ff_supported() {
            return;
        }
        let play_for = Ticks::from_ms(ms);
        let motor = |kind| BaseEffect {
            kind,
            scheduling: Replay {
                play_for,
                ..Replay::default()
            },
            ..BaseEffect::default()
        };
        let effect = EffectBuilder::new()
            .add_effect(motor(BaseEffectType::Strong {
                magnitude: magnitude(rumble.low),
            }))
            .add_effect(motor(BaseEffectType::Weak {
                magnitude: magnitude(rumble.high),
            }))
            .gamepads(&[id])
            .repeat(Repeat::For(play_for))
            .finish(&mut self.gilrs);
        match effect.and_then(|effect| effect.play().map(|()| effect)) {
            Ok(effect) => {
                self.effects.insert(device, effect);
            }
            Err(err) => log::warn!("rumble failed on pad {device}: {err}"),
        }
    }
}
