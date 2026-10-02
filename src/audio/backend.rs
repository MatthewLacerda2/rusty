//! src/audio/backend.rs — the audio backend abstraction (#212).
//!
//! The `AudioMaestro` (the resource) owns one `Box<dyn AudioBackend>` and never
//! talks to a sound device directly. Two implementations exist:
//!
//!   * [`NullBackend`] — does nothing, holds no device. It is the **default**, and
//!     the one the headless / `play` harness uses, so the deterministic sim never
//!     depends on an audio device (CLAUDE.md: the platform layer owns hardware).
//!   * `KiraBackend` (see `device/kira.rs`) — the real mixer, opened by the
//!     windowed shell (`shell/boot.rs`). It is wired in *after* `GameWorld::new`, so the
//!     harness path that calls `GameWorld::new` stays device-free.
//!
//! This indirection is the whole reason audio can satisfy the determinism guard: a
//! play action is recorded in the (deterministic) introspection log regardless of
//! backend, while the actual sound is a platform-layer side effect the `NullBackend`
//! simply skips.

use super::mixer::settings::LOW_PASS_OFF;
use super::mixer::{GroupId, GroupMix};
use super::speaker::SpeakerMode;

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
    /// Stereo pan in `[-1, 1]` (left → right), already scaled by `spatial_blend`
    /// (and narrowed by the headphones speaker mode on its way to the backend).
    pub pan: f32,
    /// The source's `spatial_blend` in `[0, 1]`: how far a stereo clip is downmixed
    /// to mono before panning (`0` keeps its stereo image untouched).
    pub spatial_blend: f32,
    /// Playback rate (`Time.timeScale` for a time-scaled voice, else `1.0`).
    pub speed: f32,
    /// Whether the voice is held paused (resumes where it left off).
    pub paused: bool,
    /// The voice's own low-pass cutoff in Hz, closed by occlusion (#467); at
    /// `LOW_PASS_OFF` the filter is bypassed.
    pub low_pass: f32,
}

impl VoiceMix {
    /// Full gain, centred, 2D, normal speed — a plain un-spatialized voice.
    pub const FLAT: VoiceMix = VoiceMix {
        gain: 1.0,
        pan: 0.0,
        spatial_blend: 0.0,
        speed: 1.0,
        paused: false,
        low_pass: LOW_PASS_OFF,
    };

    /// This mix with its gain multiplied by `master`.
    pub fn with_master(self, master: f32) -> Self {
        Self {
            gain: self.gain * master,
            ..self
        }
    }

    /// The mix the backend receives: folded with `master`, then shaped by the
    /// speaker `mode` (#546).
    pub fn for_output(self, master: f32, mode: SpeakerMode) -> Self {
        mode.shape(self.with_master(master))
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
    /// The mixer group the voice routes into (#465).
    pub group: GroupId,
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

    /// Shape the summed output for `mode` (#546) — the master-bus half of the
    /// speaker mode; the per-voice half arrives already folded into each mix.
    fn set_speaker_mode(&mut self, mode: SpeakerMode);

    /// Create mixer group `id` under `parent` (`None`: the root, summing into the
    /// output), replacing any group already at `id` (#465). Parents come first.
    fn add_group(&mut self, id: GroupId, parent: Option<GroupId>);

    /// Apply group `id`'s resolved mix (volume, filters, reverb send) in place.
    fn set_group(&mut self, id: GroupId, mix: &GroupMix);
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
    /// No output to shape; the maestro keeps the mode for read-back.
    fn set_speaker_mode(&mut self, _mode: SpeakerMode) {}
    /// No tracks to build; the maestro's mixer keeps the resolved state.
    fn add_group(&mut self, _id: GroupId, _parent: Option<GroupId>) {}
    fn set_group(&mut self, _id: GroupId, _mix: &GroupMix) {}
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
            group: GroupId::MASTER,
        };
        assert!(b.play(VoiceId(1), &params));
        // These are all no-ops; just assert they don't panic.
        b.set_mix(VoiceId(1), &VoiceMix::FLAT.with_master(0.5));
        assert!(!b.is_live(VoiceId(1)));
        b.stop(VoiceId(1));
        b.stop_all();
        b.set_speaker_mode(SpeakerMode::Tv);
        b.add_group(GroupId(1), Some(GroupId::MASTER));
    }
}
