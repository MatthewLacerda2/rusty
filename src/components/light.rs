//! src/components/light.rs — Light component
//!
//! ambient/directional/point/spot. Unity: Light. Moved verbatim from the legacy
//! `core/scene.rs`; the per-light [`LightMode`] (#438) is Unity's Light Mode.

use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LightType {
    Ambient,
    Directional,
    Point,
    Spotlight,
}

/// How a light takes part in baked lighting (#438) — Unity's Light Mode.
///
/// * `Realtime` — direct light and shadows every frame; the lightmap bake ignores it.
/// * `Mixed` — Unity's *Baked Indirect*: realtime direct light and shadows, plus its
///   bounce baked into lightmaps. The default, because it is what every light did
///   before modes existed (rendered live, bounced into the probe bake).
/// * `Baked` — direct and bounce both baked; lightmapped surfaces skip it at runtime,
///   so it costs nothing there. Surfaces without a lightmap still light from it live.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightMode {
    Realtime,
    #[default]
    Mixed,
    Baked,
}

impl LightMode {
    /// Every mode, in inspector order.
    pub const ALL: [LightMode; 3] = [LightMode::Realtime, LightMode::Mixed, LightMode::Baked];

    /// The mode's name, as the API and the inspector spell it.
    pub fn name(self) -> &'static str {
        match self {
            LightMode::Realtime => "Realtime",
            LightMode::Mixed => "Mixed",
            LightMode::Baked => "Baked",
        }
    }

    /// Parse a mode name, case-insensitively; `None` for an unknown one.
    pub fn parse(name: &str) -> Option<LightMode> {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name))
    }

    /// Whether the lightmap bake includes this light's direct light (`Baked` only).
    pub fn bakes_direct(self) -> bool {
        self == LightMode::Baked
    }

    /// Whether the lightmap bake includes this light's bounce (`Mixed` and `Baked`).
    pub fn bakes_indirect(self) -> bool {
        self != LightMode::Realtime
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightComponent {
    pub light_type: LightType,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub inner_cone: f32, // Degrees
    pub outer_cone: f32, // Degrees
    /// Whether a point or spot light casts shadows, through the shadow atlas (#468).
    /// Off by default, as in Unity: each shadowed light costs atlas space and a depth
    /// pass per tile (six for a point light). The sun ignores it and always casts its
    /// cascades.
    #[serde(default)]
    pub cast_shadows: bool,
    /// Realtime / Mixed / Baked (#438). `#[serde(default)]` so pre-#438 scenes load
    /// every light as `Mixed`, which renders exactly as they did.
    #[serde(default)]
    pub mode: LightMode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_names_round_trip_and_bake_rules() {
        for mode in LightMode::ALL {
            assert_eq!(LightMode::parse(&mode.name().to_lowercase()), Some(mode));
        }
        assert_eq!(LightMode::parse("shadowmask"), None);
        assert_eq!(LightMode::default(), LightMode::Mixed);
        assert!(!LightMode::Realtime.bakes_indirect() && !LightMode::Realtime.bakes_direct());
        assert!(LightMode::Mixed.bakes_indirect() && !LightMode::Mixed.bakes_direct());
        assert!(LightMode::Baked.bakes_indirect() && LightMode::Baked.bakes_direct());
    }

    #[test]
    fn pre_mode_light_loads_as_mixed() {
        let json = r#"{"light_type":"Point","color":[1,1,1],"intensity":1,
            "range":5,"inner_cone":0,"outer_cone":0}"#;
        let l: LightComponent = serde_json::from_str(json).unwrap();
        assert_eq!(l.mode, LightMode::Mixed);
    }
}
