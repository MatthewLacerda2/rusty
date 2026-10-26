//! Lens flare's ops (#1443): the switch, the strengths, the ghost count, the tint
//! and the flare's own chromatic aberration.

use super::{Effect, Knob};
use crate::components::rendering::post_processing::LensFlare;
use crate::components::PostProcessingComponent;
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::{finite_array_or, non_negative, unit};

/// Set whether the lens flare draws.
pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.lens_flare.active = active;
}

/// Set the whole flare's strength, clamped to `≥ 0`.
pub fn set_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.lens_flare.intensity = non_negative(value);
}

/// Set how many ghosts the flare draws, clamped to `0..=8`.
pub fn set_ghost_count(vc: &mut PostProcessingComponent, count: u32) {
    vc.lens_flare.ghost_count = count.min(LensFlare::MAX_GHOSTS);
}

/// Set the ghosts' strength, clamped to `≥ 0`.
pub fn set_ghost_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.lens_flare.ghost_intensity = non_negative(value);
}

/// Set the halo's strength, clamped to `≥ 0`.
pub fn set_halo_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.lens_flare.halo_intensity = non_negative(value);
}

/// Set the streaks' strength, clamped to `≥ 0`.
pub fn set_streak_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.lens_flare.streak_intensity = non_negative(value);
}

/// Set the flare's tint, each channel clamped to `0..=1` (a colour with a NaN or
/// infinite channel keeps the current one).
pub fn set_tint(vc: &mut PostProcessingComponent, tint: [f32; 3]) {
    vc.lens_flare.tint = finite_array_or(tint, vc.lens_flare.tint).map(unit);
}

/// Set how far the flare's channels split, clamped to `0..=1`.
pub fn set_chromatic_aberration(vc: &mut PostProcessingComponent, value: f32) {
    vc.lens_flare.chromatic_aberration = unit(value);
}

pub const EFFECT: Effect = Effect {
    name: "lens_flare",
    field: effect!(lens_flare, |c, v: LensFlare| {
        set_active(c, v.active);
        set_intensity(c, v.intensity);
        set_ghost_count(c, v.ghost_count);
        set_ghost_intensity(c, v.ghost_intensity);
        set_halo_intensity(c, v.halo_intensity);
        set_streak_intensity(c, v.streak_intensity);
        set_tint(c, v.tint);
        set_chromatic_aberration(c, v.chromatic_aberration);
    }),
    knobs: &[
        Knob {
            field: "intensity",
            api: Some("LensFlareIntensity"),
            get: |vc| vc.lens_flare.intensity,
            set: set_intensity,
        },
        Knob {
            field: "ghost_intensity",
            api: Some("LensFlareGhostIntensity"),
            get: |vc| vc.lens_flare.ghost_intensity,
            set: set_ghost_intensity,
        },
        Knob {
            field: "halo_intensity",
            api: Some("LensFlareHaloIntensity"),
            get: |vc| vc.lens_flare.halo_intensity,
            set: set_halo_intensity,
        },
        Knob {
            field: "streak_intensity",
            api: Some("LensFlareStreakIntensity"),
            get: |vc| vc.lens_flare.streak_intensity,
            set: set_streak_intensity,
        },
        Knob {
            field: "chromatic_aberration",
            api: Some("LensFlareChromaticAberration"),
            get: |vc| vc.lens_flare.chromatic_aberration,
            set: set_chromatic_aberration,
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::authoring::default_post_processing;

    #[test]
    fn every_write_is_clamped_to_its_range() {
        let mut vc = default_post_processing();
        set_intensity(&mut vc, -1.0);
        set_ghost_count(&mut vc, 99);
        set_halo_intensity(&mut vc, f32::NAN);
        set_tint(&mut vc, [2.0, -1.0, 0.5]);
        set_chromatic_aberration(&mut vc, 3.0);
        let f = vc.lens_flare;
        assert_eq!((f.intensity, f.ghost_count), (0.0, LensFlare::MAX_GHOSTS));
        assert_eq!((f.halo_intensity, f.chromatic_aberration), (0.0, 1.0));
        assert_eq!(f.tint, [1.0, 0.0, 0.5]);
        set_tint(&mut vc, [f32::INFINITY, 0.0, 0.0]);
        assert_eq!(
            vc.lens_flare.tint,
            [1.0, 0.0, 0.5],
            "a bad colour keeps the old"
        );
    }
}
