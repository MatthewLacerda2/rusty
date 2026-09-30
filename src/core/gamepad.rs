//! src/core/gamepad.rs — the gamepad half of the input model (#471).
//!
//! A pad is not a new kind of input: its **buttons are more key names** and its
//! **sticks and triggers are axes**, so it reads through the same
//! [`InputState`](crate::core::input::InputState) as the keyboard and mouse. This
//! module owns the pad's naming, the dead-zone maths, and the sim-side records a pad
//! needs beyond keys and axes: which slots are connected, and rumble requests.
//!
//! **Names.** Pad 0 is unnumbered (`"PADA"`, `"PADLEFTX"`); pad `n ≥ 1` inserts its
//! index (`"PAD1A"`, `"PAD1LEFTX"`). Buttons follow the Xbox layout (`A` is the south
//! face button on every brand). Sticks are **Y-up**: pushing a stick forward gives
//! `+1`. Triggers run `0` (released) to `1` (fully pulled).
//!
//! **Dead zones** are applied at the platform source, like the keymap: injected axes
//! (`Input.SetAxis`) are logical and reach the sim exactly as written. Sticks use a
//! *radial* dead zone (the pair's magnitude, rescaled so output starts at 0 past the
//! edge) — the per-axis kind snaps aim to the cardinal directions.
//!
//! **Rumble** is a request the sim records and the platform plays; headless it stays a
//! record, readable back by tests and bots.

/// Button name suffixes. `LT`/`RT` are the
/// triggers read as buttons (pulled past the driver's threshold); their analogue
/// value is the `LEFTTRIGGER`/`RIGHTTRIGGER` axis.
pub const BUTTONS: &[&str] = &[
    "A", "B", "X", "Y", "LB", "RB", "LT", "RT", "BACK", "START", "GUIDE", "LS", "RS", "UP", "DOWN",
    "LEFT", "RIGHT",
];

/// Axis name suffixes: the two sticks (Y-up), then the two triggers (0..1).
pub const AXES: &[&str] = &[
    "LEFTX",
    "LEFTY",
    "RIGHTX",
    "RIGHTY",
    "LEFTTRIGGER",
    "RIGHTTRIGGER",
];

/// How many pads the platform tracks at once; a fifth pad is ignored until a slot frees.
pub const MAX_PADS: usize = 4;

/// The name of a [`BUTTONS`] or [`AXES`] `suffix` on `pad`: `"PADA"` on pad 0,
/// `"PAD1A"` on pad 1.
pub fn pad_name(pad: usize, suffix: &str) -> String {
    match pad {
        0 => format!("PAD{suffix}"),
        n => format!("PAD{n}{suffix}"),
    }
}

/// Where a pad's small, unintended deflections are cut off, as a fraction of full
/// travel. A game may change them (`Input.SetDeadZone`); the platform reads them each
/// time it writes a pad's axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeadZones {
    pub stick: f32,
    pub trigger: f32,
}

impl Default for DeadZones {
    fn default() -> Self {
        Self {
            stick: 0.15,
            trigger: 0.05,
        }
    }
}

impl DeadZones {
    /// Both zones clamped to `[0, 0.95]`, so full travel always reads as `1`.
    pub fn clamped(stick: f32, trigger: f32) -> Self {
        Self {
            stick: stick.clamp(0.0, 0.95),
            trigger: trigger.clamp(0.0, 0.95),
        }
    }
}

/// A stick's radial dead zone: inside `zone` the stick reads `(0, 0)`; outside, the
/// magnitude is rescaled from `zone..1` to `0..1`, keeping the direction.
pub fn radial_dead_zone(x: f32, y: f32, zone: f32) -> (f32, f32) {
    let magnitude = x.hypot(y);
    if magnitude <= zone {
        return (0.0, 0.0);
    }
    let scaled = ((magnitude - zone) / (1.0 - zone)).min(1.0);
    let k = scaled / magnitude;
    (x * k, y * k)
}

/// A single axis's dead zone (the triggers): the same rescale on one value.
pub fn axial_dead_zone(value: f32, zone: f32) -> f32 {
    let magnitude = value.abs();
    if magnitude <= zone {
        return 0.0;
    }
    ((magnitude - zone) / (1.0 - zone)).min(1.0) * value.signum()
}

/// One rumble request: motor strengths in `0..1` for `seconds` of real time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rumble {
    /// The low-frequency (strong, heavy) motor.
    pub low: f32,
    /// The high-frequency (weak, buzzy) motor.
    pub high: f32,
    pub seconds: f32,
}

impl Rumble {
    /// Strengths clamped to `0..1`, duration to `≥ 0`. A zero request stops rumble.
    pub fn new(low: f32, high: f32, seconds: f32) -> Self {
        Self {
            low: low.clamp(0.0, 1.0),
            high: high.clamp(0.0, 1.0),
            seconds: seconds.max(0.0),
        }
    }
}

/// The sim-side pad records inside `InputState`: connected slots, dead zones, and
/// rumble (the last request per pad, plus the ones the platform has yet to play).
#[derive(Clone, Debug, Default)]
pub struct PadRecords {
    connected: [bool; MAX_PADS],
    pub dead_zones: DeadZones,
    last_rumble: [Rumble; MAX_PADS],
    queued_rumble: Vec<(usize, Rumble)>,
}

impl PadRecords {
    pub fn is_connected(&self, pad: usize) -> bool {
        self.connected.get(pad).copied().unwrap_or(false)
    }

    /// Record a pad (dis)connecting. Out-of-range slots are ignored.
    pub fn set_connected(&mut self, pad: usize, connected: bool) {
        if let Some(slot) = self.connected.get_mut(pad) {
            *slot = connected;
        }
    }

    /// Ask the platform to rumble `pad`. Out-of-range slots are ignored.
    pub fn request_rumble(&mut self, pad: usize, rumble: Rumble) {
        if let Some(slot) = self.last_rumble.get_mut(pad) {
            *slot = rumble;
            self.queued_rumble.push((pad, rumble));
        }
    }

    /// The last rumble requested for `pad` (all zeros if none).
    pub fn last_rumble(&self, pad: usize) -> Rumble {
        self.last_rumble.get(pad).copied().unwrap_or_default()
    }

    /// The requests not yet played, oldest first. The platform drains them each frame.
    pub fn take_rumble(&mut self) -> Vec<(usize, Rumble)> {
        std::mem::take(&mut self.queued_rumble)
    }
}

#[cfg(test)]
#[path = "gamepad_tests.rs"]
mod gamepad_tests;
