//! The pad pump against a fake source: slots, edges, dead zones, keymap, rumble.

use super::*;

/// Pads the test sets directly, and the rumble the pump forwarded.
#[derive(Default)]
struct FakePads {
    pads: Vec<(usize, PadSnapshot)>,
    rumbled: Vec<(usize, Rumble)>,
}

impl PadSource for FakePads {
    fn poll(&mut self) -> Vec<(usize, PadSnapshot)> {
        self.pads.clone()
    }
    fn rumble(&mut self, device: usize, rumble: Rumble) {
        self.rumbled.push((device, rumble));
    }
}

/// A snapshot with button `A` held and the left stick at `(x, y)`.
fn pressing_a(x: f32, y: f32) -> PadSnapshot {
    let mut snap = PadSnapshot::default();
    snap.buttons[0] = true;
    snap.axes[0] = x;
    snap.axes[1] = y;
    snap
}

fn frame(pump: &mut PadPump, src: &mut FakePads, input: &mut InputState, keymap: &Keymap) {
    pump.pump(src, input, keymap, true);
    input.begin_tick();
}

#[test]
fn a_pad_connects_and_its_button_edge_fires_once() {
    let (mut pump, mut input, keymap) = (PadPump::default(), InputState::new(), Keymap::new());
    let mut src = FakePads::default();
    src.pads.push((7, pressing_a(0.0, 1.0)));
    frame(&mut pump, &mut src, &mut input, &keymap);
    assert!(input.pads.is_connected(0), "the first device is pad 0");
    assert!(input.get_key_down("PadA") && input.is_key_down("PADA"));
    assert_eq!(input.axis("PadLeftY"), 1.0, "Y-up, full travel");
    frame(&mut pump, &mut src, &mut input, &keymap);
    assert!(!input.get_key_down("PADA"), "held: no second edge");
    assert!(input.is_key_down("PADA"));
}

#[test]
fn sticks_pass_through_the_dead_zone() {
    let (mut pump, mut input, keymap) = (PadPump::default(), InputState::new(), Keymap::new());
    let mut src = FakePads::default();
    src.pads.push((0, pressing_a(0.1, 0.0)));
    frame(&mut pump, &mut src, &mut input, &keymap);
    assert_eq!(
        input.axis("PADLEFTX"),
        0.0,
        "drift inside the zone reads zero"
    );
    input.pads.dead_zones = DeadZones::clamped(0.0, 0.0);
    frame(&mut pump, &mut src, &mut input, &keymap);
    assert!((input.axis("PADLEFTX") - 0.1).abs() < 1e-6);
}

#[test]
fn keymap_rebinding_covers_pad_buttons() {
    let (mut pump, mut input, mut keymap) = (PadPump::default(), InputState::new(), Keymap::new());
    keymap.bind("PADA", "SPACE");
    let mut src = FakePads::default();
    src.pads.push((0, pressing_a(0.0, 0.0)));
    frame(&mut pump, &mut src, &mut input, &keymap);
    assert!(input.get_key_down("SPACE"));
    assert!(!input.is_key_down("PADA"));
}

#[test]
fn a_resting_pad_never_releases_a_shared_keyboard_key() {
    let (mut pump, mut input, mut keymap) = (PadPump::default(), InputState::new(), Keymap::new());
    keymap.bind("PADA", "SPACE");
    input.press("SPACE"); // the keyboard holds it
    let mut src = FakePads::default();
    src.pads.push((0, PadSnapshot::default()));
    frame(&mut pump, &mut src, &mut input, &keymap);
    frame(&mut pump, &mut src, &mut input, &keymap);
    assert!(input.is_key_down("SPACE"));
}

#[test]
fn unplugging_releases_zeroes_and_frees_the_slot() {
    let (mut pump, mut input, keymap) = (PadPump::default(), InputState::new(), Keymap::new());
    let mut src = FakePads {
        pads: vec![(3, pressing_a(1.0, 0.0)), (9, pressing_a(0.0, 0.0))],
        ..FakePads::default()
    };
    frame(&mut pump, &mut src, &mut input, &keymap);
    assert!(input.is_key_down("PADA") && input.is_key_down("PAD1A"));
    src.pads.remove(0);
    frame(&mut pump, &mut src, &mut input, &keymap);
    assert!(input.get_key_up("PADA") && !input.pads.is_connected(0));
    assert_eq!(input.axis("PADLEFTX"), 0.0);
    assert!(input.is_key_down("PAD1A"), "the other pad keeps its slot");
    src.pads.push((12, PadSnapshot::default()));
    frame(&mut pump, &mut src, &mut input, &keymap);
    assert!(
        input.pads.is_connected(0),
        "a new pad takes the lowest free slot"
    );
}

#[test]
fn without_input_every_pad_reads_at_rest() {
    let (mut pump, mut input, keymap) = (PadPump::default(), InputState::new(), Keymap::new());
    let mut src = FakePads::default();
    src.pads.push((0, pressing_a(1.0, 0.0)));
    frame(&mut pump, &mut src, &mut input, &keymap);
    pump.pump(&mut src, &mut input, &keymap, false);
    input.begin_tick();
    assert!(input.get_key_up("PADA"));
    assert_eq!(input.axis("PADLEFTX"), 0.0);
    assert!(input.pads.is_connected(0), "still plugged in");
}

#[test]
fn rumble_reaches_the_device_behind_the_slot() {
    let (mut pump, mut input, keymap) = (PadPump::default(), InputState::new(), Keymap::new());
    let mut src = FakePads::default();
    src.pads.push((42, PadSnapshot::default()));
    frame(&mut pump, &mut src, &mut input, &keymap);
    let rumble = Rumble::new(1.0, 0.25, 0.5);
    input.pads.request_rumble(0, rumble);
    input.pads.request_rumble(1, rumble); // no pad there: dropped
    frame(&mut pump, &mut src, &mut input, &keymap);
    assert_eq!(src.rumbled, vec![(42, rumble)]);
}
