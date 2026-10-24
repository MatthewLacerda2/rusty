//! src/audio/stop.rs — silencing the voices of entities that leave the scene (#1297)
//! or go inactive in the hierarchy (#1363).

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

    /// Record whether entity `id`'s `AudioSource` is active in the hierarchy this
    /// frame, returning `true` on an edge: a source never seen counts as inactive,
    /// so its first active frame is a rising edge (Unity's OnEnable).
    pub fn note_source_active(&mut self, id: u32, active: bool) -> bool {
        if active {
            self.enabled_sources.insert(id)
        } else {
            self.enabled_sources.remove(&id)
        }
    }

    /// Forget the activity of every source `keep` rejects (gone from the scene).
    pub fn retain_sources(&mut self, keep: impl Fn(u32) -> bool) {
        self.enabled_sources.retain(|&id| keep(id));
    }
}
