//! crates/scene/src/rules/joint.rs — a joint drive's bounds (#449).
//!
//! The Joint ops (`authoring::physics::joint`) store a drive through this, and a
//! drive held outside a joint (a ragdoll's hit reaction, `skeleton::ragdoll`) is
//! held by the same rule.
//!
//! Pure.

use super::sanitize::non_negative;

/// A drive strength or damping as a joint stores it: floored at 0, and 0 when NaN
/// or infinite.
pub fn drive_value(v: f32) -> f32 {
    non_negative(v)
}
