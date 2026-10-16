//! src/components/visual_correction.rs — Post-process component
//!
//! bloom/exposure/SSR settings. Unity: post-process volume. Moved verbatim from
//! the legacy `core/scene.rs`, then extended (phase-3) so the renderer's post-FX
//! chain (color correction, bloom, motion blur, real SSR) actually reads them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Tonemapping operator applied as the final color-correction step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tonemap {
    /// No tonemap — clamp only. Mostly useful for debugging.
    None,
    /// Reinhard `c / (1 + c)` — cheap, soft rolloff.
    Reinhard,
    /// Filmic ACES fit — the default, richer contrast in highlights.
    #[default]
    Aces,
}

impl Tonemap {
    /// Stable index handed to the shader as a `u32`.
    pub fn to_index(self) -> u32 {
        match self {
            Tonemap::None => 0,
            Tonemap::Reinhard => 1,
            Tonemap::Aces => 2,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "VisualCorrectionFile")]
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
    /// Tonemap operator. `#[serde(default)]` keeps older scene files loading.
    #[serde(default)]
    pub tonemap: Tonemap,
    /// Display-gamma *tweak* around the neutral `1.0` (#415): the final image is
    /// `c^(1/gamma)` in linear space, then the sRGB render target does the real
    /// linear→display encode. `> 1` brightens midtones, `< 1` darkens them.
    ///
    /// Saved as `display_gamma`; the pre-#415 key `gamma` meant "the encode itself"
    /// (2.2 was neutral), so it loads as `gamma / 2.2` — see [`LEGACY_NEUTRAL_GAMMA`].
    #[serde(rename = "display_gamma")]
    pub gamma: f32,
    /// Directional-shadow cascades and reach (#435). `#[serde(default)]` loads older
    /// scenes with the engine defaults.
    pub shadows: ShadowSettings,
    /// Screen-space ambient occlusion (#436). `#[serde(default)]` loads older scenes
    /// with the engine defaults (on).
    #[serde(default)]
    pub ssao: SsaoSettings,
    /// Authored postfx modules (`Shader.Bake` with `pass = "postfx"`, #397), run in
    /// this order after tonemapping and before FXAA. Each is a module *name*, the
    /// file the renderer loads from the authored-shader workspace. `#[serde(default)]`
    /// loads older scenes with none.
    #[serde(default)]
    pub custom_effects: Vec<String>,
    /// Runtime params of the authored effects (#671), by the name a script set them
    /// under (`"damage_vignette.intensity"` → `[0.7]`): Unity's volume-profile
    /// overrides, without the blending. Every effect in `custom_effects` whose layout
    /// resolves a name draws with its value; the rest keep their baked defaults.
    /// `#[serde(default)]` loads older scenes with none.
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

/// What the pre-#415 `gamma` field held as its neutral value. Dividing an old
/// value by it keeps the user's deviation from neutral: `c^(1/g_old)` ≈
/// `srgb(c^(2.2/g_old))`, i.e. a new tweak of `g_old / 2.2`.
pub const LEGACY_NEUTRAL_GAMMA: f32 = 2.2;

fn default_gamma() -> f32 {
    1.0
}

/// The on-disk shape: today's fields plus the legacy `gamma` key, folded into
/// [`VisualCorrectionComponent::gamma`] by the `From` impl below.
#[derive(Deserialize)]
struct VisualCorrectionFile {
    active: bool,
    bloom_active: bool,
    bloom_intensity: f32,
    bloom_threshold: f32,
    exposure: f32,
    contrast: f32,
    saturation: f32,
    ssr_active: bool,
    ssr_quality: String,
    ssr_temporal_upsampling: bool,
    #[serde(default)]
    tonemap: Tonemap,
    display_gamma: Option<f32>,
    /// Pre-#415 encode gamma; only read when `display_gamma` is absent.
    gamma: Option<f32>,
    #[serde(default)]
    shadows: ShadowSettings,
    #[serde(default)]
    ssao: SsaoSettings,
    #[serde(default)]
    custom_effects: Vec<String>,
    #[serde(default)]
    post_params: BTreeMap<String, Vec<f32>>,
}

impl From<VisualCorrectionFile> for VisualCorrectionComponent {
    fn from(f: VisualCorrectionFile) -> Self {
        let gamma = f
            .display_gamma
            .or(f.gamma.map(|g| g / LEGACY_NEUTRAL_GAMMA))
            .unwrap_or_else(default_gamma);
        Self {
            active: f.active,
            bloom_active: f.bloom_active,
            bloom_intensity: f.bloom_intensity,
            bloom_threshold: f.bloom_threshold,
            exposure: f.exposure,
            contrast: f.contrast,
            saturation: f.saturation,
            ssr_active: f.ssr_active,
            ssr_quality: f.ssr_quality,
            ssr_temporal_upsampling: f.ssr_temporal_upsampling,
            tonemap: f.tonemap,
            gamma,
            shadows: f.shadows,
            ssao: f.ssao,
            custom_effects: f.custom_effects,
            post_params: f.post_params,
        }
    }
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
    fn a_legacy_encode_gamma_loads_relative_to_its_old_neutral() {
        // A saved 2.2 meant "normal"; it must not load as a 2.2 brightening tweak.
        assert!((load(r#", "gamma": 2.2"#).gamma - 1.0).abs() < 1e-6);
        assert!((load(r#", "gamma": 1.1"#).gamma - 0.5).abs() < 1e-6);
    }

    #[test]
    fn display_gamma_wins_and_absence_is_neutral() {
        assert_eq!(load(r#", "display_gamma": 1.3, "gamma": 2.2"#).gamma, 1.3);
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
    fn a_saved_component_round_trips_under_the_new_key() {
        let mut vc = load("");
        vc.gamma = 1.25;
        let json = serde_json::to_string(&vc).unwrap();
        assert!(json.contains(r#""display_gamma":1.25"#), "{json}");
        let back: VisualCorrectionComponent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.gamma, 1.25);
    }
}
