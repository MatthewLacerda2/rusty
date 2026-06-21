//! src/audio/backend.rs — the audio backend abstraction (#212).
//!
//! The `AudioMaestro` (the resource) owns one `Box<dyn AudioBackend>` and never
//! talks to a sound device directly. Two implementations exist:
//!
//!   * [`NullBackend`] — does nothing, holds no device. It is the **default**, and
//!     the one the headless / `play` harness uses, so the deterministic sim never
//!     depends on an audio device (CLAUDE.md: the platform layer owns hardware).
//!   * `RodioBackend` (see `rodio_backend.rs`) — the real mixer, opened by the
//!     windowed app in `main.rs`. It is wired in *after* `GameWorld::new`, so the
//!     harness path that calls `GameWorld::new` stays device-free.
//!
//! This indirection is the whole reason audio can satisfy the determinism guard: a
//! play action is recorded in the (deterministic) introspection log regardless of
//! backend, while the actual sound is a platform-layer side effect the `NullBackend`
//! simply skips.

/// A handle to one playing voice, opaque to the maestro. The backend maps it to its
/// own internal sink/source; the maestro only stores it to later stop or revolume.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VoiceId(pub u64);

/// How a voice should be started.
#[derive(Clone, Debug)]
pub struct PlayParams {
    /// Path to the decoded clip (`.ogg` / `.wav`).
    pub clip: String,
    /// Linear gain in `[0, 1]` already folded with the master volume by the maestro.
    pub volume: f32,
    /// Whether the voice loops.
    pub looping: bool,
}

/// The platform-layer audio device abstraction. Implementors own the mixer; the
/// maestro drives them with already-resolved (master-folded) volumes.
pub trait AudioBackend {
    /// Start a voice with `params`, returning its handle on success. A `None` means
    /// the clip could not be played (missing/undecodable file, or a null backend) —
    /// the maestro still logs the *intent*, so the agent sees the attempt.
    fn play(&mut self, id: VoiceId, params: &PlayParams) -> bool;

    /// Stop the voice `id` if it is still live (no-op otherwise).
    fn stop(&mut self, id: VoiceId);

    /// Set the voice `id`'s linear volume (already folded with master).
    fn set_volume(&mut self, id: VoiceId, volume: f32);

    /// Apply a new master volume to every live voice. The maestro passes the live
    /// `(VoiceId, per_source_volume)` pairs so the backend can rescale each.
    fn set_master_volume(&mut self, master: f32, voices: &[(VoiceId, f32)]);

    /// Drop every live voice (e.g. on Stop / leaving Play).
    fn stop_all(&mut self);
}

/// The do-nothing backend: holds no device, plays no sound, but reports a voice as
/// "started" so the maestro's bookkeeping (and the introspection roster) behaves
/// identically with or without hardware. This is what the harness runs on.
#[derive(Default)]
pub struct NullBackend;

impl AudioBackend for NullBackend {
    fn play(&mut self, _id: VoiceId, _params: &PlayParams) -> bool {
        // A null device "accepts" the voice so the playing-set bookkeeping matches
        // the real backend; no sound is produced.
        true
    }
    fn stop(&mut self, _id: VoiceId) {}
    fn set_volume(&mut self, _id: VoiceId, _volume: f32) {}
    fn set_master_volume(&mut self, _master: f32, _voices: &[(VoiceId, f32)]) {}
    fn stop_all(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_backend_accepts_but_makes_no_sound() {
        let mut b = NullBackend;
        let params = PlayParams {
            clip: "x.ogg".to_string(),
            volume: 0.5,
            looping: false,
        };
        assert!(b.play(VoiceId(1), &params));
        // These are all no-ops; just assert they don't panic.
        b.set_volume(VoiceId(1), 0.2);
        b.set_master_volume(0.8, &[(VoiceId(1), 0.5)]);
        b.stop(VoiceId(1));
        b.stop_all();
    }
}
