use super::sanitize::non_negative;

/// A drive's strength or damping, held for a joint and for a ragdoll's hit reaction alike.
pub fn drive_value(v: f32) -> f32 {
    non_negative(v)
}
