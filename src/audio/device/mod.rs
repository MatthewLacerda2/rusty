//! src/audio/device/ — the real sound device: everything that touches `rodio`.
//!
//! The platform-layer half of audio: the [`RodioBackend`] (one sink per voice), the
//! path-cached decoder it plays from, the stereo panner every voice runs through
//! (#412), and the master bus they all sum into (#546). The maestro reaches it only through the `AudioBackend` trait, and the
//! headless harness never constructs it.

pub mod decode;
pub mod master;
pub mod pan;
pub mod rodio;

pub use self::rodio::RodioBackend;
