//! The helpers every workspace member's in-crate tests share: a crate of its own,
//! because a `#[cfg(test)]` item is invisible outside its crate. Only ever a
//! dev-dependency (crates/CLAUDE.md), so nothing that ships links it.

pub mod logs;
pub mod luts;
pub mod models;
pub mod panorama;
pub mod rebakes;
pub mod temp;
