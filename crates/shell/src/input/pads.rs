use rusty_core::input::gamepad::{
    axial_dead_zone, pad_name, radial_dead_zone, DeadZones, Rumble, AXES, BUTTONS, MAX_PADS,
};
use rusty_core::input::keymap::Keymap;
use rusty_core::input::InputState;

/// Raw, before any dead zone: sticks Y-up in `-1..1`, triggers `0..1`.
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
    /// Every connected pad as `(device id, raw state)`; an id is stable while its pad stays
    /// connected.
    fn poll(&mut self) -> Vec<(usize, PadSnapshot)>;
    /// Play (or, for an all-zero request, stop) rumble on a device.
    fn rumble(&mut self, device: usize, rumble: Rumble);
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    device: usize,
    written: PadSnapshot,
}

#[derive(Debug, Default)]
pub struct PadPump {
    slots: [Option<Slot>; MAX_PADS],
}

impl PadPump {
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
