//! src/scene/authoring/rendering/light_probe_proxy_volume.rs — Shared Light Probe
//! Proxy Volume authoring ops (#1672).
//!
//! The ONE place the engine mutates a `LightProbeProxyVolumeComponent`. The
//! inspector card, the Lua `LightProbeProxyVolume.*` setters and `Component.Set`
//! route every write through these, so the bounds live once: a custom box is
//! finite and never flat, a density stays in range, and each axis of a custom
//! resolution is a power of two from 1 to 32 (Unity's rule).
//!
//! Pure.

use glam::Vec3;

use crate::components::rendering::light_probe_proxy_volume::{resolution, DENSITY_RANGE};
use crate::components::{
    LightProbeProxyVolumeComponent as Volume, ProxyBoundsMode, ProxyQuality, ProxyResolutionMode,
};
use crate::scene::authoring::sanitize::{clamped, finite3_or, finite_or, MIN_EXTENT};

/// Where the box comes from.
pub fn set_bounds_mode(v: &mut Volume, mode: ProxyBoundsMode) {
    v.bounds_mode = mode;
}

/// The custom box's size, each side at least [`MIN_EXTENT`]; a non-finite size
/// keeps the current one.
pub fn set_size(v: &mut Volume, size: Vec3) {
    v.size = finite3_or(size, v.size).abs().max(Vec3::splat(MIN_EXTENT));
}

/// The custom box's centre, in local space; a non-finite origin keeps the current
/// one.
pub fn set_origin(v: &mut Volume, origin: Vec3) {
    v.origin = finite3_or(origin, v.origin);
}

/// How the resolution is picked.
pub fn set_resolution_mode(v: &mut Volume, mode: ProxyResolutionMode) {
    v.resolution_mode = mode;
}

/// Grid points per metre, clamped to [`DENSITY_RANGE`]; NaN or infinite keeps the
/// current density.
pub fn set_probe_density(v: &mut Volume, density: f32) {
    let d = finite_or(density, v.probe_density);
    v.probe_density = clamped(d, DENSITY_RANGE.0, DENSITY_RANGE.1);
}

/// Points per axis, each rounded up to a power of two from 1 to 32.
pub fn set_grid_resolution(v: &mut Volume, dims: [u32; 3]) {
    v.grid_resolution = dims.map(resolution);
}

/// The SH bands each point keeps.
pub fn set_quality(v: &mut Volume, quality: ProxyQuality) {
    v.quality = quality;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_custom_box_stays_finite_and_never_flat() {
        let mut v = Volume::default();
        set_size(&mut v, Vec3::new(-2.0, 0.0, 3.0));
        assert_eq!(v.size, Vec3::new(2.0, MIN_EXTENT, 3.0));
        set_size(&mut v, Vec3::new(f32::NAN, 1.0, 1.0));
        assert_eq!(
            v.size,
            Vec3::new(2.0, MIN_EXTENT, 3.0),
            "NaN keeps the size"
        );
        set_origin(&mut v, Vec3::new(1.0, 2.0, 3.0));
        set_origin(&mut v, Vec3::splat(f32::INFINITY));
        assert_eq!(v.origin, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn density_and_resolution_stay_in_unity_s_range() {
        let mut v = Volume::default();
        set_probe_density(&mut v, 1000.0);
        assert_eq!(v.probe_density, DENSITY_RANGE.1);
        set_probe_density(&mut v, f32::NAN);
        assert_eq!(v.probe_density, DENSITY_RANGE.1, "NaN keeps the density");
        set_probe_density(&mut v, 0.0);
        assert_eq!(v.probe_density, DENSITY_RANGE.0);
        set_grid_resolution(&mut v, [0, 5, 100]);
        assert_eq!(v.grid_resolution, [1, 8, 32]);
    }
}
