//! The engine's own per-frame ticks the schedule runs: the render camera's sync,
//! decal ageing, the particle simulation, trail recording and the UI systems.

pub(super) mod camera_sync;
pub(super) mod decals;
pub(super) mod particles;
pub(super) mod trails;
pub(super) mod ui;
