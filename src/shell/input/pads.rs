//! src/shell/input/pads.rs — the gamepad pump (#471): pad snapshots in, `InputState` out.
//!
//! Once per frame, before the sim ticks, the shell asks a [`PadSource`] for every
//! connected pad's raw state and [`PadPump::pump`] turns it into the sim's model:
//! each device gets a stable slot (the lowest free, so the first pad plugged in is
//! pad 0), its buttons are written as keys through the [`Keymap`] (so they rebind like
//! keys), and its sticks and triggers are written as axes after the dead zones.
//! Only *changes* are written: a pad at rest never releases a keyboard key bound to
//! the same logical name. Without input (editor, Game view unfocused) every pad reads
//! as at rest. Rumble requests recorded by the sim are handed back to the source.
//!
//! The source is a trait so tests drive the pump with a fake: no real pad exists in
//! CI. The real one is [`GilrsSource`](super::pad_source::GilrsSource).

use crate::core::gamepad::{
    axial_dead_zone, pad_name, radial_dead_zone, DeadZones, Rumble, AXES, BUTTONS, MAX_PADS,
};
use crate::core::input::InputState;
use crate::core::keymap::Keymap;

/// One pad's raw state: buttons in [`BUTTONS`] order, axes in [`AXES`] order (sticks
/// Y-up in `-1..1`, triggers `0..1`), before any dead zone.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadSnapshot {
    pub buttons: [bool; BUTTONS.len()],
    pub axes: [f32; AXES.len()],
}

impl PadSnapshot {
    /// The snapshot with the dead zones applied: sticks radially, triggers per axis.
    fn filtered(&self, zones: DeadZones) -> Self {
        let [lx, ly, rx, ry, lt, rt] = self.axes;
        let (lx, ly) = radial_dead_zone(lx, ly, zones.stick);
        let (rx, ry) = radial_dead_zone(rx, ry, zones.stick);
        let (lt, rt) = (
            axial_dead_zone(lt, zones.trigger),
            axial_dead_zone(rt, zones.trigger),
        );
        Self {
            buttons: self.buttons,
            axes: [lx, ly, rx, ry, lt, rt],
        }
    }
}

/// Where pad state comes from: the OS (gilrs) in a window, a fake in tests.
pub trait PadSource {
    /// Every connected pad as `(device id, raw state)`. Device ids are the source's
    /// own and stable while the pad stays connected.
    fn poll(&mut self) -> Vec<(usize, PadSnapshot)>;
    /// Play (or, for an all-zero request, stop) rumble on a device.
    fn rumble(&mut self, device: usize, rumble: Rumble);
}

/// A slot's device and what the pump last wrote for it.
#[derive(Clone, Copy, Debug)]
struct Slot {
    device: usize,
    written: PadSnapshot,
}

/// Device → slot assignment and the last state written per slot.
#[derive(Debug, Default)]
pub struct PadPump {
    slots: [Option<Slot>; MAX_PADS],
}

impl PadPump {
    /// One frame: (dis)connect, write each pad's changes, forward rumble.
    pub fn pump(
        &mut self,
        source: &mut dyn PadSource,
        input: &mut InputState,
        keymap: &Keymap,
        has_input: bool,
    ) {
        let mut pads = source.poll();
        pads.sort_by_key(|(device, _)| *device);
        self.disconnect_missing(&pads, input, keymap);
        let zones = input.pads.dead_zones;
        for (device, raw) in &pads {
            let Some(index) = self.slot_of(*device, input) else {
                continue; // every slot taken
            };
            let target = if has_input {
                raw.filtered(zones)
            } else {
                PadSnapshot::default()
            };
            if let Some(slot) = self.slots[index].as_mut() {
                write_changes(index, &slot.written, &target, input, keymap);
                slot.written = target;
            }
        }
        for (pad, rumble) in input.pads.take_rumble() {
            if let Some(Some(slot)) = self.slots.get(pad) {
                source.rumble(slot.device, rumble);
            }
        }
    }

    /// Release and zero every slot whose device is gone, and free it.
    fn disconnect_missing(
        &mut self,
        pads: &[(usize, PadSnapshot)],
        input: &mut InputState,
        keymap: &Keymap,
    ) {
        for (index, entry) in self.slots.iter_mut().enumerate() {
            let Some(slot) = entry else { continue };
            if pads.iter().any(|(device, _)| *device == slot.device) {
                continue;
            }
            write_changes(index, &slot.written, &PadSnapshot::default(), input, keymap);
            input.pads.set_connected(index, false);
            *entry = None;
        }
    }

    /// The device's slot, assigning the lowest free one on first sight.
    fn slot_of(&mut self, device: usize, input: &mut InputState) -> Option<usize> {
        let held = |s: &Option<Slot>| s.is_some_and(|s| s.device == device);
        if let Some(index) = self.slots.iter().position(held) {
            return Some(index);
        }
        let index = self.slots.iter().position(Option::is_none)?;
        self.slots[index] = Some(Slot {
            device,
            written: PadSnapshot::default(),
        });
        input.pads.set_connected(index, true);
        Some(index)
    }
}

/// Write what differs between `from` and `to` on pad `index`: buttons as keys
/// (through the keymap), axes as axes.
fn write_changes(
    index: usize,
    from: &PadSnapshot,
    to: &PadSnapshot,
    input: &mut InputState,
    keymap: &Keymap,
) {
    for (i, suffix) in BUTTONS.iter().enumerate() {
        if from.buttons[i] != to.buttons[i] {
            let logical = keymap.resolve(&pad_name(index, suffix));
            input.set_key_state(&logical, to.buttons[i]);
        }
    }
    for (i, suffix) in AXES.iter().enumerate() {
        if from.axes[i] != to.axes[i] {
            input.set_axis(&pad_name(index, suffix), to.axes[i]);
        }
    }
}

#[cfg(test)]
#[path = "pads_tests.rs"]
mod pads_tests;
