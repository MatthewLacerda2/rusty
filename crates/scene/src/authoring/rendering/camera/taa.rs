//! crates/scene/src/authoring/rendering/camera/taa.rs — the camera's TAA ops (#1847) and
//! its numeric knobs.
//!
//! The ONE place TAA's camera fields are mutated (#287): the editor's Camera card,
//! the `Graphics.Set/GetTaa*` functions, a loaded file's hold and a
//! `Graphics.taa.<knob>` tween all route through the ops here, so each clamp lives
//! in one place. [`TAA_KNOBS`] names each float knob (the field inside `taa`, its
//! `Graphics.<api>` pair, its get/set): the counterpart of the volume effects'
//! `Knob` table, read for the Inspector sliders, the `Graphics.*` pairs and the
//! tween paths. TAA is a camera effect, so its knobs live here rather than on the
//! post-processing volume.
//!
//! Pure.

use crate::authoring::sanitize::saturated;
use rusty_components::{CameraComponent, TaaQuality};

/// Set the camera's TAA flag — temporal anti-aliasing (#1702), which replaces FXAA
/// while it runs. Shared by the editor's Camera card and `Graphics.SetTaaActive`.
pub fn set_taa_active(c: &mut CameraComponent, active: bool) {
    c.taa_active = active;
}

/// Set TAA's quality level (#1847, URP's): how much of the resolve runs.
pub fn set_taa_quality(c: &mut CameraComponent, quality: TaaQuality) {
    c.taa_quality = quality;
}

/// Set URP's *Base Blend Factor* (#1847), clamped to URP's slider bounds; a NaN
/// takes the low bound.
pub fn set_taa_base_blend_factor(c: &mut CameraComponent, value: f32) {
    c.taa_base_blend_factor = saturated(
        value,
        CameraComponent::MIN_TAA_BASE_BLEND_FACTOR,
        CameraComponent::MAX_TAA_BASE_BLEND_FACTOR,
    );
}

/// Set URP's *Variance Clamp Scale* (#1847), clamped to URP's slider bounds.
pub fn set_taa_variance_clamp_scale(c: &mut CameraComponent, value: f32) {
    c.taa_variance_clamp_scale = saturated(
        value,
        CameraComponent::MIN_TAA_VARIANCE_CLAMP_SCALE,
        CameraComponent::MAX_TAA_VARIANCE_CLAMP_SCALE,
    );
}

/// Set URP's *Jitter Scale* (#1847), clamped to `0..=1`.
pub fn set_taa_jitter_scale(c: &mut CameraComponent, value: f32) {
    c.taa_jitter_scale = saturated(value, 0.0, 1.0);
}

/// Set URP's *Mip Bias* (#1847), clamped to `-1..=0`.
pub fn set_taa_mip_bias(c: &mut CameraComponent, value: f32) {
    c.taa_mip_bias = saturated(value, CameraComponent::MIN_TAA_MIP_BIAS, 0.0);
}

/// Set URP's TAA *Contrast Adaptive Sharpening* (#1847) — RCAS strength — clamped to
/// `0..=1`; `0` runs no sharpen.
pub fn set_taa_contrast_adaptive_sharpening(c: &mut CameraComponent, value: f32) {
    c.taa_contrast_adaptive_sharpening = saturated(value, 0.0, 1.0);
}

/// One numeric TAA knob (#1847): the counterpart of a volume effect's `Knob`, but
/// over the camera component. `field` is the tween path's last part
/// (`Graphics.taa.<field>`), `api` the `Graphics.Get/Set<api>` pair's name, `label`
/// URP's display name and `range` the Inspector slider's bounds, the same ones its
/// `set` clamps to.
#[derive(Debug)]
pub struct CameraKnob {
    pub field: &'static str,
    pub api: &'static str,
    pub label: &'static str,
    pub range: (f32, f32),
    pub get: fn(&CameraComponent) -> f32,
    pub set: fn(&mut CameraComponent, f32),
}

/// TAA's float knobs, in the Inspector's order. Each is an Inspector slider, a
/// `Graphics.Get/Set<api>` pair and a `Graphics.taa.<field>` tween path, all through
/// the same op.
pub const TAA_KNOBS: &[CameraKnob] = &[
    CameraKnob {
        field: "base_blend_factor",
        api: "TaaBaseBlendFactor",
        label: "Base Blend Factor",
        range: (
            CameraComponent::MIN_TAA_BASE_BLEND_FACTOR,
            CameraComponent::MAX_TAA_BASE_BLEND_FACTOR,
        ),
        get: |c| c.taa_base_blend_factor,
        set: set_taa_base_blend_factor,
    },
    CameraKnob {
        field: "variance_clamp_scale",
        api: "TaaVarianceClampScale",
        label: "Variance Clamp Scale",
        range: (
            CameraComponent::MIN_TAA_VARIANCE_CLAMP_SCALE,
            CameraComponent::MAX_TAA_VARIANCE_CLAMP_SCALE,
        ),
        get: |c| c.taa_variance_clamp_scale,
        set: set_taa_variance_clamp_scale,
    },
    CameraKnob {
        field: "jitter_scale",
        api: "TaaJitterScale",
        label: "Jitter Scale",
        range: (0.0, 1.0),
        get: |c| c.taa_jitter_scale,
        set: set_taa_jitter_scale,
    },
    CameraKnob {
        field: "mip_bias",
        api: "TaaMipBias",
        label: "Mip Bias",
        range: (CameraComponent::MIN_TAA_MIP_BIAS, 0.0),
        get: |c| c.taa_mip_bias,
        set: set_taa_mip_bias,
    },
    CameraKnob {
        field: "contrast_adaptive_sharpening",
        api: "TaaContrastAdaptiveSharpening",
        label: "Contrast Adaptive Sharpening",
        range: (0.0, 1.0),
        get: |c| c.taa_contrast_adaptive_sharpening,
        set: set_taa_contrast_adaptive_sharpening,
    },
];

/// The TAA knob a `Graphics.<effect>.<knob>` tween path names (`effect` must be
/// `taa`): `None` for any other effect or an unknown field.
pub fn camera_knob(path: &str) -> Option<&'static CameraKnob> {
    let (effect, field) = path.split_once('.')?;
    (effect == "taa")
        .then(|| TAA_KNOBS.iter().find(|k| k.field == field))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knobs_round_trip_through_their_ops_with_clamps() {
        let mut c = CameraComponent::default();
        // A value inside every knob's own range round-trips exactly.
        for (knob, value) in TAA_KNOBS.iter().zip([0.7, 0.9, 0.5, -0.5, 0.5]) {
            (knob.set)(&mut c, value);
            assert_eq!(
                (knob.get)(&c),
                value,
                "{} should round-trip {value}",
                knob.field
            );
        }
        // Each clamp holds its own bound.
        set_taa_base_blend_factor(&mut c, 2.0);
        assert_eq!(
            c.taa_base_blend_factor,
            CameraComponent::MAX_TAA_BASE_BLEND_FACTOR
        );
        set_taa_variance_clamp_scale(&mut c, -1.0);
        assert_eq!(
            c.taa_variance_clamp_scale,
            CameraComponent::MIN_TAA_VARIANCE_CLAMP_SCALE
        );
        set_taa_jitter_scale(&mut c, 5.0);
        assert_eq!(c.taa_jitter_scale, 1.0);
        set_taa_mip_bias(&mut c, -5.0);
        assert_eq!(c.taa_mip_bias, CameraComponent::MIN_TAA_MIP_BIAS);
        set_taa_contrast_adaptive_sharpening(&mut c, 9.0);
        assert_eq!(c.taa_contrast_adaptive_sharpening, 1.0);
    }

    #[test]
    fn each_knob_clamps_to_its_own_range() {
        // The slider's range and the op's clamp are one pair of bounds.
        for knob in TAA_KNOBS {
            let mut c = CameraComponent::default();
            (knob.set)(&mut c, -1e6);
            assert_eq!((knob.get)(&c), knob.range.0, "{} low", knob.field);
            (knob.set)(&mut c, 1e6);
            assert_eq!((knob.get)(&c), knob.range.1, "{} high", knob.field);
        }
    }

    #[test]
    fn camera_knob_resolves_only_taa_paths() {
        assert!(camera_knob("taa.base_blend_factor").is_some());
        assert!(camera_knob("taa.mip_bias").is_some());
        assert!(camera_knob("taa.nope").is_none());
        assert!(camera_knob("motion_blur.intensity").is_none());
        assert!(camera_knob("bloom.intensity").is_none());
    }
}
