//! src/audio/stop.rs — silencing the voices of entities that leave the scene (#1297).

use super::maestro::AudioMaestro;

impl AudioMaestro {
    /// Stop the live voice of every `(id, position)` given, logging each `Stop` — a
    /// destroy or a scene unload: Unity silences an `AudioSource` with its object.
    /// Entities with no live voice are skipped.
    pub fn stop_sources(&mut self, targets: impl IntoIterator<Item = (u32, [f32; 3])>, tick: u64) {
        for (id, position) in targets {
            if self.is_source_playing(id) {
                self.stop_source(id, position, tick, true);
            }
        }
    }
}
