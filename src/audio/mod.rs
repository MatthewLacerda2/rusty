//! src/audio/ — the audio runtime (#212).
//!
//! The platform-layer audio engine: the [`AudioMaestro`] resource (one per World)
//! owns the device/mixer, enforces a single master volume, holds the live roster of
//! entity voices, and keeps the agent-facing introspection (a play-event log + a
//! per-source roster). 2D (non-spatialized) playback only this issue;
//! 3D spatialization is the follow-up (#213), whose fields the `AudioSource`
//! component already stores.
//!
//! Layering: the maestro and the backend trait live here so both the windowed app
//! and the (device-free) harness can construct a maestro. The real `rodio` device
//! ([`RodioBackend`]) is the *only* part that touches hardware; it is injected by
//! `main.rs` after `GameWorld::new`, so the deterministic sim/harness keep the
//! [`NullBackend`]. Nothing here reads a wall clock or unseeded RNG.
//!
//! Allowed deps: components (the `AudioSource` data), rodio (backend only).

pub mod backend;
pub mod decode;
pub mod introspection;
pub mod maestro;
pub mod rodio_backend;

pub use backend::{AudioBackend, NullBackend, PlayParams, VoiceId};
pub use introspection::{AudioEvent, AudioEventKind, AudioEventLog, VoiceInfo};
pub use maestro::AudioMaestro;
pub use rodio_backend::RodioBackend;
