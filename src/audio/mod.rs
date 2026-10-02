//! src/audio/ — the audio runtime (#212).
//!
//! The platform-layer audio engine: the [`AudioMaestro`] resource (one per World)
//! owns the device/mixer, enforces a single master volume, holds the live roster of
//! entity voices, and keeps the agent-facing introspection (a play-event log + a
//! per-source roster). Each frame the windowed shell re-resolves every live voice
//! (3D rolloff + pan from #213, `Time.timeScale` / pause) and hands the resulting
//! [`VoiceMix`] to the backend (`mix.rs`, #412); the real device pans each voice
//! through `device/voice.rs`. Every voice routes through a group of the [`Mixer`]
//! (`mixer/`, #465): buses with volume, filters and a reverb send, snapshots and
//! ducking, stepped by sim time. The speaker mode (#546, `speaker.rs`) shapes the
//! output for the listening setup: per voice here, on the summed signal at the
//! device's output stage.
//!
//! Layering: the maestro and the backend trait live here so both the windowed app
//! and the (device-free) harness can construct a maestro. The real kira device
//! ([`KiraBackend`]) is the *only* part that touches hardware; it is injected by
//! `shell/boot.rs` after `GameWorld::new`, so the deterministic sim/harness keep the
//! [`NullBackend`]. Nothing here reads a wall clock or unseeded RNG.
//!
//! Allowed deps: components (the `AudioSource` data), core (`Storage`, for the
//! persisted speaker mode), asset (the shared decoder, #385), kira (device only).

pub mod backend;
pub mod device;
pub mod introspection;
pub mod maestro;
pub mod mix;
pub mod mixer;
pub mod occlusion;
#[cfg(test)]
pub mod recording;
pub mod spatial;
pub mod speaker;

pub use backend::{AudioBackend, NullBackend, PlayParams, VoiceId, VoiceMix};
pub use device::KiraBackend;
pub use introspection::{AudioEvent, AudioEventKind, AudioEventLog, SpatialResult, VoiceInfo};
pub use maestro::AudioMaestro;
pub use mix::{MixEnv, Rolloff, Shot};
pub use mixer::{GroupPatch, GroupState, Mixer};
pub use occlusion::{Occluder, OcclusionSettings};
pub use spatial::Listener;
pub use speaker::SpeakerMode;
