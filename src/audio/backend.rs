//! src/audio/backend.rs — the audio backend abstraction (#212).
//!
//! The `AudioMaestro` (the resource) owns one `Box<dyn AudioBackend>` and never
//! talks to a sound device directly. Two implementations exist:
//!
//!   * [`NullBackend`] — does nothing, holds no device. It is the **default**, and
//!     the one the headless / `play` harness uses, so the deterministic sim never
//!     depends on an audio device (CLAUDE.md: the platform layer owns hardware).
//!   * `RodioBackend` (see `device/rodio.rs`) — the real mixer, opened by the
//!     windowed shell (`shell/boot.rs`). It is wired in *after* `GameWorld::new`, so the
//!     harness path that calls `GameWorld::new` stays device-free.
//!
//! This indirection is the whole reason audio can satisfy the determinism guard: a
//! play action is recorded in the (deterministic) introspection log regardless of
//! backend, while the actual sound is a platform-layer side effect the `NullBackend`
//! simply skips.

/// A handle to one playing voice, opaque to the maestro. The backend maps it to its
/// own internal sink/source; the maestro only stores it to later stop or remix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VoiceId(pub u64);

/// The per-voice mix the backend applies (#412): everything the maestro resolves
/// each frame, handed over in one value so gain, pan and rate can never be applied
/// from different frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceMix {
    /// Linear gain, already folded with the master volume by the maestro.
    pub gain: f32,
    /// Stereo pan in `[-1, 1]` (left → right), already scaled by `spatial_blend`.
    pub pan: f32,
    /// The source's `spatial_blend` in `[0, 1]`: how far a stereo clip is downmixed
    /// to mono before panning (`0` keeps its stereo image untouched).
    pub spatial_blend: f32,
    /// Playback rate (`Time.timeScale` for a time-scaled voice, else `1.0`).
    pub speed: f32,
    /// Whether the voice is held paused (resumes where it left off).
    pub paused: bool,
}

impl VoiceMix {
    /// Full gain, centred, 2D, normal speed — a plain un-spatialized voice.
    pub const FLAT: VoiceMix = VoiceMix {
        gain: 1.0,
        pan: 0.0,
        spatial_blend: 0.0,
        speed: 1.0,
        paused: false,
    };

    /// This mix with its gain multiplied by `master`.
    pub fn with_master(self, master: f32) -> Self {
        Self {
            gain: self.gain * master,
            ..self
        }
    }
}

/// How a voice should be started.
#[derive(Clone, Debug)]
pub struct PlayParams {
    /// Path to the decoded clip (`.ogg` / `.wav` / `.mp3`).
    pub clip: String,
    /// Whether the voice loops.
    pub looping: bool,
    /// The initial mix, so the first samples already play at the resolved state.
    pub mix: VoiceMix,
}

/// The platform-layer audio device abstraction. Implementors own the mixer; the
/// maestro drives them with already-resolved (master-folded) mixes.
pub trait AudioBackend {
    /// Start a voice with `params`, returning whether it started. `false` means the
    /// clip could not be played (missing/undecodable file) — the maestro still logs
    /// the *intent*, so the agent sees the attempt.
    fn play(&mut self, id: VoiceId, params: &PlayParams) -> bool;

    /// Stop the voice `id` if it is still live (no-op otherwise).
    fn stop(&mut self, id: VoiceId);

    /// Apply a new mix to the live voice `id` without restarting it (no-op if gone).
    fn set_mix(&mut self, id: VoiceId, mix: &VoiceMix);

    /// Whether voice `id` is still producing sound. The maestro reaps finished
    /// one-shots with it.
    fn is_live(&self, id: VoiceId) -> bool;

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
    fn set_mix(&mut self, _id: VoiceId, _mix: &VoiceMix) {}
    /// Nothing plays, so nothing is ever still sounding: one-shots reap at once.
    fn is_live(&self, _id: VoiceId) -> bool {
        false
    }
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
            looping: false,
            mix: VoiceMix::FLAT,
        };
        assert!(b.play(VoiceId(1), &params));
        // These are all no-ops; just assert they don't panic.
        b.set_mix(VoiceId(1), &VoiceMix::FLAT.with_master(0.5));
        assert!(!b.is_live(VoiceId(1)));
        b.stop(VoiceId(1));
        b.stop_all();
    }
}
