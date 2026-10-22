//! src/components/visual_correction/ — Post-process component
//!
//! bloom/exposure/SSR settings. Unity: post-process volume. Moved verbatim from
//! the former `core/scene.rs`, then extended (phase-3) so the renderer's post-FX
//! chain (color correction, bloom, motion blur, real SSR) actually reads them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

mod tonemap;

pub use tonemap::Tonemap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisualCorrectionComponent {
    pub active: bool,
    pub bloom_active: bool,
    pub bloom_intensity: f32,
    pub bloom_threshold: f32,
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub ssr_active: bool,
    pub ssr_quality: String, // "Low", "Medium", "High", "Ultra"
    pub ssr_temporal_upsampling: bool,
    /// Tonemap operator; ACES when the file names none.
    #[serde(default)]
    pub tonemap: Tonemap,
    /// Display-gamma *tweak* around the neutral `1.0` (#415): the final image is
    /// `c^(1/gamma)` in linear space, then the sRGB render target does the real
    /// linear→display encode. `> 1` brightens midtones, `< 1` darkens them.
    /// Saved as `display_gamma`; neutral when absent.
    #[serde(rename = "display_gamma", default = "default_gamma")]
    pub gamma: f32,
    /// Directional-shadow cascades and reach (#435); the engine defaults when absent.
    #[serde(default)]
    pub shadows: ShadowSettings,
    /// Screen-space ambient occlusion (#436); the engine defaults (on) when absent.
    #[serde(default)]
    pub ssao: SsaoSettings,
    /// Authored postfx modules (`Shader.Bake` with `pass = "postfx"`, #397), run in
    /// this order after tonemapping and before FXAA. Each is a module *name*, the
    /// file the renderer loads from the authored-shader workspace. Empty when absent.
    #[serde(default)]
    pub custom_effects: Vec<String>,
    /// Runtime params of the authored effects (#671), by the name a script set them
    /// under (`"damage_vignette.intensity"` → `[0.7]`): Unity's volume-profile
    /// overrides, without the blending. Every effect in `custom_effects` whose layout
    /// resolves a name draws with its value; the rest keep their baked defaults.
    /// Empty when absent.
    #[serde(default)]
    pub post_params: BTreeMap<String, Vec<f32>>,
}

/// How the sun's cascaded shadow map covers the view (#435) — HDRP's Shadows volume
/// override: how many cascades split the view, and how far from the camera shadows
/// reach. Without an active volume the renderer uses [`ShadowSettings::default`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShadowSettings {
    /// Cascades the view is split into, `1..=4`. More cascades keep near shadows
    /// sharp over a longer reach, at one more depth pass each.
    pub cascades: u32,
    /// Distance from the camera, in world units, past which nothing casts or receives
    /// a sun shadow (they fade out over its last tenth).
    pub distance: f32,
}

impl ShadowSettings {
    /// The cascade-count range the shadow map supports.
    pub const MAX_CASCADES: u32 = 4;
    /// The shortest shadow reach a write accepts.
    pub const MIN_DISTANCE: f32 = 1.0;
}

impl Default for ShadowSettings {
    /// Four cascades over 100 m: the whole of a large room or a street-scale outdoor
    /// space shadowed, with the first cascade a few metres deep for first-person
    /// contact shadows. Unity's High tier is 4 cascades over 150 m; 100 m spends the
    /// same four maps on less ground, since a shooter's fights are closer than that.
    fn default() -> Self {
        Self {
            cascades: Self::MAX_CASCADES,
            distance: 100.0,
        }
    }
}

/// Screen-space ambient occlusion (#436) — HDRP's Ambient Occlusion volume override.
/// Darkens only the ambient/indirect light where geometry crowds a point (a crate's
/// foot on the floor, a room's corners), never direct light. The quality tier decides
/// the resolution and sample count; these are the look.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SsaoSettings {
    /// Whether the AO passes run at all. Off costs nothing.
    pub active: bool,
    /// World-space reach of the occlusion search: how far from a surface another
    /// surface still shades it.
    pub radius: f32,
    /// How strongly occlusion darkens the ambient term; `0` is none, `1` the
    /// physically-plausible amount, above `1` exaggerated.
    pub intensity: f32,
}

impl SsaoSettings {
    /// The shortest radius a write accepts.
    pub const MIN_RADIUS: f32 = 0.01;
    /// The strongest intensity a write accepts.
    pub const MAX_INTENSITY: f32 = 4.0;
}

impl Default for SsaoSettings {
    /// On, half a metre, plain strength: the contact and corner shading an interior
    /// needs, without the dark halo a wider radius paints around a character.
    fn default() -> Self {
        Self {
            active: true,
            radius: 0.5,
            intensity: 1.0,
        }
    }
}

fn default_gamma() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::VisualCorrectionComponent;

    const BODY: &str = r#""active": true, "bloom_active": false, "bloom_intensity": 1.0,
        "bloom_threshold": 1.0, "exposure": 0.0, "contrast": 1.0, "saturation": 1.0,
        "ssr_active": false, "ssr_quality": "Low", "ssr_temporal_upsampling": false"#;

    fn load(extra: &str) -> VisualCorrectionComponent {
        serde_json::from_str(&format!("{{{BODY}{extra}}}")).unwrap()
    }

    #[test]
    fn display_gamma_loads_and_absence_is_neutral() {
        assert_eq!(load(r#", "display_gamma": 1.3"#).gamma, 1.3);
        assert_eq!(load("").gamma, 1.0);
    }

    #[test]
    fn a_scene_without_shadow_settings_loads_the_defaults() {
        assert_eq!(load("").shadows, super::ShadowSettings::default());
        let vc = load(r#", "shadows": { "cascades": 2 }"#);
        assert_eq!((vc.shadows.cascades, vc.shadows.distance), (2, 100.0));
    }

    #[test]
    fn a_scene_without_ssao_settings_loads_the_defaults() {
        assert_eq!(load("").ssao, super::SsaoSettings::default());
        let vc = load(r#", "ssao": { "radius": 1.5 }"#);
        assert_eq!(
            (vc.ssao.active, vc.ssao.radius, vc.ssao.intensity),
            (true, 1.5, 1.0)
        );
    }

    #[test]
    fn custom_effects_default_empty_and_round_trip_in_order() {
        assert!(load("").custom_effects.is_empty());
        let vc = load(r#", "custom_effects": ["crt", "vignette_red"]"#);
        let back: VisualCorrectionComponent =
            serde_json::from_str(&serde_json::to_string(&vc).unwrap()).unwrap();
        assert_eq!(back.custom_effects, ["crt", "vignette_red"]);
    }

    #[test]
    fn post_params_default_empty_and_round_trip() {
        assert!(load("").post_params.is_empty());
        let vc = load(r#", "post_params": {"damage_vignette.intensity": [0.7]}"#);
        let back: VisualCorrectionComponent =
            serde_json::from_str(&serde_json::to_string(&vc).unwrap()).unwrap();
        assert_eq!(back.post_params["damage_vignette.intensity"], [0.7]);
    }

    #[test]
    fn a_saved_component_round_trips_under_display_gamma() {
        let mut vc = load("");
        vc.gamma = 1.25;
        let json = serde_json::to_string(&vc).unwrap();
        assert!(json.contains(r#""display_gamma":1.25"#), "{json}");
        let back: VisualCorrectionComponent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.gamma, 1.25);
    }
}
