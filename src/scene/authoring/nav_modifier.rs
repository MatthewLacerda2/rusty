//! src/scene/authoring/nav_modifier.rs — Shared NavMeshModifierVolume authoring ops (#460).
//!
//! The ONE place the engine mutates an entity's `NavMeshModifierVolumeComponent`.
//! The editor's card and the Lua `NavMeshModifierVolume.*` setters both route
//! every write through these, so the rules live once: the centre stays finite, the
//! box keeps a positive size, and the area is a valid id (`0..MAX_AREAS`).
//!
//! Allowed deps: components (the component data), scene::nav_settings (the area limit). Pure.

use glam::Vec3;

use crate::components::NavMeshModifierVolumeComponent as Volume;
use crate::scene::nav_settings::MAX_AREAS;

/// The smallest extent a box side may have.
pub const MIN_SIZE: f32 = 1e-3;

/// Enable or disable the volume.
pub fn set_active(v: &mut Volume, active: bool) {
    v.active = active;
}

/// Set the box's centre, local to the entity. A non-finite centre is ignored.
pub fn set_center(v: &mut Volume, center: Vec3) {
    if center.is_finite() {
        v.center = center;
    }
}

/// Set the box's size, each side at least [`MIN_SIZE`]. A non-finite size is ignored.
pub fn set_size(v: &mut Volume, size: Vec3) {
    if size.is_finite() {
        v.size = size.max(Vec3::splat(MIN_SIZE));
    }
}

/// Set the area the volume assigns. An id past the area table's limit is ignored.
pub fn set_area(v: &mut Volume, area: i64) {
    if (0..MAX_AREAS as i64).contains(&area) {
        v.area = area as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setters_keep_the_volume_well_formed() {
        let mut v = Volume::default();
        set_center(&mut v, Vec3::new(1.0, 0.0, 2.0));
        set_center(&mut v, Vec3::NAN);
        assert_eq!(v.center, Vec3::new(1.0, 0.0, 2.0));
        set_size(&mut v, Vec3::new(3.0, -1.0, 2.0));
        assert_eq!(v.size, Vec3::new(3.0, MIN_SIZE, 2.0));
        set_size(&mut v, Vec3::INFINITY);
        assert_eq!(v.size.x, 3.0, "a non-finite size is ignored");
        set_area(&mut v, 5);
        set_area(&mut v, 32);
        set_area(&mut v, -1);
        assert_eq!(v.area, 5, "out-of-range ids are ignored");
        set_active(&mut v, false);
        assert!(!v.active);
    }
}
