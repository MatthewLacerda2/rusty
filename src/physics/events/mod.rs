//! src/physics/events/mod.rs — what a physics step reports: solid contacts
//! (`collision`) and trigger overlaps (`trigger`).

pub(super) mod collision;
#[cfg(test)]
mod collision_tests;
pub(super) mod trigger;
#[cfg(test)]
mod trigger_tests;
