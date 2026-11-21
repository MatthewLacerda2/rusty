//! frequency modulation: one sine bending another's phase.

pub(crate) mod feedback;
pub(crate) mod four;
pub(crate) mod two;
pub(crate) mod voicing;

/// When the note is let go, its seed and the sample rate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Note {
    pub(crate) gate: f32,
    pub(crate) seed: u64,
    pub(crate) rate: f32,
}
