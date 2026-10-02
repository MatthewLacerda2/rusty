//! The maestro's half of reverb zones (#469): keep the listener's resolved
//! reverb, hand a change to the device, and read it back.

use super::{blend, ReverbState, ZoneAt};
use crate::audio::maestro::AudioMaestro;
use glam::Vec3;

impl AudioMaestro {
    /// The reverb the listener heard at the last resolve.
    pub fn reverb_state(&self) -> ReverbState {
        self.reverb
    }

    /// Resolve the listener's reverb among `zones` and send it to the device when
    /// it changed.
    pub fn resolve_reverb(&mut self, listener: Vec3, zones: &[ZoneAt]) {
        self.set_reverb(blend(listener, zones));
    }

    /// Run the reverb bus at `state`.
    pub(crate) fn set_reverb(&mut self, state: ReverbState) {
        if state != self.reverb {
            self.reverb = state;
            self.backend.set_reverb(&state.params);
        }
    }
}
