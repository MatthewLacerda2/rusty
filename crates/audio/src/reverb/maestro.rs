use super::{blend, ReverbState, ZoneAt};
use crate::maestro::AudioMaestro;
use glam::Vec3;

impl AudioMaestro {
    pub fn reverb_state(&self) -> ReverbState {
        self.reverb
    }

    pub fn resolve_reverb(&mut self, listener: Vec3, zones: &[ZoneAt]) {
        self.set_reverb(blend(listener, zones));
    }

    pub(crate) fn set_reverb(&mut self, state: ReverbState) {
        if state != self.reverb {
            self.reverb = state;
            self.backend.set_reverb(&state.params);
        }
    }
}
