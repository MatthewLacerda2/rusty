//! src/scene/authoring/reverb_zone.rs — Shared AudioReverbZone authoring ops (#469).
//!
//! The ONE place the engine mutates a `ReverbZoneComponent`. The inspector card and
//! the Lua `AudioReverbZone.*` setters both route every write through these, so the
//! bounds live once: radii stay non-negative with the fade radius never inside the
//! full one, and params stay in range. Unity's preset rule lives here too: picking a
//! preset writes its params, editing a param makes the zone `Custom`.
//!
//! Allowed deps: components (the component data). Pure.

use crate::components::{ReverbParams, ReverbPreset, ReverbZoneComponent};

/// Set the full-effect radius, floored at 0; the fade radius grows to keep up.
pub fn set_min_distance(z: &mut ReverbZoneComponent, distance: f32) {
    z.min_distance = non_negative(distance);
    z.max_distance = z.max_distance.max(z.min_distance);
}

/// Set the fade-out radius, never under the full-effect radius.
pub fn set_max_distance(z: &mut ReverbZoneComponent, distance: f32) {
    z.max_distance = non_negative(distance).max(z.min_distance);
}

/// Switch to `preset`, writing its params; `Custom` keeps the params as they are.
pub fn set_preset(z: &mut ReverbZoneComponent, preset: ReverbPreset) {
    z.preset = preset;
    if let Some(params) = preset.params() {
        z.params = params;
    }
}

/// Set the params (clamped into range); the zone becomes `Custom`.
pub fn set_params(z: &mut ReverbZoneComponent, params: ReverbParams) {
    z.params = params.clamped();
    z.preset = ReverbPreset::Custom;
}

/// `v` floored at 0; a NaN reads as 0.
fn non_negative(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radii_stay_ordered_and_non_negative() {
        let mut z = ReverbZoneComponent::default();
        set_min_distance(&mut z, 20.0);
        assert_eq!((z.min_distance, z.max_distance), (20.0, 20.0));
        set_max_distance(&mut z, 5.0);
        assert_eq!(z.max_distance, 20.0, "never inside the full radius");
        set_min_distance(&mut z, f32::NAN);
        set_max_distance(&mut z, 30.0);
        assert_eq!((z.min_distance, z.max_distance), (0.0, 30.0));
    }

    #[test]
    fn a_preset_writes_its_params_and_an_edit_makes_the_zone_custom() {
        let mut z = ReverbZoneComponent::default();
        set_preset(&mut z, ReverbPreset::Tunnel);
        assert_eq!(z.params, ReverbPreset::Tunnel.params().unwrap());
        let wet = ReverbParams {
            wet: 9.0,
            ..z.params
        };
        set_params(&mut z, wet);
        assert_eq!((z.preset, z.params.wet), (ReverbPreset::Custom, 1.0));
        set_preset(&mut z, ReverbPreset::Custom);
        assert_eq!(z.params.wet, 1.0, "Custom keeps the hand-set params");
    }
}
