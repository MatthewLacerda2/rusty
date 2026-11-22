//! Pad 0 is unnumbered (`"PADA"`), pad `n >= 1` inserts its index (`"PAD1A"`). Sticks
//! are Y-up, triggers run 0..1. Dead zones apply at the platform source, so injected axes
//! (`Input.SetAxis`) reach the sim as written; sticks use a radial zone, since a per-axis
//! one snaps aim to the cardinal directions.

//! A pad's buttons are more key names and its sticks and triggers are axes, so it reads
//! through the same `InputState` as the keyboard. Rumble is a request the sim records and
//! the platform plays; headless it stays a record.

pub const BUTTONS: &[&str] = &[
    "A", "B", "X", "Y", "LB", "RB", "LT", "RT", "BACK", "START", "GUIDE", "LS", "RS", "UP", "DOWN",
    "LEFT", "RIGHT",
];

pub const AXES: &[&str] = &[
    "LEFTX",
    "LEFTY",
    "RIGHTX",
    "RIGHTY",
    "LEFTTRIGGER",
    "RIGHTTRIGGER",
];

pub const MAX_PADS: usize = 4;

pub fn pad_name(pad: usize, suffix: &str) -> String {
    match pad {
        0 => format!("PAD{suffix}"),
        n => format!("PAD{n}{suffix}"),
    }
}

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
    /// Each at most 0.95, so full travel reads as 1.
    pub fn clamped(stick: f32, trigger: f32) -> Self {
        Self {
            stick: stick.clamp(0.0, 0.95),
            trigger: trigger.clamp(0.0, 0.95),
        }
    }
}

pub fn radial_dead_zone(x: f32, y: f32, zone: f32) -> (f32, f32) {
    let magnitude = x.hypot(y);
    if magnitude <= zone {
        return (0.0, 0.0);
    }
    let scaled = ((magnitude - zone) / (1.0 - zone)).min(1.0);
    let k = scaled / magnitude;
    (x * k, y * k)
}

pub fn axial_dead_zone(value: f32, zone: f32) -> f32 {
    let magnitude = value.abs();
    if magnitude <= zone {
        return 0.0;
    }
    ((magnitude - zone) / (1.0 - zone)).min(1.0) * value.signum()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rumble {
    pub low: f32,
    pub high: f32,
    pub seconds: f32,
}

impl Rumble {
    pub fn new(low: f32, high: f32, seconds: f32) -> Self {
        Self {
            low: low.clamp(0.0, 1.0),
            high: high.clamp(0.0, 1.0),
            seconds: seconds.max(0.0),
        }
    }
}

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

    pub fn set_connected(&mut self, pad: usize, connected: bool) {
        if let Some(slot) = self.connected.get_mut(pad) {
            *slot = connected;
        }
    }

    pub fn request_rumble(&mut self, pad: usize, rumble: Rumble) {
        if let Some(slot) = self.last_rumble.get_mut(pad) {
            *slot = rumble;
            self.queued_rumble.push((pad, rumble));
        }
    }

    pub fn last_rumble(&self, pad: usize) -> Rumble {
        self.last_rumble.get(pad).copied().unwrap_or_default()
    }

    pub fn take_rumble(&mut self) -> Vec<(usize, Rumble)> {
        std::mem::take(&mut self.queued_rumble)
    }
}

#[cfg(test)]
#[path = "gamepad_tests.rs"]
mod gamepad_tests;
