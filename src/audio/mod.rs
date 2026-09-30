//! src/audio/ — the audio runtime (#212).
//!
//! The platform-layer audio engine: the [`AudioMaestro`] resource (one per World)
//! owns the device/mixer, enforces a single master volume, holds the live roster of
//! entity voices, and keeps the agent-facing introspection (a play-event log + a
//! per-source roster). Each frame the windowed shell re-resolves every live voice
//! (3D rolloff + pan from #213, `Time.timeScale` / pause) and hands the resulting
//! [`VoiceMix`] to the backend (`mix.rs`, #412); the real device pans through
//! `device/pan.rs`. The speaker mode (#546, `speaker.rs`) shapes that output for the
//! listening setup: per voice here, on the summed signal at the device's master bus.
//!
//! Layering: the maestro and the backend trait live here so both the windowed app
//! and the (device-free) harness can construct a maestro. The real `rodio` device
//! ([`RodioBackend`]) is the *only* part that touches hardware; it is injected by
//! `shell/boot.rs` after `GameWorld::new`, so the deterministic sim/harness keep the
//! [`NullBackend`]. Nothing here reads a wall clock or unseeded RNG.
//!
//! Allowed deps: components (the `AudioSource` data), core (`Storage`, for the
//! persisted speaker mode), rodio (backend only).

pub mod backend;
pub mod device;
pub mod introspection;
pub mod maestro;
pub mod mix;
#[cfg(test)]
pub mod recording;
pub mod spatial;
pub mod speaker;

pub use backend::{AudioBackend, NullBackend, PlayParams, VoiceId, VoiceMix};
pub use device::RodioBackend;
pub use introspection::{AudioEvent, AudioEventKind, AudioEventLog, SpatialResult, VoiceInfo};
pub use maestro::AudioMaestro;
pub use mix::{MixEnv, Rolloff, Shot};
pub use spatial::Listener;
pub use speaker::SpeakerMode;
