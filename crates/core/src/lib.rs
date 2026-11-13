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
pub mod hash;
pub mod hulls;
pub mod input;
pub mod naming;
pub mod paths;
pub mod project;
pub mod quiet;
pub mod random;
pub mod settings;
pub mod storage;
pub mod wake;

/// serde, for [`named_enum!`]'s expansion: a crate calling the macro needs no serde
/// dependency of its own.
#[doc(hidden)]
pub use serde as __serde;
