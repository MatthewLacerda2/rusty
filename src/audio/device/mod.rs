//! src/audio/device/ — the real sound device: everything that touches kira (#465).
//!
//! The platform-layer half of audio: the [`KiraBackend`] (kira's mixer), the
//! path-cached decoder it plays from, the voice every clip plays as (#412's pan law),
//! the mixer groups as kira tracks, the reverb bus reverb zones tune (#469), the speaker-mode output stage (#546),
//! the final-mix tap a capture reads and the offline backend a headless one runs on (#1579). The
//! maestro reaches it only through the `AudioBackend` trait. The windowed shell opens
//! it on the sound card; a headless run constructs it only when a capture starts,
//! over the offline backend, and keeps it from then on.

pub mod backend;
pub mod capture;
pub mod decode;
pub mod groups;
pub mod lowpass;
pub mod output;
pub mod reverb;
pub mod tap;
pub mod voice;

pub use backend::KiraBackend;
