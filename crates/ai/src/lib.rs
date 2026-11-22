// Panic-free sim: bare `.unwrap()` is denied here (use `?` or a documented
// `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod perception;
pub mod query;
pub mod ray;
pub mod stand;
