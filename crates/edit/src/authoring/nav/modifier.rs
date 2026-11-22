use glam::Vec3;

use rusty_components::NavMeshModifierVolumeComponent as Volume;
use rusty_scene::nav_settings::MAX_AREAS;
use rusty_scene::rules::sanitize::finite3_or;

pub const MIN_SIZE: f32 = rusty_scene::rules::sanitize::MIN_EXTENT;

pub fn set_active(v: &mut Volume, active: bool) {
    v.active = active;
}

pub fn set_center(v: &mut Volume, center: Vec3) {
    v.center = finite3_or(center, v.center);
}

pub fn set_size(v: &mut Volume, size: Vec3) {
    v.size = finite3_or(size, v.size).max(Vec3::splat(MIN_SIZE));
}

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
