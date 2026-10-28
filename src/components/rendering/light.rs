//! src/components/rendering/light.rs — Light component
//!
//! ambient/directional/point/spot. Unity: Light. Moved verbatim from the former
//! `core/scene.rs`; the per-light [`LightMode`] (#438) is Unity's Light Mode.

use glam::Vec3;
use serde::{Deserialize, Serialize};

crate::core::named_enum! {
    /// What kind of light it is. Its names are the editor's, the API's and the scene
    /// snapshot's: Unity's (`LightType.Spot`, #997).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum LightType {
        Ambient = "Ambient",
        Directional = "Directional",
        Point = "Point",
        Spotlight = "Spot",
    }
}

crate::core::named_enum! {
    /// How a light takes part in baked lighting (#438) — Unity's Light Mode.
    ///
    /// * `Realtime` — direct light and shadows every frame; neither the lightmap bake nor
    ///   the probe bake sees it, so it bounces nowhere.
    /// * `Mixed` — Unity's *Baked Indirect*: realtime direct light and shadows, plus its
    ///   bounce baked into lightmaps and light probes. The default, because it is what
    ///   every light did before modes existed (rendered live, bounced into the probe bake).
    /// * `Baked` — direct and bounce both baked, and never drawn live (#809): static
    ///   surfaces get it from their lightmap, dynamic objects from the light probes (whose
    ///   bake adds its direct light). It costs nothing at runtime and casts no realtime
    ///   shadow.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum LightMode {
        Realtime = "Realtime",
        #[default]
        Mixed = "Mixed",
        Baked = "Baked",
    }
}

impl LightMode {
    /// Whether the lightmap bake includes this light's direct light (`Baked` only).
    pub fn bakes_direct(self) -> bool {
        self == LightMode::Baked
    }

    /// Whether the lightmap bake includes this light's bounce (`Mixed` and `Baked`).
    pub fn bakes_indirect(self) -> bool {
        self != LightMode::Realtime
    }

    /// Whether the renderer's light loop draws this light (#809). In a frame,
    /// `Realtime` and `Mixed`: a `Baked` light reaches surfaces only through lightmaps
    /// and probes. In a bake `capture` (the probe and reflection bakes'
    /// `capture_static_cubemap`), the lights whose bounce is baked: `Mixed` and `Baked`.
    pub fn renders_live(self, capture: bool) -> bool {
        if capture {
            self.bakes_indirect()
        } else {
            self != LightMode::Baked
        }
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
    /// Realtime / Mixed / Baked (#438); `Mixed` when the file names none.
    #[serde(default)]
    pub mode: LightMode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_bake_rules() {
        assert_eq!(LightMode::default(), LightMode::Mixed);
        assert!(!LightMode::Realtime.bakes_indirect() && !LightMode::Realtime.bakes_direct());
        assert!(LightMode::Mixed.bakes_indirect() && !LightMode::Mixed.bakes_direct());
        assert!(LightMode::Baked.bakes_indirect() && LightMode::Baked.bakes_direct());
    }

    #[test]
    fn only_unitys_type_names_parse() {
        assert_eq!(LightType::Spotlight.name(), "Spot", "Unity's name (#997)");
        assert_eq!(
            LightType::parse("Spotlight"),
            None,
            "the pre-#997 name is gone"
        );
    }

    #[test]
    fn frames_drop_baked_lights_and_captures_drop_realtime_ones() {
        let live = |capture| LightMode::ALL.map(|m| m.renders_live(capture));
        assert_eq!(live(false), [true, true, false]);
        assert_eq!(live(true), [false, true, true]);
    }

    #[test]
    fn pre_mode_light_loads_as_mixed() {
        let json = r#"{"light_type":"Point","color":[1,1,1],"intensity":1,
            "range":5,"inner_cone":0,"outer_cone":0}"#;
        let l: LightComponent = serde_json::from_str(json).unwrap();
        assert_eq!(l.mode, LightMode::Mixed);
    }
}
