//! The state digest a demo stores every [`EVERY`] ticks, and a replay compares
//! (#1581): what makes "bit for bit" a check, not a promise.
//!
//! It hashes the world snapshot an agent reads (`dev::snapshot`: every entity in
//! the authoring-rich shape, the camera, the frame) and where the `Random` stream
//! stands, so a draw the world shows nothing of yet still counts.

use crate::app::GameWorld;
use crate::core::hash::fnv1a64;

/// Ticks between two digests: two seconds at 60 Hz.
pub const EVERY: u64 = 120;

/// Whether the tick `t` (from the one that entered Play) ends with a digest.
pub fn due(t: u64) -> bool {
    (t + 1).is_multiple_of(EVERY)
}

/// The world's state as one hex string.
pub fn of(world: &GameWorld) -> String {
    let snapshot = crate::dev::snapshot::snapshot(world);
    let mut bytes = serde_json::to_vec(&snapshot).unwrap_or_default();
    let random = world.script_manager().random_state();
    bytes.extend_from_slice(&random.to_le_bytes());
    format!("{:016x}", fnv1a64(&bytes))
}
