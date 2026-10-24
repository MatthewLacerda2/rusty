// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod application;
pub mod clipboard;
pub mod collections;
pub mod color;
pub mod curve;
pub mod frame_stats;
pub mod fs;
pub mod gamepad;
pub mod hash;
pub mod input;
pub mod keymap;
pub mod named_enum;
pub mod paths;
pub mod project;
pub mod quality;
pub mod random;
pub mod storage;
pub mod variant_case;
pub mod video;

pub(crate) use named_enum::named_enum;
